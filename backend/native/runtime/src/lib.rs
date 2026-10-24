//! Runtime orchestration for the platform-neutral AudioSync prototype.
//!
//! This crate deliberately owns the boundary between state machines and I/O:
//! the lower-level session crate remains socket-free, while this crate can bind
//! UDP sockets, dispatch control messages, send timestamped audio, and expose a
//! listener-side decoded PCM frame to a platform audio adapter.

use audiosync_audio::{AudioPacketPump, AudioPacketPumpError, PcmSource};
use audiosync_codec::{CodecConfig, CodecError, OpusDecoder};
use audiosync_core::{
    AUDIO_PACKET_MAGIC, AudioPacket, JitterBuffer, PacketError, PushResult, SessionId,
};
use audiosync_session::{HostSession, ListenerSession, SessionAction, SessionError, SessionEvent};
use audiosync_transport::{
    AudioFanoutReport, AudioPacketFanout, CONTROL_MAGIC, ControlMessage, ControlMessageError,
    ListenerId, SessionToken, UdpTransport,
};
use std::fmt;
use std::io;
use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::OnceLock;
use std::time::Instant;

mod host_loop;
mod playback;

pub use host_loop::{
    CommandChannelClosed, HostCommand, HostLoopConfig, HostLoopError, HostLoopHandle,
    spawn_host_loop,
};
pub use playback::{
    NullSink, PcmSink, PlaybackConfig, PlaybackError, PlaybackScheduler, PlaybackStats,
};

/// Microseconds on a stable process-wide monotonic clock.
///
/// The epoch is arbitrary; only differences and the NTP-style clock-offset
/// estimator rely on these values. Host and listener each use their own local
/// monotonic clock, which is exactly the offset being estimated.
pub fn monotonic_micros() -> i64 {
    static EPOCH: OnceLock<Instant> = OnceLock::new();
    let elapsed = EPOCH.get_or_init(Instant::now).elapsed().as_micros();
    i64::try_from(elapsed).unwrap_or(i64::MAX)
}

/// Errors returned by the host runtime while dispatching control or audio.
#[derive(Debug)]
pub enum HostRuntimeError {
    Io(io::Error),
    Control(ControlMessageError),
    Session(SessionError),
    Audio(AudioPacketPumpError),
    Packet(PacketError),
}

impl fmt::Display for HostRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "host runtime I/O failed: {error}"),
            Self::Control(error) => write!(formatter, "host control message failed: {error}"),
            Self::Session(error) => write!(formatter, "host session operation failed: {error}"),
            Self::Audio(error) => write!(formatter, "host audio pump failed: {error}"),
            Self::Packet(error) => write!(formatter, "host audio packet failed: {error}"),
        }
    }
}

impl std::error::Error for HostRuntimeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Control(error) => Some(error),
            Self::Session(error) => Some(error),
            Self::Audio(error) => Some(error),
            Self::Packet(error) => Some(error),
        }
    }
}

impl From<io::Error> for HostRuntimeError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<ControlMessageError> for HostRuntimeError {
    fn from(error: ControlMessageError) -> Self {
        Self::Control(error)
    }
}

impl From<SessionError> for HostRuntimeError {
    fn from(error: SessionError) -> Self {
        Self::Session(error)
    }
}

impl From<AudioPacketPumpError> for HostRuntimeError {
    fn from(error: AudioPacketPumpError) -> Self {
        Self::Audio(error)
    }
}

impl From<PacketError> for HostRuntimeError {
    fn from(error: PacketError) -> Self {
        Self::Packet(error)
    }
}

/// A socket-owning host runtime driven by an outer scheduler.
///
/// The runtime uses one UDP socket for the prototype control and audio paths.
/// Callers should invoke [`Self::receive_control_once`] when control data is
/// available and [`Self::send_next_audio`] on the 20 ms audio cadence. The
/// runtime does not create threads or sleep, which keeps scheduling decisions
/// with the eventual Windows/Flutter host integration.
#[derive(Debug)]
pub struct HostRuntime<S> {
    transport: UdpTransport,
    session: HostSession,
    packet_pump: AudioPacketPump<S>,
    receive_buffer: Vec<u8>,
}

impl<S: PcmSource> HostRuntime<S> {
    pub fn new<A: ToSocketAddrs>(
        bind_addr: A,
        session_id: SessionId,
        token: SessionToken,
        max_listeners: usize,
        source: S,
        capture_start_micros: u64,
    ) -> Result<Self, HostRuntimeError> {
        let transport = UdpTransport::bind(bind_addr)?;
        let session = HostSession::new(session_id, token, max_listeners);
        let packet_pump = AudioPacketPump::new(source, session_id, capture_start_micros)?;
        Ok(Self {
            transport,
            session,
            packet_pump,
            receive_buffer: vec![0_u8; 65_535],
        })
    }

