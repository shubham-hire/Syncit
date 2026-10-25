//! Narrow C ABI over the AudioSync runtime for the Flutter application.
//!
//! The bridge follows PRD §9: Flutter owns screens and controls; the Rust
//! engine owns sockets, cadence, and audio. State and events cross the
//! boundary as small JSON documents so the Dart side needs no code
//! generation beyond `dart:ffi`.
//!
//! Threading model: each host handle owns its runtime on a dedicated thread
//! (the same host loop used by the CLI). Calls are cheap channel sends; events
//! accumulate in bounded logs that Dart drains through the command/pump calls.
//! Host and listener stop calls release their global handle slots. All returned
//! strings are heap-allocated UTF-8 and must be freed with
//! [`audiosync_ffi_free_string`].

use audiosync_apps::{hex16, parse_hex16, random_bytes16};
use audiosync_runtime::{
    HostCommand, HostLoopConfig, HostLoopHandle, ListenerRuntime, NullSink, PlaybackConfig,
    PlaybackScheduler, spawn_host_loop_with_factory,
};
use audiosync_session::ListenerState;
use audiosync_transport::SessionToken;
use std::ffi::{CStr, CString, c_char};
use std::net::ToSocketAddrs;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

mod json;
use json::JsonWriter;

const MAX_HOSTS: usize = 8;
const MAX_LISTENERS: usize = 8;

struct HostEntry {
    handle: HostLoopHandle,
}

struct ListenerEntry {
    runtime: Mutex<ListenerRuntime>,
    scheduler: Mutex<PlaybackScheduler>,
    events: Mutex<Vec<String>>,
}

struct Bridge {
    hosts: Mutex<Vec<Option<HostEntry>>>,
    listeners: Mutex<Vec<Option<ListenerEntry>>>,
}

fn bridge() -> &'static Bridge {
    static BRIDGE: OnceLock<Bridge> = OnceLock::new();
    BRIDGE.get_or_init(|| Bridge {
        hosts: Mutex::new((0..MAX_HOSTS).map(|_| None).collect()),
        listeners: Mutex::new((0..MAX_LISTENERS).map(|_| None).collect()),
    })
}

fn alloc_string(text: String) -> *mut c_char {
    CString::new(text)
        .map(CString::into_raw)
        .unwrap_or_else(|_| std::ptr::null_mut())
}

/// # Safety
/// `ptr` must originate from this library's string-returning functions and
/// must not be freed twice.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn audiosync_ffi_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe { drop(CString::from_raw(ptr)) };
    }
}

/// # Safety
/// `bind_json` must be a NUL-terminated UTF-8 string for the duration of the
/// call. Expected shape: `{"bind":"0.0.0.0:41000","max_listeners":5,
/// "synthetic":true,"session":"<hex>","token":"<hex>","auto_approve":true}`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn audiosync_ffi_host_start(bind_json: *const c_char) -> *mut c_char {
    let request = match unsafe { decode_json(bind_json) } {
        Some(text) => text,
        None => return alloc_string(error_response("invalid bind json")),
    };
    match host_start(&request) {
        Ok(response) => alloc_string(response),
        Err(message) => alloc_string(error_response(&message)),
    }
}

