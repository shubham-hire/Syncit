//! PCM sources and the host-side PCM-to-Opus pipeline.
//!
//! The source contract is deliberately expressed in the codec's fixed format:
//! 48 kHz, mono, signed 16-bit PCM, one 20 ms frame at a time. Platform
//! adapters are responsible for converting their native sample format before
//! a frame reaches [`OpusEncoderPipeline`].

mod wasapi_capture;

use audiosync_codec::{CodecConfig, CodecError, OpusEncoder};
use std::fmt;

pub use wasapi_capture::{WasapiCapture, WasapiCaptureConfig};

/// A source that supplies one fixed-size PCM frame on demand.
pub trait PcmSource {
    /// Returns the PCM format produced by this source.
    fn config(&self) -> CodecConfig;

    /// Fills `pcm` with exactly one frame.
    fn read_frame(&mut self, pcm: &mut [i16]) -> Result<(), AudioSourceError>;
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
