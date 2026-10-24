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
        }
    }
}

/// Counters reported by the scheduler to UI and diagnostics.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PlaybackStats {
    pub rendered_frames: u64,
    pub concealed_frames: u64,
    pub sink_failures: u64,
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
        }
    }

    pub fn stats(&self) -> PlaybackStats {
        self.stats
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

        loop {
            let now = Instant::now();
            let Some(due) = self.next_due else {
                if runtime.pipeline().buffered_packets() > 0 {
                    // Arm the timeline on the first buffered packet: the
                    // pre-roll absorbs jitter, the delay applies manual
                    // calibration.
                    self.next_due =
                        Some(now + self.config.target_buffer + self.config.listener_delay);
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
                Ok(ListenerRuntimeEvent::Audio(_)) => {}
                Ok(ListenerRuntimeEvent::Control(events)) => {
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
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
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
    use audiosync_transport::{SessionToken, UdpTransport};

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
        let mut pump = AudioPacketPump::new(SyntheticPcmSource::new(440.0), session_id(), 0)
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
        let mut pump = AudioPacketPump::new(SyntheticPcmSource::new(330.0), session_id(), 0)
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
