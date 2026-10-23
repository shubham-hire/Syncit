//! Opus frame encoding and decoding for the SyncIt audio pipeline.
//!
//! The first codec milestone intentionally fixes one interoperable format:
//! 48 kHz mono PCM, encoded as one 20 ms Opus frame per packet. Keeping the
//! frame contract explicit lets the transport and jitter-buffer layers reason
//! about timing without depending on platform audio APIs.

use opus::{Application, Bitrate, Channels, Decoder, Encoder};
use std::fmt;

/// The sample rate used by the first SyncIt audio format.
pub const SAMPLE_RATE_HZ: u32 = 48_000;
/// The number of PCM channels in the first audio format.
pub const CHANNELS: usize = 1;
/// The duration of one encoded frame.
pub const FRAME_DURATION_MILLIS: u32 = 20;
/// The number of samples in one frame, per channel.
pub const FRAME_SAMPLES_PER_CHANNEL: usize =
    SAMPLE_RATE_HZ as usize * FRAME_DURATION_MILLIS as usize / 1_000;
/// The number of interleaved PCM samples expected by the encoder.
pub const FRAME_PCM_SAMPLES: usize = FRAME_SAMPLES_PER_CHANNEL * CHANNELS;
/// The maximum buffer reserved for one encoded frame.
pub const MAX_ENCODED_PACKET_BYTES: usize = 4_000;
/// The initial target bitrate for one listener stream.
pub const TARGET_BITRATE_BITS_PER_SECOND: i32 = 96_000;

/// The fixed PCM/Opus contract used by the first backend milestone.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CodecConfig {
    pub sample_rate_hz: u32,
    pub channels: usize,
    pub frame_duration_millis: u32,
}

impl CodecConfig {
    pub const fn pcm_samples_per_frame(self) -> usize {
        self.sample_rate_hz as usize * self.frame_duration_millis as usize / 1_000 * self.channels
    }

    pub const fn frame_samples_per_channel(self) -> usize {
        self.sample_rate_hz as usize * self.frame_duration_millis as usize / 1_000
    }

    pub fn validate(self) -> Result<(), CodecError> {
        if self.sample_rate_hz != SAMPLE_RATE_HZ {
            return Err(CodecError::UnsupportedConfig {
                field: "sample rate",
                expected: SAMPLE_RATE_HZ as usize,
                actual: self.sample_rate_hz as usize,
            });
        }
        if self.channels != CHANNELS {
            return Err(CodecError::UnsupportedConfig {
                field: "channels",
                expected: CHANNELS,
                actual: self.channels,
            });
        }
        if self.frame_duration_millis != FRAME_DURATION_MILLIS {
            return Err(CodecError::UnsupportedConfig {
                field: "frame duration in milliseconds",
                expected: FRAME_DURATION_MILLIS as usize,
                actual: self.frame_duration_millis as usize,
            });
        }
        Ok(())
    }
}

impl Default for CodecConfig {
    fn default() -> Self {
        Self {
            sample_rate_hz: SAMPLE_RATE_HZ,
            channels: CHANNELS,
            frame_duration_millis: FRAME_DURATION_MILLIS,
        }
    }
}

/// The single supported codec configuration for this milestone.
pub const DEFAULT_CODEC_CONFIG: CodecConfig = CodecConfig {
    sample_rate_hz: SAMPLE_RATE_HZ,
    channels: CHANNELS,
    frame_duration_millis: FRAME_DURATION_MILLIS,
};

/// Errors returned by the codec boundary.
#[derive(Debug)]
pub enum CodecError {
    UnsupportedConfig {
        field: &'static str,
        expected: usize,
        actual: usize,
    },
    InvalidPcmLength {
        expected: usize,
        actual: usize,
    },
    EmptyPacket,
    EncodedPacketTooLarge {
        max: usize,
        actual: usize,
    },
    UnexpectedDecodedSamples {
        expected: usize,
        actual: usize,
    },
    Opus(opus::Error),
}

