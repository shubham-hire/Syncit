//! PCM sources and the host-side PCM-to-Opus pipeline.
//!
//! The source contract is deliberately expressed in the codec's fixed format:
//! 48 kHz, mono, signed 16-bit PCM, one 20 ms frame at a time. Platform
//! adapters are responsible for converting their native sample format before
//! a frame reaches [`OpusEncoderPipeline`].

mod wasapi_capture;

use audiosync_codec::{CodecConfig, CodecError, OpusEncoder};
use audiosync_core::{AudioPacket, SessionId};
use std::fmt;

pub use wasapi_capture::{WasapiCapture, WasapiCaptureConfig};

/// A source that supplies one fixed-size PCM frame on demand.
pub trait PcmSource {
    /// Returns the PCM format produced by this source.
    fn config(&self) -> CodecConfig;

    /// Fills `pcm` with exactly one frame.
    fn read_frame(&mut self, pcm: &mut [i16]) -> Result<(), AudioSourceError>;
}

/// Boxing lets callers pick between sources (synthetic vs. a platform
/// capture) at runtime while keeping one concrete `S` for
/// `HostRuntime`/`spawn_host_loop`.
impl<S: PcmSource + ?Sized> PcmSource for Box<S> {
    fn config(&self) -> CodecConfig {
        (**self).config()
    }

    fn read_frame(&mut self, pcm: &mut [i16]) -> Result<(), AudioSourceError> {
        (**self).read_frame(pcm)
    }
}

/// Errors returned while opening or reading a PCM source.
#[derive(Debug)]
pub enum AudioSourceError {
    InvalidFrameBuffer { expected: usize, actual: usize },
    InvalidConfiguration { message: String },
    Capture { message: String },
    UnsupportedPlatform { required: &'static str },
}

impl fmt::Display for AudioSourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFrameBuffer { expected, actual } => write!(
                formatter,
                "invalid PCM frame buffer: expected {expected} samples, got {actual}"
            ),
            Self::InvalidConfiguration { message } => {
                write!(formatter, "invalid PCM source configuration: {message}")
            }
            Self::Capture { message } => write!(formatter, "PCM capture failed: {message}"),
            Self::UnsupportedPlatform { required } => {
                write!(formatter, "this PCM source requires {required}")
            }
        }
    }
}

impl std::error::Error for AudioSourceError {}

/// Errors returned by the source-to-Opus pipeline.
#[derive(Debug)]
pub enum AudioPipelineError {
    Source(AudioSourceError),
    Codec(CodecError),
}

impl fmt::Display for AudioPipelineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(error) => write!(formatter, "PCM source error: {error}"),
            Self::Codec(error) => write!(formatter, "Opus encoder error: {error}"),
        }
    }
}

impl std::error::Error for AudioPipelineError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(error) => Some(error),
            Self::Codec(error) => Some(error),
        }
    }
}

impl From<AudioSourceError> for AudioPipelineError {
    fn from(error: AudioSourceError) -> Self {
        Self::Source(error)
    }
}

impl From<CodecError> for AudioPipelineError {
    fn from(error: CodecError) -> Self {
        Self::Codec(error)
    }
}

/// Owns a PCM source and a stateful Opus encoder for one audio stream.
#[derive(Debug)]
pub struct OpusEncoderPipeline<S> {
    source: S,
    encoder: OpusEncoder,
    pcm_frame: Vec<i16>,
}

impl<S: PcmSource> OpusEncoderPipeline<S> {
    /// Creates an encoder using the format advertised by `source`.
    pub fn new(source: S) -> Result<Self, AudioPipelineError> {
        let config = source.config();
        let encoder = OpusEncoder::new(config)?;
        Ok(Self {
            source,
            encoder,
            pcm_frame: vec![0_i16; config.pcm_samples_per_frame()],
        })
    }

    pub const fn config(&self) -> CodecConfig {
        self.encoder.config()
    }

    /// Reads one PCM frame and returns its encoded Opus packet.
    pub fn next_packet(&mut self) -> Result<Vec<u8>, AudioPipelineError> {
        self.source.read_frame(&mut self.pcm_frame)?;
        Ok(self.encoder.encode(&self.pcm_frame)?)
    }

    pub const fn source(&self) -> &S {
        &self.source
    }

    pub fn source_mut(&mut self) -> &mut S {
        &mut self.source
    }

    pub fn into_source(self) -> S {
        self.source
    }
}