fn host_start(request: &str) -> Result<String, String> {
    if !request.trim_start().starts_with('{') {
        return Err("expected a json object".to_owned());
    }
    let bind = json::get_str(request, "bind").unwrap_or("0.0.0.0:41000");
    let max_listeners = json::get_u64(request, "max_listeners").unwrap_or(5) as usize;
    let synthetic = json::get_bool(request, "synthetic").unwrap_or(true);
    let auto_approve = json::get_bool(request, "auto_approve").unwrap_or(true);
    let session_bytes = match json::get_str(request, "session").and_then(|s| parse_hex16(s).ok()) {
        Some(bytes) => bytes,
        None => random_bytes16(),
    };
    let token_bytes = match json::get_str(request, "token").and_then(|s| parse_hex16(s).ok()) {
        Some(bytes) => bytes,
        None => random_bytes16(),
    };

    let bind_addr = bind
        .to_socket_addrs()
        .map_err(|_| format!("invalid bind address {bind:?}"))?
        .next()
        .ok_or_else(|| format!("bind address {bind:?} resolved to nothing"))?;

    // WASAPI initializes COM on the thread that constructs the capture. The
    // Flutter UI thread may already use a different COM apartment, so defer
    // source and runtime construction to the dedicated host-loop thread.
    let factory = move || {
        #[cfg(windows)]
        let source: Box<dyn audiosync_audio::PcmSource + Send> = if synthetic {
            Box::new(audiosync_audio::SyntheticPcmSource::new(220.0))
        } else {
            let capture = audiosync_audio::WasapiCapture::default_loopback()
                .map_err(|error| format!("WASAPI unavailable: {error}"))?;
            Box::new(capture)
        };
        #[cfg(not(windows))]
        let source: Box<dyn audiosync_audio::PcmSource + Send> = {
            let _ = synthetic;
            Box::new(audiosync_audio::SyntheticPcmSource::new(220.0))
        };

        audiosync_runtime::HostRuntime::new(
            bind_addr,
            session_bytes,
            SessionToken::new(token_bytes),
            max_listeners,
            source,
            audiosync_runtime::monotonic_micros() as u64,
        )
        .map_err(|error| format!("host runtime: {error}"))
    };

    let handle = spawn_host_loop_with_factory(
        factory,
        Box::new(move |_: audiosync_transport::ListenerId| auto_approve),
        HostLoopConfig::default(),
    )
    .map_err(|error| error.to_string())?;

    let bind_address = handle.local_addr().to_string();
    let mut hosts = bridge().hosts.lock().map_err(|_| "bridge lock poisoned")?;
    let slot = hosts
        .iter_mut()
        .enumerate()
        .find(|(_, entry)| entry.is_none())
        .map(|(index, _)| index)
        .ok_or("too many host sessions")?;
    hosts[slot] = Some(HostEntry { handle });

    let mut writer = JsonWriter::new();
    writer.raw("ok", "true");
    writer.raw("handle", &slot.to_string());
    writer.field("bind", &bind_address);
    writer.field("session", &hex16(&session_bytes));
    writer.field("token", &hex16(&token_bytes));
    Ok(writer.finish())
}

/// # Safety
/// `handle_json` must be NUL-terminated UTF-8. Shape:
/// `{"handle":N}` for events/status or
/// `{"handle":N,"command":"approve|remove|end","listener":"<hex32>"}`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn audiosync_ffi_host_command(handle_json: *const c_char) -> *mut c_char {
    let request = match unsafe { decode_json(handle_json) } {
        Some(text) => text,
        None => return alloc_string(error_response("invalid command json")),
    };
    match host_command(&request) {
        Ok(response) => alloc_string(response),
        Err(message) => alloc_string(error_response(&message)),
    }
}

fn host_command(request: &str) -> Result<String, String> {
    let handle: usize = json::get_u64(request, "handle")
        .ok_or("missing handle")?
        .try_into()
        .map_err(|_| "handle out of range")?;
    let hosts = bridge().hosts.lock().map_err(|_| "bridge lock poisoned")?;
    let entry = hosts
        .get(handle)
        .and_then(Option::as_ref)
        .ok_or("unknown host handle")?;

    if let Some(command) = json::get_str(request, "command") {
        match command {
            "approve" => {
                let listener_hex = json::get_str(request, "listener")
                    .ok_or("approve requires a listener hex id")?;
                let bytes = parse_hex16(listener_hex)?;
                entry
                    .handle
                    .send_command(HostCommand::ApproveListener(
                        audiosync_transport::ListenerId::new(bytes),
                    ))
                    .map_err(|error| error.to_string())?;
            }
            "remove" => {
                let listener_hex = json::get_str(request, "listener")
                    .ok_or("remove requires a listener hex id")?;
                let bytes = parse_hex16(listener_hex)?;
                entry
                    .handle
                    .send_command(HostCommand::RemoveListener(
                        audiosync_transport::ListenerId::new(bytes),
                    ))
                    .map_err(|error| error.to_string())?;
            }
            "end" => {
                entry
                    .handle
                    .send_command(HostCommand::EndSession(0))
                    .map_err(|error| error.to_string())?;
            }
            other => return Err(format!("unknown command {other:?}")),
        }
    }

    let events: Vec<String> = entry
        .handle
        .take_events()
        .iter()
        .map(|event| {
            let mut writer = JsonWriter::new();
            writer.field("event", &format!("{event:?}"));
            writer.finish_inner()
        })
        .collect();
    let errors: Vec<String> = entry
        .handle
        .take_errors()
        .iter()
        .map(|error| format!("{error:?}"))
        .collect();

    let mut writer = JsonWriter::new();
    writer.raw("ok", "true");
    writer.raw("running", &entry.handle.is_running().to_string());
    writer.array("events", &events);
    writer.array("errors", &errors);
    Ok(writer.finish())
}

