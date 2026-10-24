//! `audiosync-listener` joins a host session and plays the received stream
//! through the default output device (or discards it with `--null-output`).
//!
//! Example:
//! `audiosync-listener --host 192.168.137.1:41000 --session <hex> --token <hex> --delay-ms 40`

use audiosync_apps::{Args, CliError, parse_hex16};
use audiosync_codec::CodecConfig;
use audiosync_runtime::{ListenerRuntime, NullSink, PlaybackConfig, PlaybackScheduler};
use audiosync_transport::SessionToken;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

const USAGE: &str = "Usage: audiosync-listener --host IP:PORT --session HEX32 --token HEX32 \
[--delay-ms N] [--buffer-ms N] [--grace-ms N] [--volume 0.0-1.0] [--null-output] \
[--duration-secs N] [--csv FILE]";

fn main() {
    if let Err(error) = run() {
        eprintln!("audiosync-listener failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), CliError> {
    let args = Args::parse(
        std::env::args().skip(1),
        &[
            "host",
            "session",
            "token",
            "delay-ms",
            "buffer-ms",
            "grace-ms",
            "volume",
            "duration-secs",
            "csv",
        ],
        &["null-output", "help"],
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

    let host_text = args.required_value("host")?.to_owned();
    let host: SocketAddr = host_text
        .parse()
        .map_err(|_| format!("invalid --host {host_text:?}"))?;
    let session_bytes = parse_hex16(args.required_value("session")?).map_err(CliError::from)?;
    let token_bytes = parse_hex16(args.required_value("token")?).map_err(CliError::from)?;
    let delay_ms = parse_number(&args, "delay-ms", 0, 2_000)?;
    let buffer_ms = parse_number(&args, "buffer-ms", 20, 2_000)?.max(20);
    let grace_ms = parse_number(&args, "grace-ms", 20, 2_000)?.max(20);
    let volume: f32 = match args.value("volume") {
        Some(text) => text
            .parse()
            .map_err(|_| format!("invalid --volume {text:?}; expected 0.0 to 1.0"))?,
        None => 0.8,
    };
    if !(0.0..=1.0).contains(&volume) {
        return Err(format!("invalid --volume {volume}; expected 0.0 to 1.0").into());
    }
    let duration_secs = parse_number(&args, "duration-secs", 0, 86_400)?;

    let mut runtime = ListenerRuntime::new(
        "0.0.0.0:0",
        session_bytes,
        SessionToken::new(token_bytes),
        host,
        CodecConfig::default(),
        16,
    )
    .map_err(|error| format!("could not start the listener runtime: {error}"))?;
    runtime
        .set_read_timeout(Some(Duration::from_millis(5)))
        .map_err(|error| format!("could not configure the listener socket: {error}"))?;

    let null_output = args.flag("null-output");
    let mut cpal_sink = if null_output {
        None
    } else {
        match audiosync_apps::cpal_sink::CpalSink::new(volume) {
            Ok(sink) => {
                println!(
                    "audio output opened ({} channel(s) at 48 kHz)",
                    sink.channels()
                );
                Some(sink)
            }
            Err(error) => {
                eprintln!("warning: {error}; falling back to --null-output");
                None
            }
        }
    };

    println!("AudioSync listener joining {host}");
    runtime
        .send_join()
        .map_err(|error| format!("could not send the join request: {error}"))?;

    let mut scheduler = PlaybackScheduler::new(PlaybackConfig {
        target_buffer: Duration::from_millis(buffer_ms),
        conceal_grace: Duration::from_millis(grace_ms),
        listener_delay: Duration::from_millis(delay_ms),
        ..PlaybackConfig::default()
    });
    let mut null_sink = NullSink::default();
    let mut metrics_csv = match args.value("csv") {
        Some(path) => {
            use std::io::Write;
            let mut file = std::fs::File::create(path)
                .map_err(|error| format!("could not create {path:?}: {error}"))?;
            writeln!(file, "elapsed_s,state,rendered,concealed,received,duplicate,late,unauthenticated,buffered,rtt_us,offset_us")
                .map_err(|error| format!("could not write {path:?}: {error}"))?;
            Some((file, std::time::Instant::now()))
        }
        None => None,
    };

    let deadline = if duration_secs == 0 {
        None
    } else {
        Some(Instant::now() + Duration::from_secs(duration_secs))
    };
    let mut next_report = Instant::now() + Duration::from_secs(1);
    let mut last_rendered: u64 = 0;

    loop {
        let pumped = match cpal_sink.as_mut() {
            Some(sink) => scheduler.pump(&mut runtime, sink),
            None => scheduler.pump(&mut runtime, &mut null_sink),
        };
        if let Err(error) = pumped {
            eprintln!("playback error: {error}");
        }

        for event in scheduler.take_control_events() {
            println!("  session: {event:?}");
        }

        let now = Instant::now();
        if now >= next_report {
            next_report = now + Duration::from_secs(1);
            let playback = scheduler.stats();
            let pipeline = runtime.pipeline().metrics();
            let session = runtime.session();
            let rendered = playback.rendered_frames;
            let rate = rendered.saturating_sub(last_rendered);
            last_rendered = rendered;
            if let Some((file, started)) = metrics_csv.as_mut() {
                use std::io::Write;
                let _ = writeln!(
                    file,
                    "{:.1},{:?},{rendered},{},{},{},{},{},{},{},{}",
                    started.elapsed().as_secs_f32(),
                    session.state(),
                    playback.concealed_frames,
                    pipeline.received_packets,
                    pipeline.duplicate_packets,
                    pipeline.too_late_packets,
                    pipeline.unauthenticated_packets,
                    runtime.pipeline().buffered_packets(),
                    session.best_round_trip_micros().unwrap_or(-1),
                    session.clock_offset_micros().unwrap_or(0),
                );
            }
            println!(
                "state {:?} | rendered {rendered} (+{rate}/s) concealed {} | recv {} dup {} late {} forger {} | buffered {} | rtt {} µs offset {} µs",
                session.state(),
                playback.concealed_frames,
                pipeline.received_packets,
                pipeline.duplicate_packets,
                pipeline.too_late_packets,
                pipeline.unauthenticated_packets,
                runtime.pipeline().buffered_packets(),
                session
                    .best_round_trip_micros()
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "n/a".to_owned()),
                session
                    .clock_offset_micros()
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "n/a".to_owned()),
            );
        }

        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            println!("duration reached; leaving");
            break;
        }
    }

    print_final_report(&runtime, &scheduler);
    if let Some(sink) = cpal_sink {
        // Keep the stream handle alive until after the final report.
        let _ = sink.into_stream();
    }
    Ok(())
}

