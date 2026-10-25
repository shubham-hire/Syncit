//! Wall-clock playback scheduling for one listener stream.
//!
//! The scheduler turns the caller-driven listener pipeline into a paced
//! output: it drains the socket, keeps the session's heartbeat and clock-sync
//! cadence, and releases decoded or concealed PCM frames on a due-time
//! timeline derived from the first buffered packet. Platform audio backends
//! implement [`PcmSink`]; tests and diagnostics use [`NullSink`].

use crate::{
    ListenerAudioFrame, ListenerRuntime, ListenerRuntimeError, ListenerRuntimeEvent,
    monotonic_micros,
};
use audiosync_session::SessionEvent;
use std::fmt;
use std::time::{Duration, Instant};

/// Failures raised while moving decoded audio to a platform output.
#[derive(Debug)]
pub enum PlaybackError {
    Runtime(ListenerRuntimeError),
    Sink { message: String },
}

impl fmt::Display for PlaybackError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Runtime(error) => write!(formatter, "{error}"),
            Self::Sink { message } => write!(formatter, "PCM sink failed: {message}"),
        }
    }
}

impl std::error::Error for PlaybackError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Runtime(error) => Some(error),
            Self::Sink { .. } => None,
        }
    }
}

impl From<ListenerRuntimeError> for PlaybackError {
    fn from(error: ListenerRuntimeError) -> Self {
        Self::Runtime(error)
    }
}

/// Consumes one decoded or concealed PCM frame (960 mono samples at 48 kHz).
pub trait PcmSink {
    fn write_frame(&mut self, pcm: &[i16]) -> Result<(), PlaybackError>;
}

/// Discards PCM while counting frames; used by tests and `--null-output`.
#[derive(Debug, Default)]
pub struct NullSink {
    pub frames_written: u64,
    pub samples_seen: u64,
}

impl PcmSink for NullSink {
    fn write_frame(&mut self, pcm: &[i16]) -> Result<(), PlaybackError> {
        self.frames_written = self.frames_written.saturating_add(1);
        self.samples_seen = self.samples_seen.saturating_add(pcm.len() as u64);
        Ok(())
    }
}

/// Tuning knobs for playback pacing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlaybackConfig {
    /// One frame duration. Must match the codec contract.
    pub frame_duration_micros: u64,
    /// Head start applied before the first frame is released, sized to absorb
    /// network jitter. Tune from real hotspot measurements.
    pub target_buffer: Duration,
    /// How long a missing frame stays overdue before it is concealed and the
    /// timeline advances without it.
    pub conceal_grace: Duration,
    /// Manual per-listener calibration delay added to every due time.
    pub listener_delay: Duration,
    /// Heartbeat keep-alive cadence.
    pub heartbeat_period: Duration,
    /// Clock re-synchronization cadence.
    pub clock_sync_period: Duration,
    /// Upper bound on datagrams drained per pump call.
    pub drain_limit: usize,
    /// Silence from the host for this long marks the session timed out.
    pub host_loss_timeout: Duration,
    /// First delay before an automatic rejoin attempt after a timeout.
    pub rejoin_backoff_start: Duration,
    /// Upper bound on the rejoin backoff (PRD target: recovery within ~2 s).
    pub rejoin_backoff_max: Duration,
    /// Whether the scheduler re-joins automatically after a watchdog timeout.
    pub auto_rejoin: bool,
}

impl Default for PlaybackConfig {
    fn default() -> Self {
        Self {
            frame_duration_micros: 20_000,
            target_buffer: Duration::from_millis(100),
            conceal_grace: Duration::from_millis(60),
            listener_delay: Duration::ZERO,
            heartbeat_period: Duration::from_secs(1),
            clock_sync_period: Duration::from_secs(2),
            drain_limit: 64,
            host_loss_timeout: Duration::from_secs(3),
            rejoin_backoff_start: Duration::from_millis(400),
            rejoin_backoff_max: Duration::from_secs(2),
            auto_rejoin: true,
        }
    }
}