    pub fn local_addr(&self) -> Result<SocketAddr, HostRuntimeError> {
        Ok(self.transport.local_addr()?)
    }

    pub fn set_read_timeout(
        &self,
        timeout: Option<std::time::Duration>,
    ) -> Result<(), HostRuntimeError> {
        Ok(self.transport.set_read_timeout(timeout)?)
    }

    pub fn session(&self) -> &HostSession {
        &self.session
    }

    pub fn packet_pump(&self) -> &AudioPacketPump<S> {
        &self.packet_pump
    }

    pub fn packet_pump_mut(&mut self) -> &mut AudioPacketPump<S> {
        &mut self.packet_pump
    }

    /// Receives one control datagram and dispatches any resulting responses.
    pub fn receive_control_once(
        &mut self,
        now: Instant,
        host_received_micros: i64,
        host_sent_micros: i64,
    ) -> Result<Vec<SessionEvent>, HostRuntimeError> {
        let (length, source) = self.transport.recv_from(&mut self.receive_buffer)?;
        let datagram = self.receive_buffer[..length].to_vec();
        self.handle_control_datagram(
            source,
            &datagram,
            now,
            host_received_micros,
            host_sent_micros,
        )
    }

    /// Receives one control datagram and dispatches any resulting responses
    /// using real [`monotonic_micros`] timestamps for clock-sync replies.
    ///
    /// The receive timestamp is captured immediately after the datagram
    /// arrives; the send timestamp just before the response is encoded. This
    /// is the variant scheduling loops should use; the deterministic variant
    /// above stays available for tests.
    pub fn receive_control_once_realtime(
        &mut self,
        now: Instant,
    ) -> Result<Vec<SessionEvent>, HostRuntimeError> {
        let (length, source) = self.transport.recv_from(&mut self.receive_buffer)?;
        let host_received_micros = monotonic_micros();
        let datagram = self.receive_buffer[..length].to_vec();
        let host_sent_micros = monotonic_micros();
        self.handle_control_datagram(
            source,
            &datagram,
            now,
            host_received_micros,
            host_sent_micros,
        )
    }

    /// Handles one already-received control datagram and sends all session
    /// responses through the host socket.
    pub fn handle_control_datagram(
        &mut self,
        source: SocketAddr,
        datagram: &[u8],
        now: Instant,
        host_received_micros: i64,
        host_sent_micros: i64,
    ) -> Result<Vec<SessionEvent>, HostRuntimeError> {
        let message = ControlMessage::decode(datagram)?;
        let clock_request = match &message {
            ControlMessage::ClockSyncRequest {
                client_sent_micros, ..
            } => Some(*client_sent_micros),
            _ => None,
        };
        let mut actions = self.session.handle_message(source, message, now);
        if let Some(client_sent_micros) = clock_request {
            actions.extend(self.session.respond_to_clock_sync(
                source,
                client_sent_micros,
                host_received_micros,
                host_sent_micros,
                now,
            )?);
        }
        self.dispatch_actions(actions)
    }

    /// Approves a pending listener and sends the approval control message.
    pub fn approve_listener(
        &mut self,
        listener_id: ListenerId,
    ) -> Result<Vec<SessionEvent>, HostRuntimeError> {
        let actions = self.session.approve_listener(listener_id)?;
        self.dispatch_actions(actions)
    }

    /// Removes a listener and sends the removal control message.
    pub fn remove_listener(
        &mut self,
        listener_id: ListenerId,
    ) -> Result<Vec<SessionEvent>, HostRuntimeError> {
        let actions = self.session.remove_listener(listener_id)?;
        self.dispatch_actions(actions)
    }

    /// Expires listeners whose heartbeats are older than `timeout`.
    pub fn expire_stale(
        &mut self,
        now: Instant,
        timeout: std::time::Duration,
    ) -> Result<Vec<SessionEvent>, HostRuntimeError> {
        let actions = self.session.expire_stale(now, timeout);
        self.dispatch_actions(actions)
    }

    /// Ends the session and notifies all pending or active listeners.
    pub fn end_session(&mut self, reason: u8) -> Result<Vec<SessionEvent>, HostRuntimeError> {
        let actions = self.session.end_session(reason);
        self.dispatch_actions(actions)
    }