/// # Safety
/// `handle_json` must be NUL-terminated UTF-8 with shape `{"handle":N}`.
/// The host slot is removed even when the loop has already stopped.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn audiosync_ffi_host_stop(handle_json: *const c_char) -> *mut c_char {
    let request = match unsafe { decode_json(handle_json) } {
        Some(text) => text,
        None => return alloc_string(error_response("invalid stop json")),
    };
    match host_stop(&request) {
        Ok(response) => alloc_string(response),
        Err(message) => alloc_string(error_response(&message)),
    }
}

fn host_stop(request: &str) -> Result<String, String> {
    let handle: usize = json::get_u64(request, "handle")
        .ok_or("missing handle")?
        .try_into()
        .map_err(|_| "handle out of range")?;
    let entry = {
        let mut hosts = bridge().hosts.lock().map_err(|_| "bridge lock poisoned")?;
        hosts
            .get_mut(handle)
            .and_then(Option::take)
            .ok_or("unknown host handle")?
    };

    entry.handle.stop();
    Ok("{\"ok\":true}".to_owned())
}

/// # Safety
/// `join_json` must be NUL-terminated UTF-8. Shape:
/// `{"host":"192.168.137.1:41000","session":"<hex>","token":"<hex>",
/// "delay_ms":0,"buffer_ms":100}`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn audiosync_ffi_listener_start(join_json: *const c_char) -> *mut c_char {
    let request = match unsafe { decode_json(join_json) } {
        Some(text) => text,
        None => return alloc_string(error_response("invalid join json")),
    };
    match listener_start(&request) {
        Ok(response) => alloc_string(response),
        Err(message) => alloc_string(error_response(&message)),
    }
}

fn listener_start(request: &str) -> Result<String, String> {
    let host_text = json::get_str(request, "host").ok_or("missing host")?;
    let session_bytes = parse_hex16(json::get_str(request, "session").ok_or("missing session")?)?;
    let token_bytes = parse_hex16(json::get_str(request, "token").ok_or("missing token")?)?;
    let host: std::net::SocketAddr = host_text
        .parse()
        .map_err(|_| format!("invalid host {host_text:?}"))?;
    let delay_ms = json::get_u64(request, "delay_ms").unwrap_or(0);
    let buffer_ms = json::get_u64(request, "buffer_ms").unwrap_or(100).max(20);

    let runtime = ListenerRuntime::new(
        "0.0.0.0:0",
        session_bytes,
        SessionToken::new(token_bytes),
        host,
        audiosync_codec::CodecConfig::default(),
        16,
    )
    .map_err(|error| format!("listener runtime: {error}"))?;
    runtime
        .set_read_timeout(Some(Duration::from_millis(5)))
        .map_err(|error| format!("listener socket: {error}"))?;
    runtime
        .send_join()
        .map_err(|error| format!("join request: {error}"))?;

    let scheduler = PlaybackScheduler::new(PlaybackConfig {
        listener_delay: Duration::from_millis(delay_ms),
        target_buffer: Duration::from_millis(buffer_ms),
        ..PlaybackConfig::default()
    });

    let bind_address = runtime
        .local_addr()
        .map(|address| address.to_string())
        .unwrap_or_default();
    let mut listeners = bridge()
        .listeners
        .lock()
        .map_err(|_| "bridge lock poisoned")?;
    let slot = listeners
        .iter_mut()
        .enumerate()
        .find(|(_, entry)| entry.is_none())
        .map(|(index, _)| index)
        .ok_or("too many listener sessions")?;
    listeners[slot] = Some(ListenerEntry {
        runtime: Mutex::new(runtime),
        scheduler: Mutex::new(scheduler),
        events: Mutex::new(Vec::new()),
    });

    let mut writer = JsonWriter::new();
    writer.raw("ok", "true");
    writer.raw("handle", &slot.to_string());
    writer.field("bind", &bind_address);
    Ok(writer.finish())
}