/// Counters reported by the scheduler to UI and diagnostics.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PlaybackStats {
    pub rendered_frames: u64,
    pub concealed_frames: u64,
    pub sink_failures: u64,
    pub watchdog_timeouts: u64,
    pub rejoin_attempts: u64,
}

/// Paces one [`ListenerRuntime`] against wall-clock time.
#[derive(Debug)]
pub struct PlaybackScheduler {
    config: PlaybackConfig,
    next_due: Option<Instant>,
    next_heartbeat: Instant,
    next_clock_sync: Instant,
    control_events: Vec<SessionEvent>,
    stats: PlaybackStats,
    last_host_traffic: Instant,
    next_rejoin_at: Option<Instant>,
    rejoin_backoff: Duration,
}

impl PlaybackScheduler {
    pub fn new(config: PlaybackConfig) -> Self {
        let now = Instant::now();
        Self {
            config,
            next_due: None,
            next_heartbeat: now + config.heartbeat_period,
            next_clock_sync: now + config.clock_sync_period,
            control_events: Vec::new(),
            stats: PlaybackStats::default(),
            last_host_traffic: now,
            next_rejoin_at: None,
            rejoin_backoff: config.rejoin_backoff_start,
        }
    }

    pub fn stats(&self) -> PlaybackStats {
        self.stats
    }

    /// Replaces the tuning configuration while preserving timed cadence.
    ///
    /// A changed listener delay shifts the active render timeline by the same
    /// delta. Other cadence deadlines remain untouched.
    pub fn set_config(&mut self, config: PlaybackConfig) {
        let previous_delay = self.config.listener_delay;
        self.config = config;
        self.shift_next_due(previous_delay, config.listener_delay);
    }

    /// Applies a manual calibration delay without resetting the other playback
    /// settings that were chosen when the listener started.
    pub fn set_listener_delay(&mut self, delay: Duration) {
        let previous_delay = self.config.listener_delay;
        self.config.listener_delay = delay;
        self.shift_next_due(previous_delay, delay);
    }

    /// Drains session/control events surfaced by the last pumps.
    pub fn take_control_events(&mut self) -> Vec<SessionEvent> {
        std::mem::take(&mut self.control_events)
    }

    /// One scheduler iteration: drain the socket, keep session cadence, then
    /// release every frame whose due time has passed.
    pub fn pump(
        &mut self,
        runtime: &mut ListenerRuntime,
        sink: &mut dyn PcmSink,
    ) -> Result<PlaybackStats, PlaybackError> {
        self.drain_socket(runtime)?;
        self.keep_session_cadence(runtime);
        self.run_watchdog(runtime);

        loop {
            let now = Instant::now();
            let Some(due) = self.next_due else {
                if runtime.pipeline().buffered_packets() > 0 {
                    // Prefer the host packet timeline when clock sync is
                    // available. Until then, fall back to local arrival time.
                    self.next_due = Some(self.arm_timeline(runtime, now));
                    continue;
                }
                break;
            };
            if now < due {
                break;
            }

            match runtime.render_next()? {
                Some(frame) => {
                    if let Err(error) = sink.write_frame(frame_pcm(&frame)) {
                        self.stats.sink_failures = self.stats.sink_failures.saturating_add(1);
                        return Err(error);
                    }
                    self.stats.rendered_frames = self.stats.rendered_frames.saturating_add(1);
                    self.next_due = Some(due + self.frame_duration());
                }
                None if now >= due + self.config.conceal_grace => match runtime.conceal_next()? {
                    Some(frame) => {
                        if let Err(error) = sink.write_frame(frame_pcm(&frame)) {
                            self.stats.sink_failures = self.stats.sink_failures.saturating_add(1);
                            return Err(error);
                        }
                        self.stats.concealed_frames = self.stats.concealed_frames.saturating_add(1);
                        self.next_due = Some(due + self.frame_duration());
                    }
                    // No tracked sequence left (stream never started or fully
                    // drained): go idle until a packet re-arms the timeline.
                    None => self.next_due = None,
                },
                // Frame missing but still inside the grace window; a late
                // packet may still arrive before the deadline.
                None => break,
            }
        }

        Ok(self.stats)
    }