    /// Captures and sends one audio frame to every currently active listener.
    ///
    /// If there are no active listeners, no source frame is consumed. This
    /// keeps the first packet timestamp aligned with the start of actual
    /// approved playback rather than silently advancing during setup.
    pub fn send_next_audio(&mut self) -> Result<AudioFanoutReport, HostRuntimeError> {
        let destinations = self.session.active_listener_endpoints();
        if destinations.is_empty() {
            return Ok(AudioFanoutReport::default());
        }

        let packet = self.packet_pump.next_packet()?;
        Ok(AudioPacketFanout::new(&self.transport).send_packet(&packet, destinations)?)
    }

    fn dispatch_actions(
        &self,
        actions: Vec<SessionAction>,
    ) -> Result<Vec<SessionEvent>, HostRuntimeError> {
        let mut events = Vec::new();
        for action in actions {
            match action {
                SessionAction::Outbound {
                    destination,
                    message,
                } => {
                    let encoded = message.encode()?;
                    let written = self.transport.send_to(&encoded, destination)?;
                    if written != encoded.len() {
                        return Err(io::Error::new(
                            io::ErrorKind::WriteZero,
                            format!(
                                "control datagram was only partially sent: expected {}, wrote {written}",
                                encoded.len()
                            ),
                        )
                        .into());
                    }
                }
                SessionAction::Event(event) => events.push(event),
            }
        }
        Ok(events)
    }
}

/// Errors returned by the platform-neutral listener audio pipeline.
#[derive(Debug)]
pub enum ListenerAudioError {
    Packet(PacketError),
    Codec(CodecError),
    WrongSession {
        expected: SessionId,
        actual: SessionId,
    },
    InvalidJitterCapacity,
}

impl fmt::Display for ListenerAudioError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Packet(error) => write!(formatter, "listener audio packet failed: {error}"),
            Self::Codec(error) => write!(formatter, "listener audio decode failed: {error}"),
            Self::WrongSession { expected, actual } => write!(
                formatter,
                "listener received the wrong session: expected {expected:?}, got {actual:?}"
            ),
            Self::InvalidJitterCapacity => {
                write!(formatter, "listener jitter capacity must be > 0")
            }
        }
    }
}

impl std::error::Error for ListenerAudioError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Packet(error) => Some(error),
            Self::Codec(error) => Some(error),
            Self::WrongSession { .. } | Self::InvalidJitterCapacity => None,
        }
    }
}

impl From<PacketError> for ListenerAudioError {
    fn from(error: PacketError) -> Self {
        Self::Packet(error)
    }
}

impl From<CodecError> for ListenerAudioError {
    fn from(error: CodecError) -> Self {
        Self::Codec(error)
    }
}

/// Metrics emitted by the listener pipeline for diagnostics and UI reporting.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ListenerAudioMetrics {
    pub received_packets: u64,
    pub invalid_packets: u64,
    pub wrong_session_packets: u64,
    pub inserted_packets: u64,
    pub duplicate_packets: u64,
    pub too_late_packets: u64,
    pub capacity_drops: u64,
    pub decoded_frames: u64,
    pub concealed_frames: u64,
}

/// One frame ready for a platform audio output adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ListenerAudioFrame {
    Decoded {
        sequence: u32,
        timestamp_micros: u64,
        pcm: Vec<i16>,
    },
    Concealed {
        sequence: u32,
        pcm: Vec<i16>,
    },
}

/// Receives, reorders, decodes and conceals one AudioSync listener stream.
///
/// The caller decides when a missing packet has exceeded its playback deadline
/// by calling [`Self::conceal_next`]. This keeps wall-clock scheduling and
/// platform audio output outside the reusable pipeline.
#[derive(Debug)]
pub struct ListenerAudioPipeline {
    session_id: SessionId,
    jitter: JitterBuffer<AudioPacket>,
    decoder: OpusDecoder,
    metrics: ListenerAudioMetrics,
}

impl ListenerAudioPipeline {
    pub fn new(
        session_id: SessionId,
        config: CodecConfig,
        jitter_capacity: usize,
    ) -> Result<Self, ListenerAudioError> {
        if jitter_capacity == 0 {
            return Err(ListenerAudioError::InvalidJitterCapacity);
        }
        Ok(Self {
            session_id,
            jitter: JitterBuffer::new(jitter_capacity),
            decoder: OpusDecoder::new(config)?,
            metrics: ListenerAudioMetrics::default(),
        })
    }

    pub const fn session_id(&self) -> SessionId {
        self.session_id
    }

    pub fn metrics(&self) -> &ListenerAudioMetrics {
        &self.metrics
    }

    pub fn buffered_packets(&self) -> usize {
        self.jitter.len()
    }

    pub fn next_sequence(&self) -> Option<u32> {
        self.jitter.next_sequence()
    }

