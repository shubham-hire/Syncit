//! `audiosync-calibrate` measures the timing offset between two listeners by
//! cross-correlating recordings of the same playback.
//!
//! Record the same 30+ seconds of audio next to each listener's headphones
//! (any phone voice-memo app that exports 16-bit PCM WAV), then run:
//! `audiosync-calibrate --a listener1.wav --b listener2.wav`

use audiosync_apps::calibrate::{measure_offset, read_wav_mono};
use audiosync_apps::{Args, CliError};
use std::io::Read;

const USAGE: &str = "Usage: audiosync-calibrate --a FIRST.wav --b SECOND.wav [--max-lag-ms N]";

fn main() {
    if let Err(error) = run() {
        eprintln!("audiosync-calibrate failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), CliError> {
    let args = Args::parse(
        std::env::args().skip(1),
        &["a", "b", "max-lag-ms"],
        &["help"],
    )
    .map_err(|message| {
        if message.is_empty() {
            CliError {
                message: USAGE.to_owned(),
            }
        } else {
            CliError { message }
        }
    })?;

    let max_lag_ms: f64 = match args.value("max-lag-ms") {
        Some(text) => text
            .parse()
            .map_err(|_| format!("invalid --max-lag-ms {text:?}"))?,
        None => 750.0,
    };
    if !(10.0..=5_000.0).contains(&max_lag_ms) {
        return Err("--max-lag-ms must be between 10 and 5000".to_owned().into());
    }

    let first = load_wav(args.required_value("a")?)?;
    let second = load_wav(args.required_value("b")?)?;
    println!(
        "recording A: {} samples @ {} Hz",
        first.samples.len(),
        first.sample_rate
    );
    println!(
        "recording B: {} samples @ {} Hz",
        second.samples.len(),
        second.sample_rate
    );

    let (lag_seconds, quality) = measure_offset(&first, &second, max_lag_ms / 1_000.0);
    println!("correlation quality: {quality:.3}");
    if quality < 0.2 {
        return Err("the recordings do not match closely enough to measure; record longer or closer to the source"
            .to_owned()
            .into());
    }

    let lag_ms = lag_seconds * 1_000.0;
    println!("listener B is {lag_ms:+.1} ms relative to listener A");
    if lag_ms > 0.0 {
        println!(
            "suggestion: raise listener A's --delay-ms by {:.0} (or lower B's by the same)",
            lag_ms
        );
    } else {
        println!(
            "suggestion: raise listener B's --delay-ms by {:.0} (or lower A's by the same)",
            -lag_ms
        );
    }
    Ok(())
}

fn load_wav(path: &str) -> Result<audiosync_apps::calibrate::WavAudio, CliError> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|error| format!("could not open {path:?}: {error}"))?
        .read_to_end(&mut bytes)
        .map_err(|error| format!("could not read {path:?}: {error}"))?;
    read_wav_mono(&bytes).map_err(|error| format!("{path:?}: {error}").into())
}