/// Errors returned while turning encoded source frames into timestamped audio
/// packets.
#[derive(Debug)]
pub enum AudioPacketPumpError {
    Pipeline(AudioPipelineError),
    TimestampOverflow {
        capture_start_micros: u64,
        frame_index: u64,
        frame_duration_micros: u64,
    },
    FrameIndexOverflow,
}

impl fmt::Display for AudioPacketPumpError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pipeline(error) => write!(formatter, "audio packet pump failed: {error}"),
            Self::TimestampOverflow {
                capture_start_micros,
                frame_index,
                frame_duration_micros,
            } => write!(
                formatter,
                "audio packet timestamp overflow: start {capture_start_micros} µs, frame {frame_index}, duration {frame_duration_micros} µs"
            ),
            Self::FrameIndexOverflow => write!(formatter, "audio packet frame index overflowed"),
        }
    }
}

impl std::error::Error for AudioPacketPumpError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Pipeline(error) => Some(error),
            Self::TimestampOverflow { .. } | Self::FrameIndexOverflow => None,
        }
    }
}

impl From<AudioPipelineError> for AudioPacketPumpError {
    fn from(error: AudioPipelineError) -> Self {
        Self::Pipeline(error)
    }
}

/// Converts one fixed-size PCM source into a timestamped [`AudioPacket`]
/// stream.
///
/// The pump owns one stateful encoder for the lifetime of the stream. Sequence
/// numbers advance once a frame has been captured and encoded successfully;
/// timestamps use `capture_start_micros + frame_index * frame_duration` so all
/// listeners receive the same stream timeline.
#[derive(Debug)]
pub struct AudioPacketPump<S> {
    pipeline: OpusEncoderPipeline<S>,
    session_id: SessionId,
    token: [u8; 16],
    capture_start_micros: u64,
    frame_duration_micros: u64,
    next_sequence: u32,
    frame_index: u64,
}

impl<S: PcmSource> AudioPacketPump<S> {
    /// Creates a packet pump from a PCM source, the session identity, and the
    /// per-session token used to authenticate every outgoing packet. The
    /// timestamp is an initial seed; callers can rebase it before the first
    /// frame with [`Self::start_at_micros`].
    pub fn new(
        source: S,
        session_id: SessionId,
        token: [u8; 16],
        capture_start_micros: u64,
    ) -> Result<Self, AudioPacketPumpError> {
        let pipeline = OpusEncoderPipeline::new(source)?;
        Ok(Self::from_pipeline(
            pipeline,
            session_id,
            token,
            capture_start_micros,
        ))
    }

    /// Creates a packet pump around an already initialized encoder pipeline.
    pub fn from_pipeline(
        pipeline: OpusEncoderPipeline<S>,
        session_id: SessionId,
        token: [u8; 16],
        capture_start_micros: u64,
    ) -> Self {
        let frame_duration_micros =
            u64::from(pipeline.config().frame_duration_millis).saturating_mul(1_000);
        Self {
            pipeline,
            session_id,
            token,
            capture_start_micros,
            frame_duration_micros,
            next_sequence: 0,
            frame_index: 0,
        }
    }

    pub const fn config(&self) -> CodecConfig {
        self.pipeline.config()
    }

    pub const fn session_id(&self) -> SessionId {
        self.session_id
    }

    pub const fn next_sequence(&self) -> u32 {
        self.next_sequence
    }

    pub const fn frame_index(&self) -> u64 {
        self.frame_index
    }

    pub const fn capture_start_micros(&self) -> u64 {
        self.capture_start_micros
    }

    /// Rebases the first packet timestamp before the stream starts.
    ///
    /// A host can be created well before its first listener is approved. In
    /// that case the constructor's seed would make the first live packet look
    /// stale by the entire setup duration. Once a frame has been produced, the
    /// timestamp timeline is fixed and this method intentionally does nothing.
    pub fn start_at_micros(&mut self, capture_start_micros: u64) {
        if self.frame_index == 0 {
            self.capture_start_micros = capture_start_micros;
        }
    }

    pub const fn source(&self) -> &S {
        self.pipeline.source()
    }

    pub fn source_mut(&mut self) -> &mut S {
        self.pipeline.source_mut()
    }