    pub fn ingest_datagram(&mut self, datagram: &[u8]) -> Result<PushResult, ListenerAudioError> {
        let packet = AudioPacket::decode(datagram).map_err(|error| {
            self.metrics.invalid_packets = self.metrics.invalid_packets.saturating_add(1);
            ListenerAudioError::Packet(error)
        })?;
        self.ingest_packet(packet)
    }

    pub fn ingest_packet(&mut self, packet: AudioPacket) -> Result<PushResult, ListenerAudioError> {
        if packet.session_id != self.session_id {
            self.metrics.wrong_session_packets =
                self.metrics.wrong_session_packets.saturating_add(1);
            return Err(ListenerAudioError::WrongSession {
                expected: self.session_id,
                actual: packet.session_id,
            });
        }

        self.metrics.received_packets = self.metrics.received_packets.saturating_add(1);
        if self.jitter.next_sequence().is_none() {
            self.jitter.start_at(packet.sequence);
        }
        let result = self.jitter.push(packet.sequence, packet);
        match result {
            PushResult::Inserted => {
                self.metrics.inserted_packets = self.metrics.inserted_packets.saturating_add(1)
            }
            PushResult::Duplicate => {
                self.metrics.duplicate_packets = self.metrics.duplicate_packets.saturating_add(1)
            }
            PushResult::TooLate => {
                self.metrics.too_late_packets = self.metrics.too_late_packets.saturating_add(1)
            }
            PushResult::CapacityDrop => {
                self.metrics.capacity_drops = self.metrics.capacity_drops.saturating_add(1)
            }
        }
        Ok(result)
    }

    pub fn render_next(&mut self) -> Result<Option<ListenerAudioFrame>, ListenerAudioError> {
        let Some(packet) = self.jitter.pop_next() else {
            return Ok(None);
        };
        let pcm = self.decoder.decode(&packet.payload)?;
        self.metrics.decoded_frames = self.metrics.decoded_frames.saturating_add(1);
        Ok(Some(ListenerAudioFrame::Decoded {
            sequence: packet.sequence,
            timestamp_micros: packet.timestamp_micros,
            pcm,
        }))
    }

    pub fn conceal_next(&mut self) -> Result<Option<ListenerAudioFrame>, ListenerAudioError> {
        let Some(sequence) = self.jitter.next_sequence() else {
            return Ok(None);
        };
        if !self.jitter.skip_missing() {
            return Ok(None);
        }
        let pcm = self.decoder.decode_loss()?;
        self.metrics.concealed_frames = self.metrics.concealed_frames.saturating_add(1);
        Ok(Some(ListenerAudioFrame::Concealed { sequence, pcm }))
    }
}

/// Errors returned by the socket-owning listener audio wrapper.
#[derive(Debug)]
pub enum ListenerRuntimeError {
    Io(io::Error),
    Audio(ListenerAudioError),
    Control(ControlMessageError),
    Session(SessionError),
    WrongPeer {
        expected: SocketAddr,
        actual: SocketAddr,
    },
    UnknownDatagram,
    UnexpectedControlDatagram,
}

impl fmt::Display for ListenerRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "listener runtime I/O failed: {error}"),
            Self::Audio(error) => write!(formatter, "listener runtime audio failed: {error}"),
            Self::Control(error) => write!(formatter, "listener control message failed: {error}"),
            Self::Session(error) => write!(formatter, "listener session operation failed: {error}"),
            Self::WrongPeer { expected, actual } => {
                write!(
                    formatter,
                    "listener received {actual}, expected host {expected}"
                )
            }
            Self::UnknownDatagram => {
                write!(formatter, "listener received an unknown datagram type")
            }
            Self::UnexpectedControlDatagram => {
                write!(
                    formatter,
                    "listener expected audio but received a control datagram"
                )
            }
        }
    }
}

impl std::error::Error for ListenerRuntimeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Audio(error) => Some(error),
            Self::Control(error) => Some(error),
            Self::Session(error) => Some(error),
            Self::WrongPeer { .. } | Self::UnknownDatagram | Self::UnexpectedControlDatagram => {
                None
            }
        }
    }
}

impl From<io::Error> for ListenerRuntimeError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<ListenerAudioError> for ListenerRuntimeError {
    fn from(error: ListenerAudioError) -> Self {
        Self::Audio(error)
    }
}

impl From<ControlMessageError> for ListenerRuntimeError {
    fn from(error: ControlMessageError) -> Self {
        Self::Control(error)
    }
}

impl From<SessionError> for ListenerRuntimeError {
    fn from(error: SessionError) -> Self {
        Self::Session(error)
    }
}