impl fmt::Display for CodecError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedConfig {
                field,
                expected,
                actual,
            } => write!(
                formatter,
                "unsupported codec {field}: expected {expected}, got {actual}"
            ),
            Self::InvalidPcmLength { expected, actual } => write!(
                formatter,
                "invalid PCM frame length: expected {expected} samples, got {actual}"
            ),
            Self::EmptyPacket => write!(formatter, "an encoded Opus packet cannot be empty"),
            Self::EncodedPacketTooLarge { max, actual } => write!(
                formatter,
                "encoded Opus packet is too large: maximum {max} bytes, got {actual}"
            ),
            Self::UnexpectedDecodedSamples { expected, actual } => write!(
                formatter,
                "unexpected decoded frame length: expected {expected} samples per channel, got {actual}"
            ),
            Self::Opus(error) => write!(formatter, "Opus error: {error}"),
        }
    }
}

impl std::error::Error for CodecError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Opus(error) => Some(error),
            _ => None,
        }
    }
}

impl From<opus::Error> for CodecError {
    fn from(error: opus::Error) -> Self {
        Self::Opus(error)
    }
}

/// Stateful Opus encoder for fixed-size PCM frames.
#[derive(Debug)]
pub struct OpusEncoder {
    config: CodecConfig,
    encoder: Encoder,
}

impl OpusEncoder {
    pub fn new(config: CodecConfig) -> Result<Self, CodecError> {
        config.validate()?;
        let mut encoder = Encoder::new(config.sample_rate_hz, Channels::Mono, Application::Audio)?;
        encoder.set_bitrate(Bitrate::Bits(TARGET_BITRATE_BITS_PER_SECOND))?;
        Ok(Self { config, encoder })
    }

    pub const fn config(&self) -> CodecConfig {
        self.config
    }

    /// Encodes exactly one 20 ms PCM frame.
    pub fn encode(&mut self, pcm: &[i16]) -> Result<Vec<u8>, CodecError> {
        let expected = self.config.pcm_samples_per_frame();
        if pcm.len() != expected {
            return Err(CodecError::InvalidPcmLength {
                expected,
                actual: pcm.len(),
            });
        }

        let packet = self.encoder.encode_vec(pcm, MAX_ENCODED_PACKET_BYTES)?;
        if packet.is_empty() {
            return Err(CodecError::EmptyPacket);
        }
        if packet.len() > MAX_ENCODED_PACKET_BYTES {
            return Err(CodecError::EncodedPacketTooLarge {
                max: MAX_ENCODED_PACKET_BYTES,
                actual: packet.len(),
            });
        }
        Ok(packet)
    }
}

/// Stateful Opus decoder for fixed-size PCM frames.
#[derive(Debug)]
pub struct OpusDecoder {
    config: CodecConfig,
    decoder: Decoder,
}

impl OpusDecoder {
    pub fn new(config: CodecConfig) -> Result<Self, CodecError> {
        config.validate()?;
        Ok(Self {
            config,
            decoder: Decoder::new(config.sample_rate_hz, Channels::Mono)?,
        })
    }

    pub const fn config(&self) -> CodecConfig {
        self.config
    }

    /// Decodes one valid Opus packet into one fixed-size PCM frame.
    pub fn decode(&mut self, packet: &[u8]) -> Result<Vec<i16>, CodecError> {
        if packet.is_empty() {
            return Err(CodecError::EmptyPacket);
        }
        self.decode_inner(packet)
    }

    /// Produces one packet-loss-concealed PCM frame from the decoder state.
    pub fn decode_loss(&mut self) -> Result<Vec<i16>, CodecError> {
        self.decode_inner(&[])
    }