/// # Safety
/// `handle_json` must be NUL-terminated UTF-8. Shape:
/// `{"handle":N,"delay_ms":40}` (delay optional) — pumps once and returns
/// status. The platform layer calls this periodically (for example from a
/// 50 ms timer) while it owns the audio output path.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn audiosync_ffi_listener_pump(handle_json: *const c_char) -> *mut c_char {
    let request = match unsafe { decode_json(handle_json) } {
        Some(text) => text,
        None => return alloc_string(error_response("invalid pump json")),
    };
    match listener_pump(&request) {
        Ok(response) => alloc_string(response),
        Err(message) => alloc_string(error_response(&message)),
    }
}

fn listener_pump(request: &str) -> Result<String, String> {
    let handle: usize = json::get_u64(request, "handle")
        .ok_or("missing handle")?
        .try_into()
        .map_err(|_| "handle out of range")?;
    let listeners = bridge()
        .listeners
        .lock()
        .map_err(|_| "bridge lock poisoned")?;
    let entry = listeners
        .get(handle)
        .and_then(Option::as_ref)
        .ok_or("unknown listener handle")?;

    let mut runtime = entry.runtime.lock().map_err(|_| "runtime lock poisoned")?;
    let mut scheduler = entry
        .scheduler
        .lock()
        .map_err(|_| "scheduler lock poisoned")?;

    if let Some(delay_ms) = json::get_u64(request, "delay_ms") {
        scheduler.set_listener_delay(Duration::from_millis(delay_ms));
    }

    let mut sink = NullSink::default();
    if let Err(error) = scheduler.pump(&mut runtime, &mut sink) {
        push_event(&entry.events, &format!("playback error: {error}"));
    }
    for event in scheduler.take_control_events() {
        push_event(&entry.events, &format!("{event:?}"));
    }

    let playback = scheduler.stats();
    let pipeline = runtime.pipeline().metrics();
    let session = runtime.session();
    let mut writer = JsonWriter::new();
    writer.raw("ok", "true");
    writer.field("state", &format!("{:?}", session.state()));
    writer.raw(
        "active",
        if session.state() == ListenerState::Active {
            "true"
        } else {
            "false"
        },
    );
    writer.raw("rendered", &playback.rendered_frames.to_string());
    writer.raw("concealed", &playback.concealed_frames.to_string());
    writer.raw("received", &pipeline.received_packets.to_string());
    writer.raw(
        "unauthenticated",
        &pipeline.unauthenticated_packets.to_string(),
    );
    writer.raw(
        "buffered",
        &runtime.pipeline().buffered_packets().to_string(),
    );
    writer.raw(
        "rtt_us",
        &session
            .best_round_trip_micros()
            .map(|v| v.to_string())
            .unwrap_or_else(|| "-1".to_owned()),
    );
    writer.raw(
        "offset_us",
        &session
            .clock_offset_micros()
            .map(|v| v.to_string())
            .unwrap_or("0".to_owned()),
    );
    let drained: Vec<String> = entry
        .events
        .lock()
        .map(|mut events| std::mem::take(&mut *events))
        .unwrap_or_default();
    writer.array("events", &drained);
    Ok(writer.finish())
}