/// Result of dispatching one datagram received by a listener.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ListenerRuntimeEvent {
    Control(Vec<SessionEvent>),
    Audio(PushResult),
}

/// A UDP-driven listener audio receiver.
///
/// Control/session messages still belong to the listener session runtime. This
/// wrapper handles the audio datagram side and returns decoded PCM frames to a
/// platform output adapter.
#[derive(Debug)]
pub struct ListenerRuntime {
    transport: UdpTransport,
    session: ListenerSession,
    pipeline: ListenerAudioPipeline,
    receive_buffer: Vec<u8>,
}

impl ListenerRuntime {
    pub fn new<A: ToSocketAddrs>(
        bind_addr: A,
        session_id: SessionId,
        token: SessionToken,
        host_endpoint: SocketAddr,
        config: CodecConfig,
        jitter_capacity: usize,
    ) -> Result<Self, ListenerRuntimeError> {
        let transport = UdpTransport::bind(bind_addr)?;
        let session = ListenerSession::new(session_id, token, host_endpoint);
        let pipeline = ListenerAudioPipeline::new(session_id, config, jitter_capacity)?;
        Ok(Self {
            transport,
            session,
            pipeline,
            receive_buffer: vec![0_u8; 65_535],
        })
    }

    pub fn local_addr(&self) -> Result<SocketAddr, ListenerRuntimeError> {
        Ok(self.transport.local_addr()?)
    }

    pub fn set_read_timeout(
        &self,
        timeout: Option<std::time::Duration>,
    ) -> Result<(), ListenerRuntimeError> {
        Ok(self.transport.set_read_timeout(timeout)?)
    }

    pub fn pipeline(&self) -> &ListenerAudioPipeline {
        &self.pipeline
    }

    pub fn pipeline_mut(&mut self) -> &mut ListenerAudioPipeline {
        &mut self.pipeline
    }

    pub fn session(&self) -> &ListenerSession {
        &self.session
    }

    pub fn send_join(&self) -> Result<(), ListenerRuntimeError> {
        self.send_action(self.session.join_action())
    }

    pub fn send_heartbeat(&self) -> Result<bool, ListenerRuntimeError> {
        let Some(action) = self.session.heartbeat_action() else {
            return Ok(false);
        };
        self.send_action(action)?;
        Ok(true)
    }

    pub fn send_clock_sync(&self, client_sent_micros: i64) -> Result<bool, ListenerRuntimeError> {
        let Some(action) = self.session.clock_sync_action(client_sent_micros) else {
            return Ok(false);
        };
        self.send_action(action)?;
        Ok(true)
    }

    /// Receives one control or audio datagram from the configured host.
    pub fn receive_once(
        &mut self,
        client_received_micros: Option<i64>,
    ) -> Result<ListenerRuntimeEvent, ListenerRuntimeError> {
        let (length, source) = self.transport.recv_from(&mut self.receive_buffer)?;
        if source != self.session.host_endpoint() {
            return Err(ListenerRuntimeError::WrongPeer {
                expected: self.session.host_endpoint(),
                actual: source,
            });
        }

        let datagram = self.receive_buffer[..length].to_vec();
        if datagram.starts_with(&CONTROL_MAGIC) {
            let message = ControlMessage::decode(&datagram)?;
            let actions = self
                .session
                .handle_message(source, message, client_received_micros)?;
            return Ok(ListenerRuntimeEvent::Control(
                self.dispatch_actions(actions)?,
            ));
        }
        if datagram.starts_with(&AUDIO_PACKET_MAGIC) {
            return Ok(ListenerRuntimeEvent::Audio(
                self.pipeline.ingest_datagram(&datagram)?,
            ));
        }
        Err(ListenerRuntimeError::UnknownDatagram)
    }

    pub fn receive_audio_once(&mut self) -> Result<PushResult, ListenerRuntimeError> {
        match self.receive_once(None)? {
            ListenerRuntimeEvent::Audio(result) => Ok(result),
            ListenerRuntimeEvent::Control(_) => {
                Err(ListenerRuntimeError::UnexpectedControlDatagram)
            }
        }
    }

    pub fn render_next(&mut self) -> Result<Option<ListenerAudioFrame>, ListenerRuntimeError> {
        Ok(self.pipeline.render_next()?)
    }

    pub fn conceal_next(&mut self) -> Result<Option<ListenerAudioFrame>, ListenerRuntimeError> {
        Ok(self.pipeline.conceal_next()?)
    }