    fn decode_inner(&mut self, packet: &[u8]) -> Result<Vec<i16>, CodecError> {
        let mut pcm = vec![0_i16; self.config.pcm_samples_per_frame()];
        let decoded_samples_per_channel = self.decoder.decode(packet, &mut pcm, false)?;
        let expected = self.config.frame_samples_per_channel();
        if decoded_samples_per_channel != expected {
            return Err(CodecError::UnexpectedDecodedSamples {
                expected,
                actual: decoded_samples_per_channel,
            });
        }
        pcm.truncate(decoded_samples_per_channel * self.config.channels);
        Ok(pcm)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine_frame(frequency_hz: f32) -> Vec<i16> {
        (0..FRAME_PCM_SAMPLES)
            .map(|sample| {
                let phase = sample as f32 * frequency_hz * 2.0 * std::f32::consts::PI
                    / SAMPLE_RATE_HZ as f32;
                (phase.sin() * 12_000.0) as i16
            })
            .collect()
    }

    #[test]
    fn default_config_is_48_khz_mono_20_ms() {
        assert_eq!(DEFAULT_CODEC_CONFIG, CodecConfig::default());
        assert_eq!(DEFAULT_CODEC_CONFIG.sample_rate_hz, 48_000);
        assert_eq!(DEFAULT_CODEC_CONFIG.channels, 1);
        assert_eq!(DEFAULT_CODEC_CONFIG.frame_duration_millis, 20);
        assert_eq!(DEFAULT_CODEC_CONFIG.frame_samples_per_channel(), 960);
        assert_eq!(DEFAULT_CODEC_CONFIG.pcm_samples_per_frame(), 960);
    }

    #[test]
    fn encoder_rejects_unsupported_configuration_and_frame_lengths() {
        let unsupported = CodecConfig {
            channels: 2,
            ..DEFAULT_CODEC_CONFIG
        };
        assert!(matches!(
            OpusEncoder::new(unsupported),
            Err(CodecError::UnsupportedConfig {
                field: "channels",
                ..
            })
        ));

        let mut encoder =
            OpusEncoder::new(DEFAULT_CODEC_CONFIG).expect("encoder should initialize");
        assert!(matches!(
            encoder.encode(&[0_i16; FRAME_PCM_SAMPLES - 1]),
            Err(CodecError::InvalidPcmLength {
                expected: FRAME_PCM_SAMPLES,
                actual,
            }) if actual == FRAME_PCM_SAMPLES - 1
        ));
    }

    #[test]
    fn silence_round_trips_through_opus() {
        let mut encoder =
            OpusEncoder::new(DEFAULT_CODEC_CONFIG).expect("encoder should initialize");
        let mut decoder =
            OpusDecoder::new(DEFAULT_CODEC_CONFIG).expect("decoder should initialize");
        let packet = encoder
            .encode(&[0_i16; FRAME_PCM_SAMPLES])
            .expect("silence should encode");
        assert!(!packet.is_empty());
        assert!(packet.len() <= MAX_ENCODED_PACKET_BYTES);

        let decoded = decoder.decode(&packet).expect("silence should decode");
        assert_eq!(decoded.len(), FRAME_PCM_SAMPLES);
        assert!(decoded.iter().all(|sample| sample.abs() <= 1));
    }

    #[test]
    fn sine_wave_round_trips_with_a_fixed_frame_size() {
        let mut encoder =
            OpusEncoder::new(DEFAULT_CODEC_CONFIG).expect("encoder should initialize");
        let mut decoder =
            OpusDecoder::new(DEFAULT_CODEC_CONFIG).expect("decoder should initialize");
        let input = sine_frame(440.0);
        let packet = encoder.encode(&input).expect("sine wave should encode");
        let decoded = decoder.decode(&packet).expect("sine wave should decode");

        assert_eq!(decoded.len(), FRAME_PCM_SAMPLES);
        assert!(decoded.iter().any(|sample| sample.abs() > 100));
    }

    #[test]
    fn decoder_rejects_invalid_and_empty_packets() {
        let mut decoder =
            OpusDecoder::new(DEFAULT_CODEC_CONFIG).expect("decoder should initialize");
        assert!(matches!(decoder.decode(&[]), Err(CodecError::EmptyPacket)));
        assert!(decoder.decode(&[0xFF, 0x00, 0xAA]).is_err());
    }

    #[test]
    fn decoder_conceals_one_or_more_lost_frames_after_valid_audio() {
        let mut encoder =
            OpusEncoder::new(DEFAULT_CODEC_CONFIG).expect("encoder should initialize");
        let mut decoder =
            OpusDecoder::new(DEFAULT_CODEC_CONFIG).expect("decoder should initialize");
        let packet = encoder
            .encode(&sine_frame(220.0))
            .expect("sine wave should encode");
        decoder.decode(&packet).expect("first frame should decode");

        let concealed = decoder
            .decode_loss()
            .expect("first lost frame should be concealed");
        let concealed_again = decoder
            .decode_loss()
            .expect("second lost frame should be concealed");
        assert_eq!(concealed.len(), FRAME_PCM_SAMPLES);
        assert_eq!(concealed_again.len(), FRAME_PCM_SAMPLES);
    }
}