/// # Safety
/// `handle_json` must be NUL-terminated UTF-8 with shape `{"handle":N}`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn audiosync_ffi_listener_leave(handle_json: *const c_char) -> *mut c_char {
    let request = match unsafe { decode_json(handle_json) } {
        Some(text) => text,
        None => return alloc_string(error_response("invalid leave json")),
    };
    let handle: usize = match json::get_u64(&request, "handle") {
        Some(value) => value as usize,
        None => return alloc_string(error_response("missing handle")),
    };
    match listener_leave(handle) {
        Ok(response) => alloc_string(response),
        Err(message) => alloc_string(error_response(&message)),
    }
}

fn listener_leave(handle: usize) -> Result<String, String> {
    let entry = {
        let mut listeners = bridge()
            .listeners
            .lock()
            .map_err(|_| "bridge lock poisoned")?;
        listeners
            .get_mut(handle)
            .and_then(Option::take)
            .ok_or("unknown listener handle")?
    };

    let leave_result = entry
        .runtime
        .lock()
        .map_err(|_| "runtime lock poisoned")?
        .send_leave()
        .map(|_| ());
    drop(entry);
    leave_result.map_err(|error| format!("listener leave: {error}"))?;
    Ok("{\"ok\":true}".to_owned())
}

fn push_event(events: &Mutex<Vec<String>>, text: &str) {
    if let Ok(mut guard) = events.lock() {
        if guard.len() >= 256 {
            guard.drain(..128);
        }
        guard.push(text.to_owned());
    }
}

unsafe fn decode_json(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    unsafe { CStr::from_ptr(ptr) }
        .to_str()
        .ok()
        .map(str::to_owned)
}