    /// Captures, encodes, and timestamps one audio frame.
    pub fn next_packet(&mut self) -> Result<AudioPacket, AudioPacketPumpError> {
        let frame_offset = self
            .frame_index
            .checked_mul(self.frame_duration_micros)
            .ok_or(AudioPacketPumpError::TimestampOverflow {
                capture_start_micros: self.capture_start_micros,
                frame_index: self.frame_index,
                frame_duration_micros: self.frame_duration_micros,
            })?;
        let timestamp_micros = self.capture_start_micros.checked_add(frame_offset).ok_or(
            AudioPacketPumpError::TimestampOverflow {
                capture_start_micros: self.capture_start_micros,
                frame_index: self.frame_index,
                frame_duration_micros: self.frame_duration_micros,
            },
        )?;
        let next_frame_index = self
            .frame_index
            .checked_add(1)
            .ok_or(AudioPacketPumpError::FrameIndexOverflow)?;
        let payload = self.pipeline.next_packet()?;
        let mut packet = AudioPacket::new(
            self.session_id,
            self.next_sequence,
            timestamp_micros,
            payload,
        );
        packet.sign(&self.token);
        self.next_sequence = self.next_sequence.wrapping_add(1);
        self.frame_index = next_frame_index;
        Ok(packet)
    }
}

/// Deterministic mono PCM source used by the offline simulator and regression
/// tests. Frames are contiguous in phase, so a caller can use it as a stand-in
/// for a real capture stream without changing the encoder path.
#[derive(Clone, Debug)]
pub struct SyntheticPcmSource {
    config: CodecConfig,
    frequency_hz: f32,
    amplitude: f32,
    sample_cursor: u64,
}

impl SyntheticPcmSource {
    pub fn new(frequency_hz: f32) -> Self {
        Self {
            config: CodecConfig::default(),
            frequency_hz,
            amplitude: 12_000.0,
            sample_cursor: 0,
        }
    }

    pub fn with_config(
        frequency_hz: f32,
        amplitude: f32,
        config: CodecConfig,
    ) -> Result<Self, AudioSourceError> {
        config
            .validate()
            .map_err(|error| AudioSourceError::InvalidConfiguration {
                message: error.to_string(),
            })?;
        if !frequency_hz.is_finite() || frequency_hz < 0.0 {
            return Err(AudioSourceError::InvalidConfiguration {
                message: format!("frequency must be finite and non-negative, got {frequency_hz}"),
            });
        }
        if !amplitude.is_finite() || amplitude < 0.0 || amplitude > f32::from(i16::MAX) {
            return Err(AudioSourceError::InvalidConfiguration {
                message: format!(
                    "amplitude must be finite and between 0 and {}, got {amplitude}",
                    i16::MAX
                ),
            });
        }
        Ok(Self {
            config,
            frequency_hz,
            amplitude,
            sample_cursor: 0,
        })
    }

    pub const fn frequency_hz(&self) -> f32 {
        self.frequency_hz
    }

    pub const fn sample_cursor(&self) -> u64 {
        self.sample_cursor
    }
}

impl PcmSource for SyntheticPcmSource {
    fn config(&self) -> CodecConfig {
        self.config
    }