fn parse_number(args: &Args, key: &str, minimum: u64, maximum: u64) -> Result<u64, CliError> {
    match args.value(key) {
        None => Ok(minimum),
        Some(text) => {
            let value = text
                .parse()
                .map_err(|_| format!("invalid --{key} {text:?}; expected a number"))?;
            if value < minimum || value > maximum {
                return Err(format!("--{key} must be between {minimum} and {maximum}").into());
            }
            Ok(value)
        }
    }
}

fn print_final_report(runtime: &ListenerRuntime, scheduler: &PlaybackScheduler) {
    let playback = scheduler.stats();
    let pipeline = runtime.pipeline().metrics();
    println!(
        "final: rendered {} concealed {} | received {} invalid {} wrong-session {} dup {} late {} capacity-drops {} unauthenticated {} | rtt {} µs offset {} µs",
        playback.rendered_frames,
        playback.concealed_frames,
        pipeline.received_packets,
        pipeline.invalid_packets,
        pipeline.wrong_session_packets,
        pipeline.duplicate_packets,
        pipeline.too_late_packets,
        pipeline.capacity_drops,
        pipeline.unauthenticated_packets,
        runtime
            .session()
            .best_round_trip_micros()
            .map(|value| value.to_string())
            .unwrap_or_else(|| "n/a".to_owned()),
        runtime
            .session()
            .clock_offset_micros()
            .map(|value| value.to_string())
            .unwrap_or_else(|| "n/a".to_owned()),
    );
}