    fn drain_socket(&mut self, runtime: &mut ListenerRuntime) -> Result<(), PlaybackError> {
        for _ in 0..self.config.drain_limit {
            // Taken just before the blocking receive: a datagram that waited
            // in the socket inflates this sample's RTT, which the estimator's
            // lowest-RTT filter naturally discards.
            let client_received_micros = monotonic_micros();
            match runtime.receive_once(Some(client_received_micros)) {
                Ok(ListenerRuntimeEvent::Audio(_)) => {
                    self.last_host_traffic = Instant::now();
                }
                Ok(ListenerRuntimeEvent::Control(events)) => {
                    self.last_host_traffic = Instant::now();
                    if self.control_events.len() < 1_024 {
                        self.control_events.extend(events);
                    }
                }
                Err(ListenerRuntimeError::Io(error))
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) =>
                {
                    return Ok(());
                }
                // Already counted by the pipeline metric; forged traffic must
                // not interrupt playback.
                Err(ListenerRuntimeError::UnauthenticatedPacket) => return Ok(()),
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
    }

    /// Watchdog: mark the session timed out after sustained host silence,
    /// then attempt an automatic rejoin on a capped backoff. Any host traffic
    /// resets the backoff, so a listener that re-establishes contact quickly
    /// retries fast the next time.
    fn run_watchdog(&mut self, runtime: &mut ListenerRuntime) {
        use audiosync_session::ListenerState;

        let state = runtime.session().state();
        match state {
            ListenerState::TimedOut => {
                if !self.config.auto_rejoin {
                    return;
                }
                let Some(not_before) = self.next_rejoin_at else {
                    self.next_rejoin_at = Some(Instant::now());
                    return;
                };
                if Instant::now() >= not_before && runtime.send_join().is_ok() {
                    self.stats.rejoin_attempts = self.stats.rejoin_attempts.saturating_add(1);
                    self.rejoin_backoff =
                        (self.rejoin_backoff * 2).min(self.config.rejoin_backoff_max);
                    self.next_rejoin_at = Some(Instant::now() + self.rejoin_backoff);
                }
            }
            ListenerState::Disconnected | ListenerState::Rejected => {}
            ListenerState::PendingApproval | ListenerState::Active => {
                if Instant::now() >= self.last_host_traffic + self.config.host_loss_timeout
                    && runtime.session_mut().mark_timed_out().is_some()
                {
                    self.stats.watchdog_timeouts = self.stats.watchdog_timeouts.saturating_add(1);
                    self.rejoin_backoff = self.config.rejoin_backoff_start;
                    self.next_rejoin_at = Some(Instant::now());
                    self.push_event(SessionEvent::StateChanged(ListenerState::TimedOut));
                }
            }
            ListenerState::Removed | ListenerState::SessionEnded => {
                self.next_rejoin_at = None;
            }
        }
    }

    fn push_event(&mut self, event: SessionEvent) {
        if self.control_events.len() < 1_024 {
            self.control_events.push(event);
        }
    }

    fn keep_session_cadence(&mut self, runtime: &mut ListenerRuntime) {
        let now = Instant::now();
        if now >= self.next_heartbeat && runtime.send_heartbeat().is_ok() {
            self.next_heartbeat = now + self.config.heartbeat_period;
        }
        if now >= self.next_clock_sync && runtime.send_clock_sync(monotonic_micros()).is_ok() {
            self.next_clock_sync = now + self.config.clock_sync_period;
        }
    }

    fn frame_duration(&self) -> Duration {
        Duration::from_micros(self.config.frame_duration_micros)
    }

    fn arm_timeline(&self, runtime: &ListenerRuntime, now: Instant) -> Instant {
        let timeline_base = runtime
            .pipeline()
            .next_packet_timestamp_micros()
            .and_then(|timestamp| {
                runtime
                    .session()
                    .clock_offset_micros()
                    .map(|offset| (timestamp, offset))
            })
            .and_then(|(timestamp, offset)| host_timestamp_to_instant(timestamp, offset, now))
            .unwrap_or(now);
        let with_buffer = timeline_base
            .checked_add(self.config.target_buffer)
            .unwrap_or(timeline_base);
        with_buffer
            .checked_add(self.config.listener_delay)
            .unwrap_or(with_buffer)
    }

    fn shift_next_due(&mut self, previous: Duration, next: Duration) {
        let Some(due) = self.next_due else {
            return;
        };
        if next > previous {
            self.next_due = due.checked_add(next - previous).or(Some(due));
        } else if previous > next {
            self.next_due = Some(
                due.checked_sub(previous - next)
                    .unwrap_or_else(Instant::now),
            );
        }
    }
}

/// Converts a host packet timestamp into a listener-local monotonic deadline.
/// `offset_micros` is host-minus-listener, matching the clock estimator's
/// contract.
fn host_timestamp_to_instant(
    host_timestamp_micros: u64,
    offset_micros: i64,
    now: Instant,
) -> Option<Instant> {
    let host_timestamp_micros = i64::try_from(host_timestamp_micros).ok()?;
    let listener_timestamp_micros = host_timestamp_micros.checked_sub(offset_micros)?;
    let local_now_micros = monotonic_micros();
    let delta_micros = listener_timestamp_micros.checked_sub(local_now_micros)?;
    if delta_micros >= 0 {
        now.checked_add(Duration::from_micros(delta_micros as u64))
    } else {
        now.checked_sub(Duration::from_micros(delta_micros.unsigned_abs()))
    }
}

fn frame_pcm(frame: &ListenerAudioFrame) -> &[i16] {
    match frame {
        ListenerAudioFrame::Decoded { pcm, .. } => pcm,
        ListenerAudioFrame::Concealed { pcm, .. } => pcm,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use audiosync_audio::{AudioPacketPump, SyntheticPcmSource};
    use audiosync_codec::CodecConfig;
    use audiosync_session::ListenerState;
    use audiosync_transport::{ControlMessage, ListenerId, SessionToken, UdpTransport};

    fn session_id() -> audiosync_core::SessionId {
        [0x91; 16]
    }

    fn token() -> SessionToken {
        SessionToken::new([0x92; 16])
    }

    fn send_packet(
        sender: &UdpTransport,
        packet: &audiosync_core::AudioPacket,
        destination: std::net::SocketAddr,
    ) {
        sender
            .send_to(&packet.encode().expect("packet encodes"), destination)
            .expect("packet sends");
    }

    #[test]
    fn scheduler_renders_delivered_frames_after_the_target_buffer() {
        let sender = UdpTransport::bind("127.0.0.1:0").expect("sender binds");
        let mut listener = ListenerRuntime::new(
            "127.0.0.1:0",
            session_id(),
            token(),
            sender.local_addr().expect("sender address"),
            CodecConfig::default(),
            16,
        )
        .expect("listener runtime initializes");
        listener
            .set_read_timeout(Some(Duration::from_millis(2)))
            .expect("listener timeout configures");

        let destination = listener.local_addr().expect("listener address");
        let mut pump = AudioPacketPump::new(
            SyntheticPcmSource::new(440.0),
            session_id(),
            token().into_bytes(),
            0,
        )
        .expect("pump initializes");
        for _ in 0..5 {
            let packet = pump.next_packet().expect("packet encodes");
            send_packet(&sender, &packet, destination);
        }

        let mut scheduler = PlaybackScheduler::new(PlaybackConfig {
            target_buffer: Duration::from_millis(40),
            ..PlaybackConfig::default()
        });
        let mut sink = NullSink::default();
        let deadline = Instant::now() + Duration::from_secs(5);
        while sink.frames_written < 5 && Instant::now() < deadline {
            let _ = scheduler.pump(&mut listener, &mut sink);
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(sink.frames_written, 5);
        assert_eq!(scheduler.stats().rendered_frames, 5);
        assert_eq!(scheduler.stats().concealed_frames, 0);
    }

    #[test]
    fn scheduler_uses_clock_synced_packet_timestamp_for_initial_deadline() {
        let sender = UdpTransport::bind("127.0.0.1:0").expect("sender binds");
        let host_endpoint = sender.local_addr().expect("host address");
        let mut listener = ListenerRuntime::new(
            "127.0.0.1:0",
            session_id(),
            token(),
            host_endpoint,
            CodecConfig::default(),
            8,
        )
        .expect("listener runtime initializes");
        listener
            .set_read_timeout(Some(Duration::from_millis(2)))
            .expect("listener timeout configures");

        let listener_id = ListenerId::new([0x08; 16]);
        {
            let session = listener.session_mut();
            let _ = session.handle_message(
                host_endpoint,
                ControlMessage::JoinAccepted {
                    session_id: session_id(),
                    listener_id,
                },
                None,
            );
            let _ = session.handle_message(
                host_endpoint,
                ControlMessage::ListenerApproved {
                    session_id: session_id(),
                    listener_id,
                },
                None,
            );
            let _ = session.handle_message(
                host_endpoint,
                ControlMessage::ClockSyncResponse {
                    session_id: session_id(),
                    client_sent_micros: 0,
                    host_received_micros: 5,
                    host_sent_micros: 5,
                },
                Some(10),
            );
        }
        assert_eq!(listener.session().clock_offset_micros(), Some(0));

        let capture_start = monotonic_micros() as u64 + 120_000;
        let mut pump = AudioPacketPump::new(
            SyntheticPcmSource::new(440.0),
            session_id(),
            token().into_bytes(),
            capture_start,
        )
        .expect("pump initializes");
        let packet = pump.next_packet().expect("packet encodes");
        send_packet(
            &sender,
            &packet,
            listener.local_addr().expect("listener address"),
        );

        let mut scheduler = PlaybackScheduler::new(PlaybackConfig {
            target_buffer: Duration::ZERO,
            conceal_grace: Duration::from_millis(20),
            ..PlaybackConfig::default()
        });
        let mut sink = NullSink::default();
        let started = Instant::now();
        scheduler
            .pump(&mut listener, &mut sink)
            .expect("initial pump succeeds");
        assert_eq!(sink.frames_written, 0);

        let deadline = started + Duration::from_secs(2);
        while sink.frames_written < 1 && Instant::now() < deadline {
            scheduler
                .pump(&mut listener, &mut sink)
                .expect("timestamped pump succeeds");
            std::thread::sleep(Duration::from_millis(5));
        }

        assert_eq!(sink.frames_written, 1);
        assert!(started.elapsed() >= Duration::from_millis(50));
    }

    #[test]
    fn changing_listener_delay_shifts_active_timeline_only() {
        let mut scheduler = PlaybackScheduler::new(PlaybackConfig {
            target_buffer: Duration::from_millis(40),
            conceal_grace: Duration::from_millis(70),
            listener_delay: Duration::from_millis(10),
            ..PlaybackConfig::default()
        });
        let due = Instant::now() + Duration::from_millis(100);
        scheduler.next_due = Some(due);

        scheduler.set_listener_delay(Duration::from_millis(50));
        assert_eq!(
            scheduler.next_due.expect("timeline remains armed"),
            due + Duration::from_millis(40)
        );
        assert_eq!(scheduler.config.target_buffer, Duration::from_millis(40));
        assert_eq!(scheduler.config.conceal_grace, Duration::from_millis(70));

        scheduler.set_listener_delay(Duration::ZERO);
        assert_eq!(
            scheduler.next_due.expect("timeline remains armed"),
            due - Duration::from_millis(10)
        );
    }

    #[test]
    fn watchdog_marks_timeout_and_rejoins_automatically() {
        let host_socket = UdpTransport::bind("127.0.0.1:0").expect("fake host binds");
        let host_endpoint = host_socket.local_addr().expect("fake host address");
        let mut listener = ListenerRuntime::new(
            "127.0.0.1:0",
            session_id(),
            token(),
            host_endpoint,
            CodecConfig::default(),
            8,
        )
        .expect("listener runtime initializes");
        listener
            .set_read_timeout(Some(Duration::from_millis(2)))
            .expect("listener timeout configures");

        // Reach Active without a real host by feeding the session directly.
        let listener_id = ListenerId::new([0x07; 16]);
        {
            let session = listener.session_mut();
            let _ = session.handle_message(
                host_endpoint,
                ControlMessage::JoinAccepted {
                    session_id: session_id(),
                    listener_id,
                },
                None,
            );
            let _ = session.handle_message(
                host_endpoint,
                ControlMessage::ListenerApproved {
                    session_id: session_id(),
                    listener_id,
                },
                None,
            );
        }
        assert_eq!(listener.session().state(), ListenerState::Active);

        let mut scheduler = PlaybackScheduler::new(PlaybackConfig {
            host_loss_timeout: Duration::from_millis(150),
            rejoin_backoff_start: Duration::from_millis(50),
            rejoin_backoff_max: Duration::from_millis(200),
            ..PlaybackConfig::default()
        });
        let mut sink = NullSink::default();

        let deadline = Instant::now() + Duration::from_secs(5);
        while listener.session().state() == ListenerState::Active && Instant::now() < deadline {
            let _ = scheduler.pump(&mut listener, &mut sink);
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(listener.session().state(), ListenerState::TimedOut);
        assert_eq!(scheduler.stats().watchdog_timeouts, 1);

        // Keep pumping until the automatic rejoin fires.
        let deadline = Instant::now() + Duration::from_secs(3);
        while scheduler.stats().rejoin_attempts < 1 && Instant::now() < deadline {
            let _ = scheduler.pump(&mut listener, &mut sink);
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(scheduler.stats().rejoin_attempts >= 1);

        // The rejoin reaches the (fake) host as a JoinRequest.
        host_socket
            .set_read_timeout(Some(Duration::from_millis(500)))
            .expect("fake host timeout configures");
        let mut buffer = [0_u8; 1_024];
        let (length, source) = host_socket
            .recv_from(&mut buffer)
            .expect("rejoin join request arrives");
        assert_eq!(
            source,
            listener.local_addr().expect("listener has an address")
        );
        let message = ControlMessage::decode(&buffer[..length]).expect("rejoin decodes");
        assert!(matches!(message, ControlMessage::JoinRequest { .. }));
        assert!(scheduler.stats().rejoin_attempts >= 1);
    }

    #[test]
    fn scheduler_conceals_a_missing_frame_after_the_grace_window() {
        let sender = UdpTransport::bind("127.0.0.1:0").expect("sender binds");
        let mut listener = ListenerRuntime::new(
            "127.0.0.1:0",
            session_id(),
            token(),
            sender.local_addr().expect("sender address"),
            CodecConfig::default(),
            16,
        )
        .expect("listener runtime initializes");
        listener
            .set_read_timeout(Some(Duration::from_millis(2)))
            .expect("listener timeout configures");

        let destination = listener.local_addr().expect("listener address");
        let mut pump = AudioPacketPump::new(
            SyntheticPcmSource::new(330.0),
            session_id(),
            token().into_bytes(),
            0,
        )
        .expect("pump initializes");
        // Generate sequences 0..3, then deliver all except sequence 1.
        let mut packets = Vec::new();
        for _ in 0..3 {
            packets.push(pump.next_packet().expect("packet encodes"));
        }
        for packet in packets.iter().filter(|packet| packet.sequence != 1) {
            send_packet(&sender, packet, destination);
        }

        let mut scheduler = PlaybackScheduler::new(PlaybackConfig {
            target_buffer: Duration::from_millis(30),
            conceal_grace: Duration::from_millis(80),
            ..PlaybackConfig::default()
        });
        let mut sink = NullSink::default();
        let deadline = Instant::now() + Duration::from_secs(5);
        while (sink.frames_written < 3 || scheduler.stats().concealed_frames < 1)
            && Instant::now() < deadline
        {
            let _ = scheduler.pump(&mut listener, &mut sink);
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(
            sink.frames_written, 3,
            "sequence 0 decoded + 1 concealed + 2 decoded"
        );
        assert_eq!(scheduler.stats().rendered_frames, 2);
        assert_eq!(scheduler.stats().concealed_frames, 1);
    }
}