    fn send_action(&self, action: SessionAction) -> Result<(), ListenerRuntimeError> {
        let SessionAction::Outbound {
            destination,
            message,
        } = action
        else {
            return Ok(());
        };
        let encoded = message.encode()?;
        let written = self.transport.send_to(&encoded, destination)?;
        if written != encoded.len() {
            return Err(io::Error::new(
                io::ErrorKind::WriteZero,
                format!(
                    "control datagram was only partially sent: expected {}, wrote {written}",
                    encoded.len()
                ),
            )
            .into());
        }
        Ok(())
    }

    fn dispatch_actions(
        &self,
        actions: Vec<SessionAction>,
    ) -> Result<Vec<SessionEvent>, ListenerRuntimeError> {
        let mut events = Vec::new();
        for action in actions {
            match action {
                SessionAction::Outbound { .. } => self.send_action(action)?,
                SessionAction::Event(event) => events.push(event),
            }
        }
        Ok(events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use audiosync_audio::SyntheticPcmSource;
    use audiosync_codec::{CodecConfig, FRAME_PCM_SAMPLES};
    use audiosync_session::ListenerState;
    use std::time::{Duration, Instant};

    fn session_id() -> SessionId {
        [0x71; 16]
    }

    fn token() -> SessionToken {
        SessionToken::new([0x72; 16])
    }

    fn receive_control(socket: &UdpTransport) -> ControlMessage {
        let mut bytes = [0_u8; 1_024];
        let (length, _) = socket
            .recv_from(&mut bytes)
            .expect("control datagram should arrive");
        ControlMessage::decode(&bytes[..length]).expect("control datagram should decode")
    }

    #[test]
    fn host_runtime_approves_listener_syncs_clock_and_sends_audio() {
        let mut host = HostRuntime::new(
            "127.0.0.1:0",
            session_id(),
            token(),
            2,
            SyntheticPcmSource::new(220.0),
            1_000_000,
        )
        .expect("host runtime should bind and initialize");
        let listener = UdpTransport::bind("127.0.0.1:0").expect("listener should bind");
        listener
            .set_read_timeout(Some(Duration::from_millis(250)))
            .expect("listener timeout should be configured");
        let listener_endpoint = listener.local_addr().expect("listener has an address");
        let host_endpoint = host.local_addr().expect("host has an address");

        let join = ControlMessage::JoinRequest {
            session_id: session_id(),
            token: token(),
        };
        let join_bytes = join.encode().expect("join should encode");
        listener
            .send_to(&join_bytes, host_endpoint)
            .expect("join should send");
        let events = host
            .receive_control_once(Instant::now(), 10, 11)
            .expect("host should process the join");
        assert!(events.iter().any(|event| matches!(
            event,
            SessionEvent::ListenerJoined { endpoint, .. } if *endpoint == listener_endpoint
        )));
        assert!(matches!(
            receive_control(&listener),
            ControlMessage::JoinAccepted { .. }
        ));

        let listener_id = host
            .session()
            .listener_id_for(listener_endpoint)
            .expect("host should assign a listener ID");
        host.approve_listener(listener_id)
            .expect("host should approve the listener");
        assert!(matches!(
            receive_control(&listener),
            ControlMessage::ListenerApproved { listener_id: id, .. } if id == listener_id
        ));

        let clock_request = ControlMessage::ClockSyncRequest {
            session_id: session_id(),
            client_sent_micros: 100,
        };
        let clock_bytes = clock_request.encode().expect("clock request should encode");
        listener
            .send_to(&clock_bytes, host_endpoint)
            .expect("clock request should send");
        let clock_events = host
            .receive_control_once(Instant::now(), 25_005, 25_006)
            .expect("host should process the clock request");
        assert!(clock_events.iter().any(|event| matches!(
            event,
            SessionEvent::ClockSyncRequestReceived {
                endpoint,
                client_sent_micros: 100,
                ..
            } if *endpoint == listener_endpoint
        )));
        assert_eq!(
            receive_control(&listener),
            ControlMessage::ClockSyncResponse {
                session_id: session_id(),
                client_sent_micros: 100,
                host_received_micros: 25_005,
                host_sent_micros: 25_006,
            }
        );

        let report = host
            .send_next_audio()
            .expect("host should send one audio packet");
        assert_eq!(report.attempted, 1);
        assert_eq!(report.sent, 1);
        assert!(report.is_complete());

        let mut bytes = [0_u8; 65_535];
        let (length, source) = listener
            .recv_from(&mut bytes)
            .expect("audio datagram should arrive");
        assert_eq!(source, host_endpoint);
        let packet = AudioPacket::decode(&bytes[..length]).expect("audio packet should decode");
        assert_eq!(packet.session_id, session_id());
        assert_eq!(packet.sequence, 0);
        assert_eq!(packet.timestamp_micros, 1_000_000);
    }

    #[test]
    fn host_and_listener_runtimes_share_control_and_audio_on_one_socket() {
        let mut host = HostRuntime::new(
            "127.0.0.1:0",
            session_id(),
            token(),
            2,
            SyntheticPcmSource::new(220.0),
            3_000_000,
        )
        .expect("host runtime should initialize");
        let host_endpoint = host.local_addr().expect("host has an address");
        let mut listener = ListenerRuntime::new(
            "127.0.0.1:0",
            session_id(),
            token(),
            host_endpoint,
            CodecConfig::default(),
            8,
        )
        .expect("listener runtime should initialize");
        listener
            .set_read_timeout(Some(Duration::from_millis(250)))
            .expect("listener timeout should be configured");

        listener.send_join().expect("listener join should send");
        let join_events = host
            .receive_control_once(Instant::now(), 10, 11)
            .expect("host should process listener join");
        assert!(
            join_events
                .iter()
                .any(|event| matches!(event, SessionEvent::ListenerJoined { .. }))
        );
        assert!(matches!(
            listener
                .receive_once(None)
                .expect("listener should receive join acceptance"),
            ListenerRuntimeEvent::Control(events)
                if events.iter().any(|event| matches!(
                    event,
                    SessionEvent::StateChanged(ListenerState::PendingApproval)
                ))
        ));

        let listener_id = host
            .session()
            .listener_id_for(listener.local_addr().expect("listener has an address"))
            .expect("host should assign listener ID");
        host.approve_listener(listener_id)
            .expect("host should approve listener");
        assert!(matches!(
            listener
                .receive_once(None)
                .expect("listener should receive approval"),
            ListenerRuntimeEvent::Control(events)
                if events.iter().any(|event| matches!(
                    event,
                    SessionEvent::StateChanged(ListenerState::Active)
                ))
        ));

        assert!(
            listener
                .send_clock_sync(100)
                .expect("clock request should send")
        );
        host.receive_control_once(Instant::now(), 25_105, 25_106)
            .expect("host should answer clock sync");
        assert!(matches!(
            listener
                .receive_once(Some(111))
                .expect("listener should receive clock response"),
            ListenerRuntimeEvent::Control(events)
                if events.iter().any(|event| matches!(
                    event,
                    SessionEvent::ClockSyncUpdated {
                        offset_micros: 25_000,
                        round_trip_micros: 10,
                        ..
                    }
                ))
        ));

        let report = host
            .send_next_audio()
            .expect("host should send audio after approval");
        assert!(report.is_complete());
        assert!(matches!(
            listener
                .receive_once(None)
                .expect("listener should receive audio"),
            ListenerRuntimeEvent::Audio(PushResult::Inserted)
        ));
        assert!(matches!(
            listener
                .render_next()
                .expect("listener should render audio")
                .expect("audio frame should be ready"),
            ListenerAudioFrame::Decoded {
                sequence: 0,
                ref pcm,
                ..
            } if pcm.len() == FRAME_PCM_SAMPLES
        ));
    }

    #[test]
    fn listener_pipeline_reorders_and_conceals_a_missing_frame() {
        let mut pump = AudioPacketPump::new(SyntheticPcmSource::new(330.0), session_id(), 0)
            .expect("synthetic source should initialize");
        let first = pump.next_packet().expect("first packet should encode");
        let missing = pump.next_packet().expect("missing packet should encode");
        let later = pump.next_packet().expect("later packet should encode");
        let mut listener = ListenerAudioPipeline::new(session_id(), CodecConfig::default(), 4)
            .expect("listener pipeline should initialize");

        listener
            .ingest_datagram(&first.encode().expect("first packet should encode"))
            .expect("first packet should be accepted");
        listener
            .ingest_datagram(&later.encode().expect("later packet should encode"))
            .expect("later packet should be accepted");

        let decoded = listener
            .render_next()
            .expect("first packet should decode")
            .expect("first frame should be ready");
        assert!(matches!(
            decoded,
            ListenerAudioFrame::Decoded {
                sequence: 0,
                timestamp_micros: 0,
                ref pcm,
            } if pcm.len() == FRAME_PCM_SAMPLES
        ));

        let concealed = listener
            .conceal_next()
            .expect("missing frame should conceal")
            .expect("concealed frame should be ready");
        assert!(matches!(
            concealed,
            ListenerAudioFrame::Concealed {
                sequence: 1,
                ref pcm,
            } if pcm.len() == FRAME_PCM_SAMPLES
        ));

        let decoded_later = listener
            .render_next()
            .expect("later packet should decode")
            .expect("later frame should be ready");
        assert!(matches!(
            decoded_later,
            ListenerAudioFrame::Decoded {
                sequence: 2,
                ref pcm,
                ..
            } if pcm.len() == FRAME_PCM_SAMPLES
        ));
        assert_eq!(listener.metrics().received_packets, 2);
        assert_eq!(listener.metrics().inserted_packets, 2);
        assert_eq!(listener.metrics().decoded_frames, 2);
        assert_eq!(listener.metrics().concealed_frames, 1);
        assert_eq!(listener.next_sequence(), Some(3));
        assert_eq!(missing.sequence, 1);
    }

    #[test]
    fn listener_pipeline_rejects_wrong_sessions_and_invalid_capacity() {
        assert!(matches!(
            ListenerAudioPipeline::new(session_id(), CodecConfig::default(), 0),
            Err(ListenerAudioError::InvalidJitterCapacity)
        ));

        let mut listener = ListenerAudioPipeline::new(session_id(), CodecConfig::default(), 2)
            .expect("listener pipeline should initialize");
        let wrong = AudioPacket::new([0xFF; 16], 0, 0, vec![1, 2, 3]);
        let error = listener
            .ingest_packet(wrong)
            .expect_err("wrong session must be rejected");
        assert!(matches!(error, ListenerAudioError::WrongSession { .. }));
        assert_eq!(listener.metrics().wrong_session_packets, 1);
    }

    #[test]
    fn listener_runtime_receives_and_decodes_a_udp_audio_packet() {
        let session_id = session_id();
        let sender = UdpTransport::bind("127.0.0.1:0").expect("sender should bind");
        let mut listener = ListenerRuntime::new(
            "127.0.0.1:0",
            session_id,
            token(),
            sender.local_addr().expect("sender has an address"),
            CodecConfig::default(),
            4,
        )
        .expect("listener runtime should bind");
        listener
            .set_read_timeout(Some(Duration::from_millis(250)))
            .expect("listener timeout should be configured");
        let mut pump = AudioPacketPump::new(SyntheticPcmSource::new(440.0), session_id, 0)
            .expect("packet pump should initialize");
        let packet = pump.next_packet().expect("packet should encode");
        let encoded = packet.encode().expect("packet should serialize");
        sender
            .send_to(
                &encoded,
                listener.local_addr().expect("listener has an address"),
            )
            .expect("audio packet should send");

        assert_eq!(
            listener
                .receive_audio_once()
                .expect("listener should receive audio"),
            PushResult::Inserted
        );
        let frame = listener
            .render_next()
            .expect("listener should decode audio")
            .expect("decoded frame should be available");
        assert!(matches!(
            frame,
            ListenerAudioFrame::Decoded {
                sequence: 0,
                ref pcm,
                ..
            } if pcm.len() == FRAME_PCM_SAMPLES
        ));
    }

    #[test]
    fn local_host_to_listener_audio_path_survives_500_packets() {
        let session_id = session_id();
        let sender = UdpTransport::bind("127.0.0.1:0").expect("host audio socket should bind");
        let mut listener = ListenerRuntime::new(
            "127.0.0.1:0",
            session_id,
            token(),
            sender.local_addr().expect("sender has an address"),
            CodecConfig::default(),
            8,
        )
        .expect("listener runtime should bind");
        listener
            .set_read_timeout(Some(Duration::from_millis(250)))
            .expect("listener timeout should be configured");
        let destination = listener.local_addr().expect("listener has an address");
        let fanout = AudioPacketFanout::new(&sender);
        let mut pump = AudioPacketPump::new(SyntheticPcmSource::new(220.0), session_id, 2_000_000)
            .expect("packet pump should initialize");

        for expected_sequence in 0..500_u32 {
            let packet = pump.next_packet().expect("synthetic packet should encode");
            assert_eq!(packet.sequence, expected_sequence);
            let report = fanout
                .send_packet(&packet, [destination])
                .expect("audio packet should send");
            assert!(report.is_complete());
            assert_eq!(
                listener
                    .receive_audio_once()
                    .expect("listener should receive audio"),
                PushResult::Inserted
            );
            let frame = listener
                .render_next()
                .expect("listener should decode audio")
                .expect("decoded frame should be ready");
            assert!(matches!(
                frame,
                ListenerAudioFrame::Decoded {
                    sequence,
                    ref pcm,
                    ..
                } if sequence == expected_sequence && pcm.len() == FRAME_PCM_SAMPLES
            ));
        }

        assert_eq!(listener.pipeline().metrics().received_packets, 500);
        assert_eq!(listener.pipeline().metrics().decoded_frames, 500);
        assert_eq!(listener.pipeline().metrics().concealed_frames, 0);
    }
}
