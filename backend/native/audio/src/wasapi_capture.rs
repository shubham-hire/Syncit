use crate::{AudioSourceError, PcmSource};
use audiosync_codec::CodecConfig;

/// WASAPI capture settings for the Windows system-output loopback source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WasapiCaptureConfig {
    /// Requested shared-mode audio-engine buffer duration.
    pub buffer_duration_millis: u32,
    /// Maximum time to poll for the next capture packet before returning an error.
    pub wait_timeout_millis: u32,
}

impl Default for WasapiCaptureConfig {
    fn default() -> Self {
        Self {
            buffer_duration_millis: 20,
            wait_timeout_millis: 1_000,
        }
    }
}

impl WasapiCaptureConfig {
    fn validate(self) -> Result<(), AudioSourceError> {
        if self.buffer_duration_millis == 0 {
            return Err(AudioSourceError::InvalidConfiguration {
                message: "buffer duration must be greater than zero".to_owned(),
            });
        }
        if self.wait_timeout_millis == 0 {
            return Err(AudioSourceError::InvalidConfiguration {
                message: "wait timeout must be greater than zero".to_owned(),
            });
        }
        Ok(())
    }
}

#[cfg(windows)]
mod windows {
    use super::*;
    use audiosync_codec::SAMPLE_RATE_HZ;
    use std::collections::VecDeque;
    use wasapi::{DeviceEnumerator, Direction, Role, SampleType, StreamMode, WaveFormat};

    const CAPTURE_CHANNELS: usize = 2;
    const CAPTURE_BYTES_PER_SAMPLE: usize = std::mem::size_of::<f32>();

    /// Windows WASAPI shared-mode loopback capture from the default render
    /// endpoint. The adapter converts the endpoint's stereo float stream into
    /// the mono signed-16 PCM frames required by the Opus boundary.
    pub struct WasapiCapture {
        audio_client: wasapi::AudioClient,
        capture_client: wasapi::AudioCaptureClient,
        pcm_queue: VecDeque<i16>,
        config: CodecConfig,
        bytes_per_frame: usize,
        wait_timeout_millis: u32,
        com_initialized: bool,
    }

