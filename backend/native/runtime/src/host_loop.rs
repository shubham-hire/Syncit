//! Single-threaded host scheduler for one [`HostRuntime`].
//!
//! The loop owns the runtime on one thread: it receives control datagrams
//! between audio ticks, runs the 20 ms audio cadence from a drift-corrected
//! anchor, expires stale listeners periodically, and executes commands sent
//! through a channel by the UI or CLI (approve, remove, end session). Keeping
//! everything on one thread avoids lock contention jitter on the audio path
//! and keeps the runtime's socket-free session core unchanged.

use crate::{HostRuntime, HostRuntimeError};
use audiosync_audio::PcmSource;
use audiosync_session::SessionEvent;
use audiosync_transport::ListenerId;
use std::fmt;
use std::io;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// Commands a running host loop executes between control receives.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostCommand {
    ApproveListener(ListenerId),
    RemoveListener(ListenerId),
    EndSession(u8),
}

/// Tuning knobs for the host loop's timing behavior.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostLoopConfig {
    /// Audio cadence. Must match the codec frame duration.
    pub audio_tick_micros: u64,
    /// Minimum socket wait so a busy loop never spins without yielding.
    pub control_wait_floor: Duration,
    /// How often stale-listener expiry runs.
    pub expire_period: Duration,
    /// How long without a heartbeat before a listener is expired.
    pub listener_timeout: Duration,
    /// Upper bound on one socket wait so the stop flag stays responsive.
    pub max_socket_wait: Duration,
}

impl Default for HostLoopConfig {
    fn default() -> Self {
        Self {
            audio_tick_micros: 20_000,
            control_wait_floor: Duration::from_millis(1),
            expire_period: Duration::from_secs(1),
            listener_timeout: Duration::from_secs(5),
            max_socket_wait: Duration::from_millis(50),
        }
    }
}

/// Errors returned by the host-loop handle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommandChannelClosed;

impl fmt::Display for CommandChannelClosed {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "the host loop command channel is closed")
    }
}

impl std::error::Error for CommandChannelClosed {}

/// Host-loop-specific failures.
#[derive(Debug)]
pub enum HostLoopError {
    Command(CommandChannelClosed),
    Runtime(HostRuntimeError),
}

impl fmt::Display for HostLoopError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Command(error) => write!(formatter, "{error}"),
            Self::Runtime(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for HostLoopError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Command(error) => Some(error),
            Self::Runtime(error) => Some(error),
        }
    }
}

impl From<CommandChannelClosed> for HostLoopError {
    fn from(error: CommandChannelClosed) -> Self {
        Self::Command(error)
    }
}

impl From<HostRuntimeError> for HostLoopError {
    fn from(error: HostRuntimeError) -> Self {
        Self::Runtime(error)
    }
}

/// Errors reported while constructing a host runtime on its scheduler thread.
#[derive(Debug)]
pub enum HostLoopStartError {
    /// The runtime factory could not construct the source or runtime.
    Factory(String),
    /// The runtime could not finish its socket setup on the scheduler thread.
    Runtime(HostRuntimeError),
    /// The scheduler thread exited before reporting its startup result.
    Thread,
}

impl fmt::Display for HostLoopStartError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Factory(message) => write!(formatter, "{message}"),
            Self::Runtime(error) => write!(formatter, "{error}"),
            Self::Thread => write!(formatter, "the host loop exited during startup"),
        }
    }
}

impl std::error::Error for HostLoopStartError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Runtime(error) => Some(error),
            Self::Factory(_) | Self::Thread => None,
        }
    }
}

const MAX_RETAINED_ITEMS: usize = 1_024;
const RETAIN_AFTER_OVERFLOW: usize = 512;

struct HostLoopShared {
    stop: AtomicBool,
    events: Mutex<Vec<SessionEvent>>,
    errors: Mutex<Vec<HostRuntimeError>>,
}

impl HostLoopShared {
    fn new() -> Self {
        Self {
            stop: AtomicBool::new(false),
            events: Mutex::new(Vec::new()),
            errors: Mutex::new(Vec::new()),
        }
    }

