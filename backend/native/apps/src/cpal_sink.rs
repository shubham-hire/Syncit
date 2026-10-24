//! CPAL output sink for the listener prototype.
//!
//! Requests a 48 kHz output stream (mono or stereo) from the default output
//! device, converts the pipeline's mono i16 frames to the stream's channel
//! count and sample format, and feeds an audio callback from a bounded queue.
//! Underruns play as silence rather than failing the stream.

use audiosync_runtime::{PcmSink, PlaybackError};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{
    BuildStreamError, Device, OutputCallbackInfo, SampleFormat, Stream, StreamConfig,
    SupportedStreamConfig,
};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// Frames buffered ahead before the oldest complete frame is dropped. Bounds
/// memory if the audio callback stalls; oversized backlog means the output
/// clock is behind and old audio is stale anyway.
const MAX_QUEUED_FRAMES: usize = 10;

/// Plays decoded PCM frames through the system's default output device.
pub struct CpalSink {
    queue: Arc<Mutex<VecDeque<f32>>>,
    stream: Stream,
    channels: usize,
    volume: f32,
    frames_written: u64,
    dropped_frames: u64,
}

impl CpalSink {
    /// Opens the default output device at 48 kHz. `volume` in 0.0..=1.0 is
    /// applied per sample before queueing.
    pub fn new(volume: f32) -> Result<Self, PlaybackError> {
        let volume = volume.clamp(0.0, 1.0);
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| sink_error("no default audio output device is available"))?;

        let config = pick_48khz_config(&device)?;
        let channels = config.channels().max(1) as usize;
        let queue: Arc<Mutex<VecDeque<f32>>> = Arc::new(Mutex::new(VecDeque::new()));

        let stream = match config.sample_format() {
            SampleFormat::F32 => {
                build_stream::<f32>(&device, &config, Arc::clone(&queue), |sample| sample)?
            }
            SampleFormat::I16 => {
                build_stream::<i16>(&device, &config, Arc::clone(&queue), |sample| {
                    (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16
                })?
            }
            other => {
                return Err(sink_error(format!(
                    "output device uses unsupported sample format {other:?} (need f32 or i16)"
                )));
            }
        };

        stream
            .play()
            .map_err(|error| sink_error(format!("could not start the output stream: {error}")))?;

        Ok(Self {
            queue,
            stream,
            channels,
            volume,
            frames_written: 0,
            dropped_frames: 0,
        })
    }

    pub fn channels(&self) -> usize {
        self.channels
    }

    pub fn frames_written(&self) -> u64 {
        self.frames_written
    }

    pub fn dropped_frames(&self) -> u64 {
        self.dropped_frames
    }

    /// Keeps the stream alive; CPAL stops playing when the last handle drops.
    pub fn into_stream(self) -> Stream {
        self.stream
    }
}

impl PcmSink for CpalSink {
    fn write_frame(&mut self, pcm: &[i16]) -> Result<(), PlaybackError> {
        let frame_samples = pcm.len() * self.channels;
        let Ok(mut queue) = self.queue.lock() else {
            return Err(sink_error("audio queue lock poisoned"));
        };
        while queue.len() + frame_samples > MAX_QUEUED_FRAMES * pcm.len() * self.channels {
            // Drop whole frames from the front to stay aligned.
            queue.drain(..pcm.len() * self.channels);
            self.dropped_frames = self.dropped_frames.saturating_add(1);
        }
        for &sample in pcm {
            let scaled = (f32::from(sample) / 32_768.0) * self.volume;
            for _ in 0..self.channels {
                queue.push_back(scaled);
            }
        }
        self.frames_written = self.frames_written.saturating_add(1);
        Ok(())
    }
}

fn pick_48khz_config(device: &Device) -> Result<SupportedStreamConfig, PlaybackError> {
    let ranges = device
        .supported_output_configs()
        .map_err(|error| sink_error(format!("could not query output device: {error}")))?;

    let mut fallback: Option<SupportedStreamConfig> = None;
    for range in ranges {
        if range.min_sample_rate().0 > 48_000 || range.max_sample_rate().0 < 48_000 {
            continue;
        }
        let candidate = range.with_sample_rate(cpal::SampleRate(48_000));
        if matches!(candidate.channels(), 1 | 2) {
            return Ok(candidate);
        }
        if fallback.is_none() {
            fallback = Some(candidate);
        }
    }

    fallback.ok_or_else(|| sink_error("the default output device does not support 48 kHz audio"))
}

fn build_stream<T: cpal::SizedSample + cpal::FromSample<f32>>(
    device: &Device,
    config: &SupportedStreamConfig,
    queue: Arc<Mutex<VecDeque<f32>>>,
    convert: impl Fn(f32) -> T + Send + 'static,
) -> Result<Stream, PlaybackError> {
    let stream_config: StreamConfig = config.clone().into();
    let stream = device
        .build_output_stream(
            &stream_config,
            move |data: &mut [T], _: &OutputCallbackInfo| {
                // A failed lock means the sink was dropped; output silence.
                let Ok(mut queue) = queue.lock() else {
                    for slot in data.iter_mut() {
                        *slot = convert(0.0);
                    }
                    return;
                };
                for slot in data.iter_mut() {
                    *slot = convert(queue.pop_front().unwrap_or(0.0));
                }
            },
            |error| eprintln!("audio output error: {error}"),
            None,
        )
        .map_err(|error: BuildStreamError| {
            sink_error(format!("could not build the output stream: {error}"))
        })?;
    Ok(stream)
}

fn sink_error(message: impl Into<String>) -> PlaybackError {
    PlaybackError::Sink {
        message: message.into(),
    }
}