    impl std::fmt::Debug for WasapiCapture {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter
                .debug_struct("WasapiCapture")
                .field("pcm_queue_len", &self.pcm_queue.len())
                .field("config", &self.config)
                .field("bytes_per_frame", &self.bytes_per_frame)
                .field("wait_timeout_millis", &self.wait_timeout_millis)
                .finish_non_exhaustive()
        }
    }

    impl WasapiCapture {
        pub fn default_loopback() -> Result<Self, AudioSourceError> {
            Self::new(WasapiCaptureConfig::default())
        }

        pub fn new(config: WasapiCaptureConfig) -> Result<Self, AudioSourceError> {
            config.validate()?;
            wasapi::initialize_mta()
                .ok()
                .map_err(|error| AudioSourceError::Capture {
                    message: format!("could not initialize Windows COM: {error}"),
                })?;

            match Self::open(config) {
                Ok(mut capture) => {
                    capture.com_initialized = true;
                    Ok(capture)
                }
                Err(error) => {
                    wasapi::deinitialize();
                    Err(error)
                }
            }
        }

        fn open(config: WasapiCaptureConfig) -> Result<Self, AudioSourceError> {
            let enumerator = DeviceEnumerator::new().map_err(capture_error)?;
            let device = enumerator
                .get_default_device_for_role(&Direction::Render, &Role::Multimedia)
                .map_err(capture_error)?;
            let mut audio_client = device.get_iaudioclient().map_err(capture_error)?;

            let capture_format = WaveFormat::new(
                32,
                32,
                &SampleType::Float,
                SAMPLE_RATE_HZ as usize,
                CAPTURE_CHANNELS,
                None,
            );
            let bytes_per_frame = capture_format.get_blockalign() as usize;
            let expected_bytes_per_frame = CAPTURE_CHANNELS * CAPTURE_BYTES_PER_SAMPLE;
            if bytes_per_frame != expected_bytes_per_frame {
                return Err(AudioSourceError::InvalidConfiguration {
                    message: format!(
                        "unexpected WASAPI block alignment: expected {expected_bytes_per_frame} bytes, got {bytes_per_frame}"
                    ),
                });
            }

            let buffer_duration_hns = i64::from(config.buffer_duration_millis)
                .checked_mul(10_000)
                .ok_or_else(|| AudioSourceError::InvalidConfiguration {
                    message: "buffer duration overflows the WASAPI time unit".to_owned(),
                })?;
            audio_client
                .initialize_client(
                    &capture_format,
                    &Direction::Capture,
                    &StreamMode::PollingShared {
                        autoconvert: true,
                        buffer_duration_hns,
                    },
                )
                .map_err(capture_error)?;
            let capture_client = audio_client
                .get_audiocaptureclient()
                .map_err(capture_error)?;
            audio_client.start_stream().map_err(capture_error)?;

            Ok(Self {
                audio_client,
                capture_client,
                pcm_queue: VecDeque::with_capacity(
                    2 * CodecConfig::default().pcm_samples_per_frame(),
                ),
                config: CodecConfig::default(),
                bytes_per_frame,
                wait_timeout_millis: config.wait_timeout_millis,
                com_initialized: false,
            })
        }

        fn read_available_packets(&mut self) -> Result<(), AudioSourceError> {
            loop {
                let packet_frames = self
                    .capture_client
                    .get_next_packet_size()
                    .map_err(capture_error)?
                    .ok_or_else(|| AudioSourceError::Capture {
                        message: "WASAPI loopback did not expose shared-mode packets".to_owned(),
                    })?;
                if packet_frames == 0 {
                    return Ok(());
                }

                let packet_bytes = (packet_frames as usize)
                    .checked_mul(self.bytes_per_frame)
                    .ok_or_else(|| AudioSourceError::Capture {
                        message: "WASAPI packet size overflows the local buffer".to_owned(),
                    })?;
                let mut bytes = vec![0_u8; packet_bytes];
                let (frames_read, buffer_info) = self
                    .capture_client
                    .read_from_device(&mut bytes)
                    .map_err(capture_error)?;
                let frames_read = frames_read as usize;
                if frames_read == 0 {
                    return Ok(());
                }

                if buffer_info.flags.silent {
                    self.pcm_queue
                        .extend(std::iter::repeat_n(0_i16, frames_read));
                } else {
                    self.append_float_stereo_as_mono(&bytes, frames_read)?;
                }
            }
        }

        fn append_float_stereo_as_mono(
            &mut self,
            bytes: &[u8],
            frames: usize,
        ) -> Result<(), AudioSourceError> {
            let required_bytes = frames.checked_mul(self.bytes_per_frame).ok_or_else(|| {
                AudioSourceError::Capture {
                    message: "WASAPI frame count overflows the local buffer".to_owned(),
                }
            })?;
            if bytes.len() < required_bytes {
                return Err(AudioSourceError::Capture {
                    message: format!(
                        "WASAPI returned {frames} frames but only {} bytes were read",
                        bytes.len()
                    ),
                });
            }

            for frame in 0..frames {
                let offset = frame * self.bytes_per_frame;
                let left = read_f32(&bytes[offset..offset + CAPTURE_BYTES_PER_SAMPLE]);
                let right = read_f32(
                    &bytes[offset + CAPTURE_BYTES_PER_SAMPLE
                        ..offset + CAPTURE_BYTES_PER_SAMPLE * CAPTURE_CHANNELS],
                );
                let average = (left + right) * 0.5;
                let sample = if average.is_finite() {
                    (average.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16
                } else {
                    0
                };
                self.pcm_queue.push_back(sample);
            }
            Ok(())
        }
    }

    impl PcmSource for WasapiCapture {
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

            let deadline = std::time::Instant::now()
                + std::time::Duration::from_millis(u64::from(self.wait_timeout_millis));
            while self.pcm_queue.len() < expected {
                self.read_available_packets()?;
                if self.pcm_queue.len() >= expected {
                    break;
                }
                let remaining = deadline.saturating_duration_since(std::time::Instant::now());
                if remaining.is_zero() {
                    return Err(AudioSourceError::Capture {
                        message: format!(
                            "WASAPI loopback produced no capture packet within {} ms",
                            self.wait_timeout_millis
                        ),
                    });
                }
                std::thread::sleep(remaining.min(std::time::Duration::from_millis(5)));
            }

            for sample in pcm.iter_mut() {
                *sample = self
                    .pcm_queue
                    .pop_front()
                    .expect("the capture queue was filled before reading a frame");
            }
            Ok(())
        }
    }

    impl Drop for WasapiCapture {
        fn drop(&mut self) {
            let _ = self.audio_client.stop_stream();
            if self.com_initialized {
                wasapi::deinitialize();
            }
        }
    }

    fn capture_error(error: impl std::fmt::Display) -> AudioSourceError {
        AudioSourceError::Capture {
            message: error.to_string(),
        }
    }

    fn read_f32(bytes: &[u8]) -> f32 {
        f32::from_le_bytes(
            bytes
                .try_into()
                .expect("WASAPI float sample has a fixed four-byte representation"),
        )
    }
}

#[cfg(windows)]
pub use windows::WasapiCapture;

#[cfg(not(windows))]
mod unsupported {
    use super::*;

    /// Placeholder type on non-Windows targets so callers can report the
    /// platform limitation without conditionalizing the rest of the pipeline.
    #[derive(Debug)]
    pub struct WasapiCapture {
        config: CodecConfig,
    }

    impl WasapiCapture {
        pub fn default_loopback() -> Result<Self, AudioSourceError> {
            Self::new(WasapiCaptureConfig::default())
        }

        pub fn new(config: WasapiCaptureConfig) -> Result<Self, AudioSourceError> {
            config.validate()?;
            Err(AudioSourceError::UnsupportedPlatform {
                required: "Windows WASAPI",
            })
        }
    }

    impl PcmSource for WasapiCapture {
        fn config(&self) -> CodecConfig {
            self.config
        }

        fn read_frame(&mut self, _pcm: &mut [i16]) -> Result<(), AudioSourceError> {
            Err(AudioSourceError::UnsupportedPlatform {
                required: "Windows WASAPI",
            })
        }
    }
}

#[cfg(not(windows))]
pub use unsupported::WasapiCapture;