    fn push_event(&self, event: SessionEvent) {
        let Ok(mut guard) = self.events.lock() else {
            return;
        };
        if guard.len() >= MAX_RETAINED_ITEMS {
            guard.drain(..RETAIN_AFTER_OVERFLOW);
        }
        guard.push(event);
    }

    fn push_error(&self, error: HostRuntimeError) {
        let Ok(mut guard) = self.errors.lock() else {
            return;
        };
        if guard.len() >= MAX_RETAINED_ITEMS {
            guard.drain(..RETAIN_AFTER_OVERFLOW);
        }
        guard.push(error);
    }
}

/// Handle to a running host loop.
pub struct HostLoopHandle {
    shared: Arc<HostLoopShared>,
    commands: Sender<HostCommand>,
    thread: Option<JoinHandle<()>>,
    local_addr: SocketAddr,
}

impl fmt::Debug for HostLoopHandle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HostLoopHandle")
            .field("local_addr", &self.local_addr)
            .field("running", &self.is_running())
            .finish_non_exhaustive()
    }
}

impl HostLoopHandle {
    /// The address the host's single control/audio socket is bound to.
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Queues a command for the loop. Commands execute within one socket wait
    /// of roughly `max_socket_wait` at the latest.
    pub fn send_command(&self, command: HostCommand) -> Result<(), HostLoopError> {
        self.commands
            .send(command)
            .map_err(|_| HostLoopError::Command(CommandChannelClosed))
    }

    /// Drains retained session events (joins, approvals, timeouts, ...).
    pub fn take_events(&self) -> Vec<SessionEvent> {
        let Ok(mut guard) = self.shared.events.lock() else {
            return Vec::new();
        };
        std::mem::take(&mut *guard)
    }

    /// Drains retained runtime errors, including audio pump failures.
    pub fn take_errors(&self) -> Vec<HostRuntimeError> {
        let Ok(mut guard) = self.shared.errors.lock() else {
            return Vec::new();
        };
        std::mem::take(&mut *guard)
    }

    pub fn is_running(&self) -> bool {
        !self.shared.stop.load(Ordering::Relaxed)
    }

