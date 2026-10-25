//! `audiosync-host` streams captured system audio (or a synthetic tone on
//! non-Windows machines) to approved listeners over UDP.
//!
//! Run it, note the printed session/token, then start `audiosync-listener`
//! on the receiving machine. Commands on stdin: `list`, `end` (or Ctrl-D).

use audiosync_apps::{Args, CliError, hex16, parse_hex16, random_bytes16};
use audiosync_runtime::{HostCommand, HostLoopConfig, HostLoopHandle, spawn_host_loop};
use audiosync_session::{ListenerState, SessionEvent};
use audiosync_transport::{ListenerId, SessionToken};
use std::collections::HashMap;
use std::io::Write;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

const USAGE: &str = "Usage: audiosync-host [--bind ADDR] [--max-listeners N] [--session HEX32] \
[--token HEX32] [--synthetic] [--manual-approve]";

fn main() {
    if let Err(error) = run() {
        eprintln!("audiosync-host failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), CliError> {
    let args = Args::parse(
        std::env::args().skip(1),
        &["bind", "max-listeners", "session", "token"],
        &["synthetic", "manual-approve", "help"],
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

    let bind = args.value("bind").unwrap_or("0.0.0.0:41000").to_owned();
    let max_listeners: usize = args
        .value("max-listeners")
        .map(|value| {
            value
                .parse()
                .map_err(|_| format!("invalid --max-listeners {value:?}; expected a number"))
        })
        .transpose()?
        .unwrap_or(5);
    let session_bytes = match args.value("session") {
        Some(text) => parse_hex16(text).map_err(CliError::from)?,
        None => random_bytes16(),
    };
    let token_bytes = match args.value("token") {
        Some(text) => parse_hex16(text).map_err(CliError::from)?,
        None => random_bytes16(),
    };
    let auto_approve = !args.flag("manual-approve");

    #[cfg(windows)]
    let (source_kind, source): (&str, Box<dyn audiosync_audio::PcmSource + Send>) =
        if args.flag("synthetic") {
            (
                "synthetic tone",
                Box::new(audiosync_audio::SyntheticPcmSource::new(220.0)),
            )
        } else {
            (
                "WASAPI loopback",
                Box::new(
                    audiosync_audio::WasapiCapture::default_loopback()
                        .map_err(|error| format!("WASAPI capture unavailable: {error}"))?,
                ),
            )
        };

    #[cfg(not(windows))]
    let (source_kind, source) = {
        if !args.flag("synthetic") {
            eprintln!("note: WASAPI capture requires Windows; using the synthetic source");
        }
        (
            "synthetic tone",
            audiosync_audio::SyntheticPcmSource::new(220.0),
        )
    };

    let runtime = audiosync_runtime::HostRuntime::new(
        bind.as_str(),
        session_bytes,
        SessionToken::new(token_bytes),
        max_listeners,
        source,
        audiosync_runtime::monotonic_micros() as u64,
    )
    .map_err(|error| format!("could not start the host runtime: {error}"))?;

    let handle = spawn_host_loop(
        runtime,
        Box::new(move |_: ListenerId| auto_approve),
        HostLoopConfig::default(),
    )
    .map_err(|error| format!("could not start the host loop: {error}"))?;

    println!("AudioSync host");
    println!("  bind address : {}", handle.local_addr());
    println!("  session id   : {}", hex16(&session_bytes));
    println!("  session token: {}", hex16(&token_bytes));
    println!("  source       : {source_kind}");
    println!(
        "  approval     : {}",
        if auto_approve {
            "auto"
        } else {
            "manual (`list` then approve in a future build)"
        }
    );
    println!();
    println!("listener arguments:");
    println!(
        "  audiosync-listener --host {} --session {} --token {}",
        host_address(&handle),
        hex16(&session_bytes),
        hex16(&token_bytes)
    );
    println!();
    println!("commands: list | end");

    let (stdin_sender, stdin_receiver) = mpsc::channel::<String>();
    thread::spawn(move || {
        let mut line = String::new();
        loop {
            line.clear();
            match std::io::stdin().read_line(&mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    if stdin_sender.send(line.trim().to_owned()).is_err() {
                        break;
                    }
                }
            }
        }
    });

    let mut listeners: HashMap<ListenerId, (String, ListenerState)> = HashMap::new();
    let mut next_report = Instant::now() + Duration::from_secs(2);
    let mut stdin_open = true;
    loop {
        if stdin_open {
            match stdin_receiver.recv_timeout(Duration::from_millis(200)) {
                Ok(command) if command == "end" => {
                    println!("ending session...");
                    let _ = handle.send_command(HostCommand::EndSession(0));
                    thread::sleep(Duration::from_millis(300));
                    handle.stop();
                    println!("session ended");
                    break;
                }
                Ok(command) if command == "list" => print_listeners(&mut listeners),
                Ok(command) if command.is_empty() => {}
                Ok(command) => println!("unknown command {command:?}; use: list | end"),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    // Closed stdin (pipes, background runs) must not end the
                    // session; only the `end` command or process kill does.
                    println!("stdin closed; send 'end' via a pipe or press Ctrl+C to stop");
                    stdin_open = false;
                }
            }
        } else {
            thread::sleep(Duration::from_millis(200));
        }

        for event in handle.take_events() {
            apply_event(&mut listeners, &event);
            println!("  event: {event:?}");
        }
        for error in handle.take_errors() {
            eprintln!("  host error: {error}");
        }
        if !handle.is_running() {
            handle.stop();
            println!("host loop stopped");
            break;
        }

        let now = Instant::now();
        if now >= next_report {
            next_report = now + Duration::from_secs(10);
            let active = listeners
                .values()
                .filter(|(_, state)| *state == ListenerState::Active)
                .count();
            println!(
                "status: {active} active listener(s), {} tracked",
                listeners.len()
            );
        }
    }

    let _ = std::io::stdout().flush();
    Ok(())
}

/// Prefer the wildcard-resolved local address for the listener command line.
fn host_address(handle: &HostLoopHandle) -> String {
    let address = handle.local_addr();
    if address.ip().is_unspecified() {
        format!("127.0.0.1:{}", address.port())
    } else {
        address.to_string()
    }
}

fn apply_event(listeners: &mut HashMap<ListenerId, (String, ListenerState)>, event: &SessionEvent) {
    match event {
        SessionEvent::ListenerJoined {
            endpoint,
            listener_id,
            ..
        }
        | SessionEvent::ListenerReconnected {
            endpoint,
            listener_id,
            ..
        } => {
            listeners.insert(
                *listener_id,
                (endpoint.to_string(), ListenerState::PendingApproval),
            );
        }
        SessionEvent::ListenerApproved { listener_id, .. } => {
            if let Some(entry) = listeners.get_mut(listener_id) {
                entry.1 = ListenerState::Active;
            }
        }
        SessionEvent::ListenerRemoved { listener_id, .. } => {
            if let Some(entry) = listeners.get_mut(listener_id) {
                entry.1 = ListenerState::Removed;
            }
        }
        SessionEvent::ListenerTimedOut { listener_id, .. } => {
            if let Some(entry) = listeners.get_mut(listener_id) {
                entry.1 = ListenerState::TimedOut;
            }
        }
        _ => {}
    }
}

fn print_listeners(listeners: &mut HashMap<ListenerId, (String, ListenerState)>) {
    if listeners.is_empty() {
        println!("no listeners have joined yet");
        return;
    }
    for (listener_id, (endpoint, state)) in listeners.iter() {
        println!("  {listener_id:?} at {endpoint}: {state:?}");
    }
}
