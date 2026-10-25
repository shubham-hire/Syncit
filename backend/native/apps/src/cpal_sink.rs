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
use std::sync::mpsc::{self, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

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
            SampleFormat::U8 => {
                build_stream::<u8>(&device, &config, Arc::clone(&queue), float_to_u8)?
            }
            other => {
                return Err(sink_error(format!(
                    "output device uses unsupported sample format {other:?} (need f32, i16, or u8)"
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

/// Send-safe wrapper that owns the CPAL stream on a dedicated output thread.
///
/// CPAL deliberately marks its stream handle as not `Send`, while the FFI
/// bridge stores listener entries behind a cross-thread mutex. This adapter
/// keeps the non-`Send` stream and [`CpalSink`] on one worker thread and sends
/// decoded mono PCM frames to it through a bounded channel.
pub struct CpalChannelSink {
    sender: Option<SyncSender<Vec<i16>>>,
    thread: Option<JoinHandle<()>>,
    frames_written: u64,
}

impl CpalChannelSink {
    const QUEUE_CAPACITY: usize = 64;

    /// Opens the default output device on the worker thread and waits until
    /// the stream has either started or reported an initialization error.
    pub fn new(volume: f32) -> Result<Self, PlaybackError> {
        let (sender, receiver) = mpsc::sync_channel::<Vec<i16>>(Self::QUEUE_CAPACITY);
        let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
        let thread = thread::spawn(move || match CpalSink::new(volume) {
            Ok(mut sink) => {
                let _ = ready_sender.send(Ok(()));
                while let Ok(frame) = receiver.recv() {
                    if let Err(error) = sink.write_frame(&frame) {
                        eprintln!("audio output error: {error}");
                        break;
                    }
                }
            }
            Err(error) => {
                let _ = ready_sender.send(Err(error.to_string()));
            }
        });

        match ready_receiver.recv() {
            Ok(Ok(())) => Ok(Self {
                sender: Some(sender),
                thread: Some(thread),
                frames_written: 0,
            }),
            Ok(Err(message)) => {
                let _ = thread.join();
                Err(sink_error(message))
            }
            Err(_) => {
                let _ = thread.join();
                Err(sink_error("audio output worker exited during startup"))
            }
        }
    }
}

impl PcmSink for CpalChannelSink {
    fn write_frame(&mut self, pcm: &[i16]) -> Result<(), PlaybackError> {
        let sender = self
            .sender
            .as_ref()
            .ok_or_else(|| sink_error("audio output worker is stopped"))?;
        match sender.try_send(pcm.to_vec()) {
            Ok(()) => {
                self.frames_written = self.frames_written.saturating_add(1);
                Ok(())
            }
            Err(TrySendError::Full(_)) => Err(sink_error("audio output queue is full")),
            Err(TrySendError::Disconnected(_)) => {
                Err(sink_error("audio output worker disconnected"))
            }
        }
    }
}

impl Drop for CpalChannelSink {
    fn drop(&mut self) {
        // Close the channel before joining so the worker can leave recv().
        self.sender.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
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

fn build_stream<T: cpal::SizedSample>(
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

fn float_to_u8(sample: f32) -> u8 {
    (((sample.clamp(-1.0, 1.0) + 1.0) * 0.5) * f32::from(u8::MAX)).round() as u8
}

fn sink_error(message: impl Into<String>) -> PlaybackError {
    PlaybackError::Sink {
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::float_to_u8;

    #[test]
    fn unsigned_eight_bit_output_maps_signed_pcm_range() {
        assert_eq!(float_to_u8(-1.0), 0);
        assert_eq!(float_to_u8(0.0), 128);
        assert_eq!(float_to_u8(1.0), 255);
        assert_eq!(float_to_u8(-2.0), 0);
        assert_eq!(float_to_u8(2.0), 255);
    }
}