    fn read_frame(&mut self, pcm: &mut [i16]) -> Result<(), AudioSourceError> {
        let expected = self.config.pcm_samples_per_frame();
        if pcm.len() != expected {
            return Err(AudioSourceError::InvalidFrameBuffer {
                expected,
                actual: pcm.len(),
            });
        }

        let sample_rate = f64::from(self.config.sample_rate_hz);
        let frequency = f64::from(self.frequency_hz);
        let amplitude = f64::from(self.amplitude);
        for (offset, sample) in pcm.iter_mut().enumerate() {
            let sample_index = self.sample_cursor.saturating_add(offset as u64);
            let phase = sample_index as f64 * frequency * 2.0 * std::f64::consts::PI / sample_rate;
            *sample = (phase.sin() * amplitude).round() as i16;
        }
        self.sample_cursor = self.sample_cursor.saturating_add(pcm.len() as u64);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use audiosync_codec::{FRAME_PCM_SAMPLES, OpusDecoder};

    #[test]
    fn synthetic_source_produces_contiguous_non_silent_frames() {
        let mut source = SyntheticPcmSource::new(440.0);
        let mut first = vec![0_i16; FRAME_PCM_SAMPLES];
        let mut second = vec![0_i16; FRAME_PCM_SAMPLES];

        source
            .read_frame(&mut first)
            .expect("first synthetic frame should be available");
        source
            .read_frame(&mut second)
            .expect("second synthetic frame should be available");

        assert!(first.iter().any(|sample| *sample != 0));
        assert!(second.iter().any(|sample| *sample != 0));
        assert_eq!(source.sample_cursor(), (FRAME_PCM_SAMPLES * 2) as u64);
        assert_ne!(first, second);
    }

    #[test]
    fn boxed_source_delegates_to_the_inner_source() {
        let mut source: Box<dyn PcmSource + Send> = Box::new(SyntheticPcmSource::new(220.0));
        assert_eq!(source.config().sample_rate_hz, 48_000);

        let mut frame = vec![0_i16; FRAME_PCM_SAMPLES];
        source
            .read_frame(&mut frame)
            .expect("boxed source should read through to the inner source");
        assert!(frame.iter().any(|sample| *sample != 0));
    }

    #[test]
    fn encoder_pipeline_connects_synthetic_pcm_to_opus() {
        let source = SyntheticPcmSource::new(220.0);
        let mut pipeline =
            OpusEncoderPipeline::new(source).expect("synthetic source should initialize");
        let mut decoder = OpusDecoder::new(pipeline.config()).expect("decoder should initialize");

        let packet = pipeline.next_packet().expect("synthetic PCM should encode");
        let decoded = decoder
            .decode(&packet)
            .expect("encoded frame should decode");

        assert!(!packet.is_empty());
        assert_eq!(decoded.len(), FRAME_PCM_SAMPLES);
        assert!(decoded.iter().any(|sample| sample.abs() > 100));
    }

    #[test]
    fn packet_pump_assigns_contiguous_sequence_and_timestamp_values() {
        let session_id = [0x42; 16];
        let mut pump = AudioPacketPump::new(
            SyntheticPcmSource::new(220.0),
            session_id,
            [0x5A; 16],
            1_000_000,
        )
        .expect("synthetic source should initialize");

        let first = pump.next_packet().expect("first packet should encode");
        let second = pump.next_packet().expect("second packet should encode");

        assert_eq!(first.session_id, session_id);
        assert_eq!(second.session_id, session_id);
        assert_eq!(first.sequence, 0);
        assert_eq!(second.sequence, 1);
        assert_eq!(first.timestamp_micros, 1_000_000);
        assert_eq!(second.timestamp_micros, 1_020_000);
        assert!(!first.payload.is_empty());
        assert!(!second.payload.is_empty());
        assert_eq!(pump.next_sequence(), 2);
        assert_eq!(pump.frame_index(), 2);
    }

    #[test]
    fn packet_pump_rebases_only_before_the_first_frame() {
        let mut pump =
            AudioPacketPump::new(SyntheticPcmSource::new(220.0), [0x31; 16], [0x41; 16], 1)
                .expect("synthetic source should initialize");

        pump.start_at_micros(10_000);
        assert_eq!(pump.capture_start_micros(), 10_000);

        let first = pump.next_packet().expect("first packet should encode");
        pump.start_at_micros(20_000);

        assert_eq!(first.timestamp_micros, 10_000);
        assert_eq!(pump.capture_start_micros(), 10_000);
    }

    #[test]
    fn packet_pump_reports_timestamp_overflow_before_reading_another_frame() {
        let mut pump = AudioPacketPump::new(
            SyntheticPcmSource::new(220.0),
            [0x11; 16],
            [0x22; 16],
            u64::MAX,
        )
        .expect("synthetic source should initialize");

        pump.next_packet()
            .expect("the first frame can use the maximum timestamp");
        let error = pump
            .next_packet()
            .expect_err("the next timestamp must overflow");

        assert!(matches!(
            error,
            AudioPacketPumpError::TimestampOverflow { frame_index: 1, .. }
        ));
        assert_eq!(pump.frame_index(), 1);
    }

    #[test]
    fn source_rejects_a_non_frame_sized_destination() {
        let mut source = SyntheticPcmSource::new(220.0);
        let error = source
            .read_frame(&mut [0_i16; FRAME_PCM_SAMPLES - 1])
            .expect_err("a partial frame must be rejected");

        assert!(matches!(
            error,
            AudioSourceError::InvalidFrameBuffer {
                expected: FRAME_PCM_SAMPLES,
                actual,
            } if actual == FRAME_PCM_SAMPLES - 1
        ));
    }

    #[test]
    fn wasapi_capture_rejects_zero_timeouts_before_opening_a_device() {
        let error = WasapiCapture::new(WasapiCaptureConfig {
            buffer_duration_millis: 0,
            ..WasapiCaptureConfig::default()
        })
        .expect_err("zero-duration capture should be rejected");

        assert!(matches!(
            error,
            AudioSourceError::InvalidConfiguration { .. }
        ));
    }

    #[cfg(not(windows))]
    #[test]
    fn wasapi_capture_reports_its_platform_requirement() {
        let error = WasapiCapture::default_loopback()
            .expect_err("WASAPI must not initialize on a non-Windows test host");

        assert!(matches!(
            error,
            AudioSourceError::UnsupportedPlatform {
                required: "Windows WASAPI"
            }
        ));
    }
}