    /// Requests a stop and waits for the loop thread to finish. Sending
    /// [`HostCommand::EndSession`] first produces an orderly shutdown.
    pub fn stop(mut self) {
        let _ = self.commands.send(HostCommand::EndSession(0));
        let deadline = Instant::now() + Duration::from_millis(250);
        while self.is_running() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(1));
        }
        self.shared.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for HostLoopHandle {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Policy deciding whether a newly joined listener is approved immediately.
pub type ApprovalPolicy = Box<dyn Fn(ListenerId) -> bool + Send>;

/// Spawns the host loop for `runtime` on a background thread.
///
/// The `approval` closure runs on the loop thread whenever a listener joins;
/// returning `true` approves the listener right away (auto-approve). Manual
/// approval is available later through [`HostCommand::ApproveListener`] after
/// inspecting [`HostLoopHandle::take_events`].
pub fn spawn_host_loop<S>(
    runtime: HostRuntime<S>,
    approval: ApprovalPolicy,
    config: HostLoopConfig,
) -> Result<HostLoopHandle, HostLoopError>
where
    S: PcmSource + Send + 'static,
{
    let local_addr = runtime.local_addr()?;
    runtime.set_read_timeout(Some(config.control_wait_floor))?;
    let shared = Arc::new(HostLoopShared::new());
    let (command_sender, command_receiver) = mpsc::channel::<HostCommand>();
    let thread_shared = Arc::clone(&shared);
    let thread =
        thread::spawn(move || run_loop(runtime, approval, config, thread_shared, command_receiver));

    Ok(HostLoopHandle {
        shared,
        commands: command_sender,
        thread: Some(thread),
        local_addr,
    })
}

/// Constructs and starts a host runtime on the scheduler thread.
///
/// This variant is important for platform sources such as WASAPI: COM and
/// other thread-affine resources must be initialized on the same thread that
/// will use and drop them. The caller blocks until the factory has either
/// produced a bound runtime or reported an initialization error.
pub fn spawn_host_loop_with_factory<S, F>(
    factory: F,
    approval: ApprovalPolicy,
    config: HostLoopConfig,
) -> Result<HostLoopHandle, HostLoopStartError>
where
    S: PcmSource + Send + 'static,
    F: FnOnce() -> Result<HostRuntime<S>, String> + Send + 'static,
{
    let shared = Arc::new(HostLoopShared::new());
    let (command_sender, command_receiver) = mpsc::channel::<HostCommand>();
    let (startup_sender, startup_receiver) = mpsc::sync_channel(1);
    let thread_shared = Arc::clone(&shared);
    let thread = thread::spawn(move || {
        let runtime = match factory() {
            Ok(runtime) => runtime,
            Err(message) => {
                let _ = startup_sender.send(Err(HostLoopStartError::Factory(message)));
                return;
            }
        };

        let local_addr = match runtime.local_addr() {
            Ok(local_addr) => local_addr,
            Err(error) => {
                let _ = startup_sender.send(Err(HostLoopStartError::Runtime(error)));
                return;
            }
        };
        if let Err(error) = runtime.set_read_timeout(Some(config.control_wait_floor)) {
            let _ = startup_sender.send(Err(HostLoopStartError::Runtime(error)));
            return;
        }

        if startup_sender.send(Ok(local_addr)).is_err() {
            return;
        }
        run_loop(runtime, approval, config, thread_shared, command_receiver);
    });

    let local_addr = match startup_receiver.recv() {
        Ok(Ok(local_addr)) => local_addr,
        Ok(Err(error)) => {
            let _ = thread.join();
            return Err(error);
        }
        Err(_) => {
            let _ = thread.join();
            return Err(HostLoopStartError::Thread);
        }
    };

    Ok(HostLoopHandle {
        shared,
        commands: command_sender,
        thread: Some(thread),
        local_addr,
    })
}

fn run_loop<S: PcmSource>(
    mut runtime: HostRuntime<S>,
    approval: ApprovalPolicy,
    config: HostLoopConfig,
    shared: Arc<HostLoopShared>,
    commands: Receiver<HostCommand>,
) {
    let start = Instant::now();
    let mut tick: u64 = 0;
    let mut next_expire = start + config.expire_period;

    while !shared.stop.load(Ordering::Relaxed) {
        execute_commands(&mut runtime, &shared, &commands);

        let now = Instant::now();
        let next_tick = start
            + Duration::from_micros(
                tick.saturating_add(1)
                    .saturating_mul(config.audio_tick_micros),
            );
        let wait = if next_tick > now {
            (next_tick - now).min(config.max_socket_wait)
        } else {
            config.control_wait_floor
        }
        .max(config.control_wait_floor);
        if runtime.set_read_timeout(Some(wait)).is_ok() {
            match runtime.receive_control_once_realtime(Instant::now()) {
                Ok(events) => {
                    for event in events {
                        let auto_approve = match &event {
                            SessionEvent::ListenerJoined { listener_id, .. }
                                if approval(*listener_id) =>
                            {
                                Some(*listener_id)
                            }
                            _ => None,
                        };
                        shared.push_event(event);

                        if let Some(listener_id) = auto_approve {
                            match runtime.approve_listener(listener_id) {
                                Ok(approval_events) => {
                                    for approval_event in approval_events {
                                        shared.push_event(approval_event);
                                    }
                                }
                                Err(error) => shared.push_error(error),
                            }
                        }
                    }
                }
                Err(HostRuntimeError::Io(error))
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                    ) => {}
                Err(error) => shared.push_error(error),
            }
        }

        let now = Instant::now();
        if now >= next_tick {
            tick = tick.saturating_add(1);
            if let Err(error) = runtime.send_next_audio() {
                shared.push_error(error);
            }
        }

        let now = Instant::now();
        if now >= next_expire {
            match runtime.expire_stale(now, config.listener_timeout) {
                Ok(events) => {
                    for event in events {
                        shared.push_event(event);
                    }
                }
                Err(error) => shared.push_error(error),
            }
            next_expire = now + config.expire_period;
        }
    }
}