fn error_response(message: &str) -> String {
    let mut writer = JsonWriter::new();
    writer.raw("ok", "false");
    writer.field("error", message);
    writer.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;
    use std::time::{Duration, Instant};

    #[test]
    fn json_writer_produces_flat_objects() {
        let mut writer = JsonWriter::new();
        writer.raw("ok", "true");
        writer.field("name", "value with \"quotes\"");
        writer.array("events", &["one".to_owned(), "two".to_owned()]);
        let text = writer.finish();
        assert!(text.starts_with('{') && text.ends_with('}'));
        assert!(text.contains("\"ok\":true"));
        assert!(text.contains("\"events\":[\"one\",\"two\"]"));
    }

    #[test]
    fn json_reader_extracts_values() {
        let text =
            r#"{"bind":"0.0.0.0:41000","max_listeners":5,"synthetic":true,"session":"aabb"}"#;
        assert_eq!(json::get_str(text, "bind"), Some("0.0.0.0:41000"));
        assert_eq!(json::get_u64(text, "max_listeners"), Some(5));
        assert_eq!(json::get_bool(text, "synthetic"), Some(true));
        assert_eq!(json::get_str(text, "session"), Some("aabb"));
        assert_eq!(json::get_str(text, "missing"), None);
    }

    fn call(function: unsafe extern "C" fn(*const c_char) -> *mut c_char, json: &str) -> String {
        let argument = CString::new(json).expect("test json is NUL-safe");
        let raw = unsafe { function(argument.as_ptr()) };
        assert!(!raw.is_null(), "bridge returned null for {json}");
        let result = unsafe { CStr::from_ptr(raw) }
            .to_string_lossy()
            .into_owned();
        unsafe { audiosync_ffi_free_string(raw) };
        result
    }

    #[test]
    fn ffi_bridge_streams_from_host_to_listener_over_the_c_abi() {
        // Start a host with fixed credentials on an OS-assigned port.
        let host_json = call(
            audiosync_ffi_host_start,
            r#"{"bind":"127.0.0.1:0","synthetic":true,"auto_approve":true,"max_listeners":2}"#,
        );
        assert!(host_json.contains("\"ok\":true"), "host start: {host_json}");
        let session = json::get_str(&host_json, "session").expect("session echoed");
        let token = json::get_str(&host_json, "token").expect("token echoed");
        let bind = json::get_str(&host_json, "bind").expect("bind echoed");

        // Join a listener through the C ABI.
        let join = format!(
            r#"{{"host":"{bind}","session":"{session}","token":"{token}","buffer_ms":60}}"#
        );
        let listener_json = call(audiosync_ffi_listener_start, &join);
        assert!(
            listener_json.contains("\"ok\":true"),
            "listener: {listener_json}"
        );

        // Pump until the listener reports it is active and rendering.
        let listener_handle = json::get_u64(&listener_json, "handle").expect("listener handle");
        let pump_json = format!(r#"{{"handle":{listener_handle}}}"#);
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut active = false;
        while Instant::now() < deadline {
            let status = call(audiosync_ffi_listener_pump, &pump_json);
            if status.contains("\"active\":true") {
                active = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(active, "listener never reached active state");

        let deadline = Instant::now() + Duration::from_secs(10);
        let mut rendered = 0_u64;
        while Instant::now() < deadline {
            let status = call(audiosync_ffi_listener_pump, &pump_json);
            if let Some(value) = json::get_u64(&status, "rendered") {
                rendered = value;
                if rendered >= 20 {
                    break;
                }
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(rendered >= 20, "rendered only {rendered} frames");

        // The host reports the join in its event log; then end and leave.
        let host_handle = json::get_u64(&host_json, "handle").expect("host handle");
        let host_command_json = format!(r#"{{"handle":{host_handle}}}"#);
        let host_status = call(audiosync_ffi_host_command, &host_command_json);
        assert!(
            host_status.contains("ListenerJoined"),
            "host events: {host_status}"
        );
        assert!(
            host_status.contains("ListenerApproved"),
            "host approval events: {host_status}"
        );

        let leave_json = format!(r#"{{"handle":{listener_handle}}}"#);
        let leave = call(audiosync_ffi_listener_leave, &leave_json);
        assert!(leave.contains("\"ok\":true"));

        let removal_deadline = Instant::now() + Duration::from_secs(2);
        let mut removal_seen = false;
        while Instant::now() < removal_deadline {
            let status = call(audiosync_ffi_host_command, &host_command_json);
            if status.contains("ListenerRemoved") {
                removal_seen = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(removal_seen, "host did not observe listener leave");

        let listener_again = call(audiosync_ffi_listener_start, &join);
        assert!(
            listener_again.contains("\"ok\":true"),
            "listener slot was not reusable: {listener_again}"
        );
        let listener_again_handle =
            json::get_u64(&listener_again, "handle").expect("reused listener handle");
        let leave_again = call(
            audiosync_ffi_listener_leave,
            &format!(r#"{{"handle":{listener_again_handle}}}"#),
        );
        assert!(
            leave_again.contains("\"ok\":true"),
            "reused listener leave: {leave_again}"
        );

        let stop = call(
            audiosync_ffi_host_stop,
            &format!(r#"{{"handle":{host_handle}}}"#),
        );
        assert!(stop.contains("\"ok\":true"), "host stop: {stop}");
    }

    #[test]
    fn ffi_host_slots_are_reusable_after_stop() {
        for _ in 0..10 {
            let host_json = call(
                audiosync_ffi_host_start,
                r#"{"bind":"127.0.0.1:0","synthetic":true,"max_listeners":1}"#,
            );
            assert!(host_json.contains("\"ok\":true"), "host start: {host_json}");
            let handle = json::get_u64(&host_json, "handle").expect("host handle");
            let stop = call(
                audiosync_ffi_host_stop,
                &format!(r#"{{"handle":{handle}}}"#),
            );
            assert!(stop.contains("\"ok\":true"), "host stop: {stop}");
        }
    }

    #[test]
    fn ffi_bridge_rejects_malformed_requests() {
        let response = call(audiosync_ffi_host_start, "not json at all");
        assert!(
            response.contains("\"ok\":false"),
            "garbage input: {response}"
        );
        let listener = call(audiosync_ffi_listener_start, r#"{"host":"nope"}"#);
        assert!(listener.contains("\"ok\":false"), "listener: {listener}");
    }
}
