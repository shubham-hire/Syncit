use audiosync_audio::{OpusEncoderPipeline, WasapiCapture};
use audiosync_codec::{FRAME_PCM_SAMPLES, OpusDecoder};
use std::env;
use std::process;
use std::time::Instant;

const DEFAULT_FRAME_COUNT: usize = 500;
const NON_SILENT_SAMPLE_THRESHOLD: i16 = 32;

fn main() {
    if let Err(error) = run() {
        eprintln!("WASAPI capture smoke test failed: {error}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let frame_count = parse_frame_count(env::args().skip(1))?;
    let started = Instant::now();

    let source = WasapiCapture::default_loopback()
        .map_err(|error| format!("WASAPI initialization failed: {error}"))?;
    let mut pipeline: OpusEncoderPipeline<WasapiCapture> = OpusEncoderPipeline::new(source)
        .map_err(|error| format!("Opus encoder initialization failed: {error}"))?;
    let config = pipeline.config();
    if config.channels != 1 || config.pcm_samples_per_frame() != FRAME_PCM_SAMPLES {
        return Err(format!(
            "unexpected pipeline PCM configuration: expected mono {FRAME_PCM_SAMPLES}-sample frames, got {config:?}"
        ));
    }

    let mut decoder = OpusDecoder::new(config)
        .map_err(|error| format!("Opus decoder initialization failed: {error}"))?;
    let mut frames_captured = 0_usize;
    let mut packets_encoded = 0_usize;
    let mut packets_decoded = 0_usize;
    let mut maximum_packet_size = 0_usize;
    let mut non_silent_frames = 0_usize;

    for frame_index in 0..frame_count {
        let packet = pipeline.next_packet().map_err(|error| {
            format!(
                "capture/encoding failed at frame {} of {frame_count} (including WASAPI timeout handling): {error}",
                frame_index + 1
            )
        })?;
        frames_captured += 1;
        packets_encoded += 1;
        maximum_packet_size = maximum_packet_size.max(packet.len());

        let decoded = decoder.decode(&packet).map_err(|error| {
            format!(
                "Opus decoding failed at packet {} of {frame_count}: {error}",
                packets_encoded
            )
        })?;
        packets_decoded += 1;
        if decoded.len() != FRAME_PCM_SAMPLES {
            return Err(format!(
                "decoded packet {} has {} mono PCM samples; expected exactly {FRAME_PCM_SAMPLES}",
                packets_decoded,
                decoded.len()
            ));
        }
        if decoded
            .iter()
            .any(|sample| sample.unsigned_abs() > NON_SILENT_SAMPLE_THRESHOLD.unsigned_abs())
        {
            non_silent_frames += 1;
        }
    }

    let elapsed = started.elapsed();
    println!("WASAPI capture smoke test PASS");
    println!("frames captured: {frames_captured}");
    println!("packets encoded: {packets_encoded}");
    println!("packets decoded: {packets_decoded}");
    println!("maximum Opus packet size: {maximum_packet_size} bytes");
    println!("non-silent frames: {non_silent_frames}");
    println!("WASAPI timeouts: 0");
    println!("elapsed time: {:.3} seconds", elapsed.as_secs_f64());
    Ok(())
}

fn parse_frame_count(mut arguments: impl Iterator<Item = String>) -> Result<usize, String> {
    let mut frame_count = DEFAULT_FRAME_COUNT;
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--frames" => {
                let value = arguments
                    .next()
                    .ok_or_else(|| "--frames requires a positive integer".to_owned())?;
                frame_count = value.parse::<usize>().map_err(|_| {
                    format!("invalid --frames value {value:?}; expected a positive integer")
                })?;
            }
            "--help" | "-h" => {
                println!(
                    "Usage: wasapi_capture_smoke [--frames N]\n\nCaptures N 20 ms frames from the default Windows render endpoint, round-trips each frame through Opus in memory, and reports capture statistics.\n\nDefault: --frames {DEFAULT_FRAME_COUNT}"
                );
                process::exit(0);
            }
            other => {
                return Err(format!(
                    "unrecognized argument {other:?}; use --frames N or --help"
                ));
            }
        }
    }

    if frame_count == 0 {
        return Err("--frames must be greater than zero".to_owned());
    }
    Ok(frame_count)
}