fn execute_commands<S: PcmSource>(
    runtime: &mut HostRuntime<S>,
    shared: &HostLoopShared,
    commands: &Receiver<HostCommand>,
) {
    loop {
        match commands.try_recv() {
            Ok(HostCommand::ApproveListener(listener_id)) => {
                match runtime.approve_listener(listener_id) {
                    Ok(events) => {
                        for event in events {
                            shared.push_event(event);
                        }
                    }
                    Err(error) => shared.push_error(error),
                }
            }
            Ok(HostCommand::RemoveListener(listener_id)) => {
                match runtime.remove_listener(listener_id) {
                    Ok(events) => {
                        for event in events {
                            shared.push_event(event);
                        }
                    }
                    Err(error) => shared.push_error(error),
                }
            }
            Ok(HostCommand::EndSession(reason)) => {
                match runtime.end_session(reason) {
                    Ok(events) => {
                        for event in events {
                            shared.push_event(event);
                        }
                        // EndSession has already dispatched the terminal
                        // notifications; stop the loop so its owner can
                        // release the handle and socket deterministically.
                        shared.stop.store(true, Ordering::Relaxed);
                    }
                    Err(error) => shared.push_error(error),
                }
                return;
            }
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => {
                // The handle is gone; stop gracefully at the next check.
                shared.stop.store(true, Ordering::Relaxed);
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ListenerRuntime, NullSink, PlaybackConfig, PlaybackScheduler};
    use audiosync_audio::SyntheticPcmSource;
    use audiosync_codec::CodecConfig;
    use audiosync_session::ListenerState;
    use audiosync_transport::SessionToken;

    fn session_id() -> audiosync_core::SessionId {
        [0x81; 16]
    }

    fn token() -> SessionToken {
        SessionToken::new([0x82; 16])
    }

    fn auto_approve_all() -> ApprovalPolicy {
        Box::new(|_: ListenerId| true)
    }

    fn spawn_test_host(listener_timeout: Duration) -> HostLoopHandle {
        let runtime = HostRuntime::new(
            "127.0.0.1:0",
            session_id(),
            token(),
            4,
            SyntheticPcmSource::new(220.0),
            1_000_000,
        )
        .expect("host runtime should initialize");
        let config = HostLoopConfig {
            listener_timeout,
            ..HostLoopConfig::default()
        };
        spawn_host_loop(runtime, auto_approve_all(), config).expect("host loop should spawn")
    }

    #[test]
    fn factory_constructs_the_runtime_on_the_host_loop_thread() {
        let caller_thread = thread::current().id();
        let (constructed_sender, constructed_receiver) = mpsc::channel();
        let host = spawn_host_loop_with_factory(
            move || {
                constructed_sender
                    .send(thread::current().id())
                    .expect("startup receiver should remain available");
                HostRuntime::new(
                    "127.0.0.1:0",
                    session_id(),
                    token(),
                    1,
                    SyntheticPcmSource::new(220.0),
                    1_000_000,
                )
                .map_err(|error| error.to_string())
            },
            auto_approve_all(),
            HostLoopConfig::default(),
        )
        .expect("factory host loop should start");

        let constructed_thread = constructed_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("factory should run before startup returns");
        assert_ne!(constructed_thread, caller_thread);
        host.stop();
    }

    fn wait_until(deadline_ms: u64, mut condition: impl FnMut() -> bool) -> bool {
        let deadline = Instant::now() + Duration::from_millis(deadline_ms);
        while Instant::now() < deadline {
            if condition() {
                return true;
            }
            thread::sleep(Duration::from_millis(10));
        }
        condition()
    }

    /// Pumps the listener until `expected` frames have been rendered.
    fn pump_until(
        scheduler: &mut PlaybackScheduler,
        runtime: &mut ListenerRuntime,
        sink: &mut NullSink,
        expected: u64,
        deadline_ms: u64,
    ) -> bool {
        wait_until(deadline_ms, || {
            let _ = scheduler.pump(runtime, sink);
            sink.frames_written >= expected
        })
    }

    #[test]
    fn host_loop_streams_synthetic_audio_to_a_listener() {
        let host = spawn_test_host(Duration::from_secs(5));
        let host_endpoint = host.local_addr();
        let mut listener = ListenerRuntime::new(
            "127.0.0.1:0",
            session_id(),
            token(),
            host_endpoint,
            CodecConfig::default(),
            16,
        )
        .expect("listener runtime should initialize");
        listener
            .set_read_timeout(Some(Duration::from_millis(2)))
            .expect("listener timeout should configure");

        listener.send_join().expect("join should send");
        let mut scheduler = PlaybackScheduler::new(PlaybackConfig::default());
        let mut sink = NullSink::default();
        assert!(
            pump_until(&mut scheduler, &mut listener, &mut sink, 40, 20_000),
            "expected at least 40 rendered frames, got {}",
            sink.frames_written
        );
        assert!(scheduler.stats().concealed_frames <= 2);
        assert!(
            host.take_events()
                .iter()
                .any(|event| matches!(event, SessionEvent::ListenerJoined { .. })),
            "host should retain the join event"
        );

        host.send_command(HostCommand::EndSession(1))
            .expect("end command should queue");
        assert!(wait_until(2_000, || {
            let _ = scheduler.pump(&mut listener, &mut sink);
            listener.session().state() == ListenerState::SessionEnded
        }));
        host.stop();
    }

    #[test]
    fn host_loop_removes_a_listener_on_command() {
        let host = spawn_test_host(Duration::from_secs(5));
        let mut listener = ListenerRuntime::new(
            "127.0.0.1:0",
            session_id(),
            token(),
            host.local_addr(),
            CodecConfig::default(),
            8,
        )
        .expect("listener runtime should initialize");
        listener
            .set_read_timeout(Some(Duration::from_millis(2)))
            .expect("listener timeout should configure");
        listener.send_join().expect("join should send");

        let mut scheduler = PlaybackScheduler::new(PlaybackConfig::default());
        let mut sink = NullSink::default();
        assert!(wait_until(5_000, || {
            let _ = scheduler.pump(&mut listener, &mut sink);
            listener.session().state() == ListenerState::Active
        }));
        assert!(
            host.take_events()
                .iter()
                .any(|event| matches!(event, SessionEvent::ListenerApproved { .. })),
            "host should retain the approval event"
        );

        let listener_id = listener
            .session()
            .listener_id()
            .expect("listener should have an ID after approval");
        host.send_command(HostCommand::RemoveListener(listener_id))
            .expect("remove command should queue");
        assert!(wait_until(5_000, || {
            let _ = scheduler.pump(&mut listener, &mut sink);
            listener.session().state() == ListenerState::Removed
        }));
        assert!(
            host.take_events()
                .iter()
                .any(|event| matches!(event, SessionEvent::ListenerRemoved { .. })),
            "host should retain the removal event"
        );
        host.stop();
    }

    #[test]
    fn host_loop_expires_a_silent_listener() {
        let host = spawn_test_host(Duration::from_millis(400));
        let mut listener = ListenerRuntime::new(
            "127.0.0.1:0",
            session_id(),
            token(),
            host.local_addr(),
            CodecConfig::default(),
            8,
        )
        .expect("listener runtime should initialize");
        listener
            .set_read_timeout(Some(Duration::from_millis(2)))
            .expect("listener timeout should configure");
        listener.send_join().expect("join should send");

        // Reach active state, then stop heartbeating by pumping without the
        // scheduler's heartbeat cadence: read-only drains keep no session
        // traffic flowing.
        let mut scheduler = PlaybackScheduler::new(PlaybackConfig::default());
        let mut sink = NullSink::default();
        assert!(wait_until(5_000, || {
            let _ = scheduler.pump(&mut listener, &mut sink);
            listener.session().state() == ListenerState::Active
        }));
        drop(scheduler);

        assert!(
            wait_until(5_000, || host.take_events().iter().any(|event| matches!(
                event,
                SessionEvent::ListenerTimedOut { .. }
            ))),
            "host should expire the silent listener"
        );
        host.stop();
    }
}
