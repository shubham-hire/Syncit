//! Platform-neutral host and listener session state for SyncIt.
//!
//! The session layer deliberately does not own sockets, threads, or clocks.
//! Callers feed decoded control messages into the state machines and send the
//! returned [`SessionAction::Outbound`] messages through their chosen runtime.

use audiosync_core::{ClockEstimator, ClockSample, SessionId};
use audiosync_transport::{CONTROL_PROTOCOL_VERSION, ControlMessage, ListenerId, SessionToken};
use std::collections::HashMap;
use std::fmt;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

/// Reason codes sent in a [`ControlMessage::JoinRejected`] message.
pub const REJECT_UNKNOWN_SESSION: u8 = 1;
pub const REJECT_INVALID_TOKEN: u8 = 2;
pub const REJECT_MAX_LISTENERS: u8 = 3;
pub const REJECT_SESSION_ENDED: u8 = 4;

/// The externally visible state of a listener session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ListenerState {
    /// No join has been accepted for this listener yet.
    Disconnected,
    /// The host has assigned an ID, but has not approved the listener.
    PendingApproval,
    /// The listener can receive session traffic.
    Active,
    /// The host rejected the most recent join request.
    Rejected,
    /// The host explicitly removed this listener.
    Removed,
    /// The host stopped responding before the listener rejoined.
    TimedOut,
    /// The host ended the session.
    SessionEnded,
}

/// Host-side state retained for an endpoint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostListenerState {
    PendingApproval,
    Active,
    Removed,
    TimedOut,
}

/// Events emitted by the state machines for logging, UI updates, and metrics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SessionEvent {
    ListenerJoined {
        endpoint: SocketAddr,
        listener_id: ListenerId,
    },
    ListenerReconnected {
        endpoint: SocketAddr,
        listener_id: ListenerId,
    },
    ListenerApproved {
        endpoint: SocketAddr,
        listener_id: ListenerId,
    },
    ListenerRemoved {
        endpoint: SocketAddr,
        listener_id: ListenerId,
    },
    ListenerTimedOut {
        endpoint: SocketAddr,
        listener_id: ListenerId,
    },
    HeartbeatReceived {
        endpoint: SocketAddr,
        listener_id: ListenerId,
    },
    ClockSyncRequestReceived {
        endpoint: SocketAddr,
        listener_id: ListenerId,
        client_sent_micros: i64,
    },
    ClockSyncUpdated {
        offset_micros: i64,
        round_trip_micros: i64,
        estimate_changed: bool,
    },
    JoinRejected {
        endpoint: SocketAddr,
        reason: u8,
    },
    MessageIgnored {
        endpoint: SocketAddr,
        reason: IgnoreReason,
    },
    StateChanged(ListenerState),
}

/// Why a message was ignored rather than applied to session state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IgnoreReason {
    WrongSession,
    UnknownEndpoint,
    UnknownListener,
    WrongPeer,
    InvalidDirection,
    InvalidState,
}

/// A state transition result for a session caller.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SessionAction {
    /// The caller should encode and send this control message.
    Outbound {
        destination: SocketAddr,
        message: ControlMessage,
    },
    /// The caller should publish or record this event.
    Event(SessionEvent),
}

/// Counters for control-plane behavior. Audio loss counters belong to the
/// transport/audio pipeline and are intentionally not mixed into this type.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SessionMetrics {
    pub join_requests: u64,
    pub join_accepted: u64,
    pub join_rejected: u64,
    pub duplicate_joins: u64,
    pub approvals: u64,
    pub removals: u64,
    pub heartbeats: u64,
    pub clock_sync_requests: u64,
    pub clock_sync_responses: u64,
    pub reconnects: u64,
    pub timeouts: u64,
    pub invalid_messages: u64,
    pub unknown_endpoints: u64,
    pub active_listeners: usize,
}

/// Errors returned when a caller asks a state machine to perform an invalid
/// direct operation. Invalid network input is handled as an event instead of
/// panicking.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SessionError {
    UnknownListener(ListenerId),
    UnknownEndpoint(SocketAddr),
    WrongPeer {
        expected: SocketAddr,
        actual: SocketAddr,
    },
    InvalidSession {
        expected: SessionId,
        actual: SessionId,
    },
    InvalidState {
        expected: &'static str,
        actual: ListenerState,
    },
    ListenerIdMismatch {
        expected: ListenerId,
        actual: ListenerId,
    },
    MissingClientReceiveTimestamp,
    InvalidClockSample,
}

impl fmt::Display for SessionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownListener(listener_id) => {
                write!(formatter, "unknown listener {listener_id:?}")
            }
            Self::UnknownEndpoint(endpoint) => write!(formatter, "unknown endpoint {endpoint}"),
            Self::WrongPeer { expected, actual } => {
                write!(formatter, "message came from {actual}, expected {expected}")
            }
            Self::InvalidSession { expected, actual } => {
                write!(
                    formatter,
                    "wrong session: expected {expected:?}, got {actual:?}"
                )
            }
            Self::InvalidState { expected, actual } => {
                write!(
                    formatter,
                    "invalid listener state: expected {expected}, got {actual:?}"
                )
            }
            Self::ListenerIdMismatch { expected, actual } => write!(
                formatter,
                "listener ID mismatch: expected {expected:?}, got {actual:?}"
            ),
            Self::MissingClientReceiveTimestamp => {
                write!(formatter, "a client receive timestamp is required")
            }
            Self::InvalidClockSample => write!(formatter, "clock sample is invalid"),
        }
    }
}

impl std::error::Error for SessionError {}

#[derive(Clone, Debug)]
struct HostListener {
    listener_id: ListenerId,
    endpoint: SocketAddr,
    state: HostListenerState,
    last_seen: Instant,
    generation: u64,
}

/// Host-side session state. The host owns the approval decision and maps a
/// network endpoint to one stable listener ID for the lifetime of the session.
#[derive(Clone, Debug)]
pub struct HostSession {
    session_id: SessionId,
    token: SessionToken,
    max_listeners: usize,
    next_listener_serial: u128,
    listeners: HashMap<SocketAddr, HostListener>,
    metrics: SessionMetrics,
    ended: bool,
}

impl HostSession {
    pub fn new(session_id: SessionId, token: SessionToken, max_listeners: usize) -> Self {
        Self {
            session_id,
            token,
            max_listeners,
            next_listener_serial: 1,
            listeners: HashMap::new(),
            metrics: SessionMetrics::default(),
            ended: false,
        }
    }

    pub const fn session_id(&self) -> SessionId {
        self.session_id
    }

    pub const fn token(&self) -> SessionToken {
        self.token
    }

    pub const fn max_listeners(&self) -> usize {
        self.max_listeners
    }

    pub fn metrics(&self) -> &SessionMetrics {
        &self.metrics
    }

    pub fn listener_state(&self, listener_id: ListenerId) -> Option<HostListenerState> {
        self.listeners
            .values()
            .find(|listener| listener.listener_id == listener_id)
            .map(|listener| listener.state)
    }

    pub fn listener_id_for(&self, endpoint: SocketAddr) -> Option<ListenerId> {
        self.listeners
            .get(&endpoint)
            .map(|listener| listener.listener_id)
    }

    pub fn listener_endpoint(&self, listener_id: ListenerId) -> Option<SocketAddr> {
        self.listeners
            .values()
            .find(|listener| listener.listener_id == listener_id)
            .map(|listener| listener.endpoint)
    }

    /// Returns the endpoints that currently have permission to receive audio.
    ///
    /// The returned list is sorted to keep runtime fan-out and tests
    /// deterministic even though the host stores listeners in a hash map.
    pub fn active_listener_endpoints(&self) -> Vec<SocketAddr> {
        let mut endpoints: Vec<SocketAddr> = self
            .listeners
            .values()
            .filter(|listener| listener.state == HostListenerState::Active)
            .map(|listener| listener.endpoint)
            .collect();
        endpoints.sort_unstable();
        endpoints
    }

    /// Handles a decoded message received from a UDP endpoint.
    ///
    /// Clock-sync requests are recorded and surfaced as an event. Callers that
    /// have access to receive/send timestamps should then call
    /// [`Self::respond_to_clock_sync`].
    pub fn handle_message(
        &mut self,
        source: SocketAddr,
        message: ControlMessage,
        now: Instant,
    ) -> Vec<SessionAction> {
        let message_session = message.session_id();
        if matches!(&message, ControlMessage::JoinRequest { .. }) {
            self.metrics.join_requests = self.metrics.join_requests.saturating_add(1);
        }

        if message_session != self.session_id {
            self.metrics.invalid_messages = self.metrics.invalid_messages.saturating_add(1);
            if matches!(&message, ControlMessage::JoinRequest { .. }) {
                return self.reject_join(source, message_session, REJECT_UNKNOWN_SESSION);
            }
            return vec![SessionAction::Event(SessionEvent::MessageIgnored {
                endpoint: source,
                reason: IgnoreReason::WrongSession,
            })];
        }

        match message {
            ControlMessage::JoinRequest { token, .. } => self.handle_join(source, token, now),
            ControlMessage::Heartbeat { .. } => self.handle_heartbeat(source, now),
            ControlMessage::ClockSyncRequest {
                client_sent_micros, ..
            } => self.handle_clock_sync_request(source, client_sent_micros, now),
            ControlMessage::JoinPending { .. }
            | ControlMessage::JoinAccepted { .. }
            | ControlMessage::JoinRejected { .. }
            | ControlMessage::ListenerApproved { .. }
            | ControlMessage::ListenerRemoved { .. }
            | ControlMessage::ClockSyncResponse { .. }
            | ControlMessage::SessionEnded { .. } => {
                self.metrics.invalid_messages = self.metrics.invalid_messages.saturating_add(1);
                vec![SessionAction::Event(SessionEvent::MessageIgnored {
                    endpoint: source,
                    reason: IgnoreReason::InvalidDirection,
                })]
            }
        }
    }

    fn handle_join(
        &mut self,
        source: SocketAddr,
        token: SessionToken,
        now: Instant,
    ) -> Vec<SessionAction> {
        if self.ended {
            return self.reject_join(source, self.session_id, REJECT_SESSION_ENDED);
        }

        if token != self.token {
            self.metrics.invalid_messages = self.metrics.invalid_messages.saturating_add(1);
            return self.reject_join(source, self.session_id, REJECT_INVALID_TOKEN);
        }

        if let Some(listener) = self.listeners.get_mut(&source) {
            let listener_id = listener.listener_id;
            let was_reconnect = matches!(
                listener.state,
                HostListenerState::TimedOut | HostListenerState::Removed
            );
            listener.last_seen = now;
            listener.generation = listener.generation.saturating_add(1);
            if was_reconnect {
                listener.state = HostListenerState::PendingApproval;
                self.metrics.reconnects = self.metrics.reconnects.saturating_add(1);
            }

            self.metrics.duplicate_joins = self.metrics.duplicate_joins.saturating_add(1);
            self.metrics.join_accepted = self.metrics.join_accepted.saturating_add(1);
            let mut actions = vec![SessionAction::Outbound {
                destination: source,
                message: ControlMessage::JoinAccepted {
                    session_id: self.session_id,
                    listener_id,
                },
            }];
            if was_reconnect {
                actions.push(SessionAction::Event(SessionEvent::ListenerReconnected {
                    endpoint: source,
                    listener_id,
                }));
            }
            if listener.state == HostListenerState::Active {
                actions.push(SessionAction::Outbound {
                    destination: source,
                    message: ControlMessage::ListenerApproved {
                        session_id: self.session_id,
                        listener_id,
                    },
                });
            }
            return actions;
        }

        if self.connected_listener_count() >= self.max_listeners {
            return self.reject_join(source, self.session_id, REJECT_MAX_LISTENERS);
        }

        let listener_id = self.allocate_listener_id();
        self.listeners.insert(
            source,
            HostListener {
                listener_id,
                endpoint: source,
                state: HostListenerState::PendingApproval,
                last_seen: now,
                generation: 1,
            },
        );
        self.metrics.join_accepted = self.metrics.join_accepted.saturating_add(1);
        vec![
            SessionAction::Outbound {
                destination: source,
                message: ControlMessage::JoinAccepted {
                    session_id: self.session_id,
                    listener_id,
                },
            },
            SessionAction::Event(SessionEvent::ListenerJoined {
                endpoint: source,
                listener_id,
            }),
        ]
    }

    fn reject_join(
        &mut self,
        destination: SocketAddr,
        session_id: SessionId,
        reason: u8,
    ) -> Vec<SessionAction> {
        self.metrics.join_rejected = self.metrics.join_rejected.saturating_add(1);
        vec![
            SessionAction::Outbound {
                destination,
                message: ControlMessage::JoinRejected { session_id, reason },
            },
            SessionAction::Event(SessionEvent::JoinRejected {
                endpoint: destination,
                reason,
            }),
        ]
    }

    fn handle_heartbeat(&mut self, source: SocketAddr, now: Instant) -> Vec<SessionAction> {
        let Some(listener) = self.listeners.get_mut(&source) else {
            self.metrics.unknown_endpoints = self.metrics.unknown_endpoints.saturating_add(1);
            return vec![SessionAction::Event(SessionEvent::MessageIgnored {
                endpoint: source,
                reason: IgnoreReason::UnknownEndpoint,
            })];
        };

        if matches!(
            listener.state,
            HostListenerState::Removed | HostListenerState::TimedOut
        ) {
            self.metrics.invalid_messages = self.metrics.invalid_messages.saturating_add(1);
            return vec![SessionAction::Event(SessionEvent::MessageIgnored {
                endpoint: source,
                reason: IgnoreReason::InvalidState,
            })];
        }

        listener.last_seen = now;
        self.metrics.heartbeats = self.metrics.heartbeats.saturating_add(1);
        vec![SessionAction::Event(SessionEvent::HeartbeatReceived {
            endpoint: source,
            listener_id: listener.listener_id,
        })]
    }

    fn handle_clock_sync_request(
        &mut self,
        source: SocketAddr,
        client_sent_micros: i64,
        now: Instant,
    ) -> Vec<SessionAction> {
        let Some(listener) = self.listeners.get_mut(&source) else {
            self.metrics.unknown_endpoints = self.metrics.unknown_endpoints.saturating_add(1);
            return vec![SessionAction::Event(SessionEvent::MessageIgnored {
                endpoint: source,
                reason: IgnoreReason::UnknownEndpoint,
            })];
        };
        if matches!(
            listener.state,
            HostListenerState::Removed | HostListenerState::TimedOut
        ) {
            self.metrics.invalid_messages = self.metrics.invalid_messages.saturating_add(1);
            return vec![SessionAction::Event(SessionEvent::MessageIgnored {
                endpoint: source,
                reason: IgnoreReason::InvalidState,
            })];
        }

        listener.last_seen = now;
        self.metrics.clock_sync_requests = self.metrics.clock_sync_requests.saturating_add(1);
        vec![SessionAction::Event(
            SessionEvent::ClockSyncRequestReceived {
                endpoint: source,
                listener_id: listener.listener_id,
                client_sent_micros,
            },
        )]
    }

    /// Creates a clock-sync response after the caller records host receive and
    /// send timestamps from its monotonic clock.
    pub fn respond_to_clock_sync(
        &mut self,
        source: SocketAddr,
        client_sent_micros: i64,
        host_received_micros: i64,
        host_sent_micros: i64,
        now: Instant,
    ) -> Result<Vec<SessionAction>, SessionError> {
        let Some(listener) = self.listeners.get_mut(&source) else {
            self.metrics.unknown_endpoints = self.metrics.unknown_endpoints.saturating_add(1);
            return Err(SessionError::UnknownEndpoint(source));
        };
        if matches!(
            listener.state,
            HostListenerState::Removed | HostListenerState::TimedOut
        ) {
            return Err(SessionError::InvalidState {
                expected: "pending or active",
                actual: listener_state_for_host(listener.state),
            });
        }

        listener.last_seen = now;
        self.metrics.clock_sync_responses = self.metrics.clock_sync_responses.saturating_add(1);
        Ok(vec![SessionAction::Outbound {
            destination: source,
            message: ControlMessage::ClockSyncResponse {
                session_id: self.session_id,
                client_sent_micros,
                host_received_micros,
                host_sent_micros,
            },
        }])
    }

    /// Approves a pending listener. Repeating approval is safe and sends the
    /// approval again so a lost UDP response can be recovered.
    pub fn approve_listener(
        &mut self,
        listener_id: ListenerId,
    ) -> Result<Vec<SessionAction>, SessionError> {
        let endpoint = self
            .listener_endpoint(listener_id)
            .ok_or(SessionError::UnknownListener(listener_id))?;
        let was_pending = {
            let listener = self
                .listeners
                .get_mut(&endpoint)
                .expect("listener endpoint was found immediately before mutation");

            if listener.state == HostListenerState::Removed
                || listener.state == HostListenerState::TimedOut
            {
                return Err(SessionError::InvalidState {
                    expected: "pending or active",
                    actual: listener_state_for_host(listener.state),
                });
            }

            let was_pending = listener.state == HostListenerState::PendingApproval;
            listener.state = HostListenerState::Active;
            was_pending
        };
        if was_pending {
            self.metrics.approvals = self.metrics.approvals.saturating_add(1);
            self.update_active_listener_count();
        }
        Ok(vec![SessionAction::Outbound {
            destination: endpoint,
            message: ControlMessage::ListenerApproved {
                session_id: self.session_id,
                listener_id,
            },
        }])
    }

    /// Removes a listener. Repeating removal is safe and does not count as a
    /// second removal.
    pub fn remove_listener(
        &mut self,
        listener_id: ListenerId,
    ) -> Result<Vec<SessionAction>, SessionError> {
        let endpoint = self
            .listener_endpoint(listener_id)
            .ok_or(SessionError::UnknownListener(listener_id))?;
        let was_removed = {
            let listener = self
                .listeners
                .get_mut(&endpoint)
                .expect("listener endpoint was found immediately before mutation");
            let was_removed = listener.state == HostListenerState::Removed;
            listener.state = HostListenerState::Removed;
            was_removed
        };
        if !was_removed {
            self.metrics.removals = self.metrics.removals.saturating_add(1);
            self.update_active_listener_count();
        }
        Ok(if was_removed {
            Vec::new()
        } else {
            vec![
                SessionAction::Outbound {
                    destination: endpoint,
                    message: ControlMessage::ListenerRemoved {
                        session_id: self.session_id,
                        listener_id,
                    },
                },
                SessionAction::Event(SessionEvent::ListenerRemoved {
                    endpoint,
                    listener_id,
                }),
            ]
        })
    }

    /// Marks listeners that have not sent a heartbeat within `timeout` as
    /// timed out. A later valid join request reuses the same listener ID.
    pub fn expire_stale(&mut self, now: Instant, timeout: Duration) -> Vec<SessionAction> {
        let stale: Vec<(SocketAddr, ListenerId)> = self
            .listeners
            .values()
            .filter(|listener| {
                matches!(
                    listener.state,
                    HostListenerState::PendingApproval | HostListenerState::Active
                ) && now
                    .checked_duration_since(listener.last_seen)
                    .is_some_and(|elapsed| elapsed >= timeout)
            })
            .map(|listener| (listener.endpoint, listener.listener_id))
            .collect();

        let mut actions = Vec::new();
        for (endpoint, listener_id) in stale {
            let Some(listener) = self.listeners.get_mut(&endpoint) else {
                continue;
            };
            listener.state = HostListenerState::TimedOut;
            self.metrics.timeouts = self.metrics.timeouts.saturating_add(1);
            actions.push(SessionAction::Outbound {
                destination: endpoint,
                message: ControlMessage::ListenerRemoved {
                    session_id: self.session_id,
                    listener_id,
                },
            });
            actions.push(SessionAction::Event(SessionEvent::ListenerTimedOut {
                endpoint,
                listener_id,
            }));
        }
        self.update_active_listener_count();
        actions
    }

    pub fn end_session(&mut self, reason: u8) -> Vec<SessionAction> {
        if self.ended {
            return Vec::new();
        }
        self.ended = true;
        let listeners: Vec<SocketAddr> = self
            .listeners
            .values()
            .filter(|listener| {
                matches!(
                    listener.state,
                    HostListenerState::PendingApproval | HostListenerState::Active
                )
            })
            .map(|listener| listener.endpoint)
            .collect();
        for listener in self.listeners.values_mut() {
            listener.state = HostListenerState::Removed;
        }
        self.update_active_listener_count();
        listeners
            .into_iter()
            .map(|destination| SessionAction::Outbound {
                destination,
                message: ControlMessage::SessionEnded {
                    session_id: self.session_id,
                    reason,
                },
            })
            .collect()
    }

    fn active_listener_count(&self) -> usize {
        self.listeners
            .values()
            .filter(|listener| listener.state == HostListenerState::Active)
            .count()
    }

    fn connected_listener_count(&self) -> usize {
        self.listeners
            .values()
            .filter(|listener| {
                matches!(
                    listener.state,
                    HostListenerState::PendingApproval | HostListenerState::Active
                )
            })
            .count()
    }

    fn update_active_listener_count(&mut self) {
        self.metrics.active_listeners = self.active_listener_count();
    }

    fn allocate_listener_id(&mut self) -> ListenerId {
        let serial = self.next_listener_serial;
        self.next_listener_serial = self.next_listener_serial.saturating_add(1);
        ListenerId::new(serial.to_be_bytes())
    }
}

fn listener_state_for_host(state: HostListenerState) -> ListenerState {
    match state {
        HostListenerState::PendingApproval => ListenerState::PendingApproval,
        HostListenerState::Active => ListenerState::Active,
        HostListenerState::Removed => ListenerState::Removed,
        HostListenerState::TimedOut => ListenerState::TimedOut,
    }
}

/// Listener-side session state. It validates that all host messages arrive
/// from the configured host endpoint and owns the listener's clock estimator.
#[derive(Clone, Debug)]
pub struct ListenerSession {
    session_id: SessionId,
    token: SessionToken,
    host_endpoint: SocketAddr,
    listener_id: Option<ListenerId>,
    state: ListenerState,
    clock: ClockEstimator,
    metrics: SessionMetrics,
}

impl ListenerSession {
    pub fn new(session_id: SessionId, token: SessionToken, host_endpoint: SocketAddr) -> Self {
        Self {
            session_id,
            token,
            host_endpoint,
            listener_id: None,
            state: ListenerState::Disconnected,
            clock: ClockEstimator::default(),
            metrics: SessionMetrics::default(),
        }
    }

    pub const fn session_id(&self) -> SessionId {
        self.session_id
    }

    pub const fn token(&self) -> SessionToken {
        self.token
    }

    pub const fn host_endpoint(&self) -> SocketAddr {
        self.host_endpoint
    }

    pub const fn listener_id(&self) -> Option<ListenerId> {
        self.listener_id
    }

    pub const fn state(&self) -> ListenerState {
        self.state
    }

    /// Marks the listener as timed out after losing contact with the host.
    ///
    /// Terminal host decisions (`Removed`, `SessionEnded`) are preserved.
    /// Returns the previous state when a transition happened, so callers can
    /// surface a `StateChanged` event.
    pub fn mark_timed_out(&mut self) -> Option<ListenerState> {
        if matches!(
            self.state,
            ListenerState::Removed | ListenerState::SessionEnded | ListenerState::TimedOut
        ) {
            return None;
        }
        let previous = self.state;
        self.state = ListenerState::TimedOut;
        self.metrics.active_listeners = 0;
        Some(previous)
    }

    pub fn metrics(&self) -> &SessionMetrics {
        &self.metrics
    }

    pub fn clock_offset_micros(&self) -> Option<i64> {
        self.clock.offset_micros()
    }

    pub fn best_round_trip_micros(&self) -> Option<i64> {
        self.clock.best_round_trip_micros()
    }

    pub fn join_action(&self) -> SessionAction {
        SessionAction::Outbound {
            destination: self.host_endpoint,
            message: ControlMessage::JoinRequest {
                session_id: self.session_id,
                token: self.token,
            },
        }
    }

    pub fn heartbeat_action(&self) -> Option<SessionAction> {
        matches!(
            self.state,
            ListenerState::PendingApproval | ListenerState::Active
        )
        .then_some(SessionAction::Outbound {
            destination: self.host_endpoint,
            message: ControlMessage::Heartbeat {
                session_id: self.session_id,
            },
        })
    }

    pub fn clock_sync_action(&self, client_sent_micros: i64) -> Option<SessionAction> {
        matches!(
            self.state,
            ListenerState::PendingApproval | ListenerState::Active
        )
        .then_some(SessionAction::Outbound {
            destination: self.host_endpoint,
            message: ControlMessage::ClockSyncRequest {
                session_id: self.session_id,
                client_sent_micros,
            },
        })
    }

    /// Applies one decoded message from the configured host endpoint.
    pub fn handle_message(
        &mut self,
        source: SocketAddr,
        message: ControlMessage,
        client_received_micros: Option<i64>,
    ) -> Result<Vec<SessionAction>, SessionError> {
        if source != self.host_endpoint {
            self.metrics.invalid_messages = self.metrics.invalid_messages.saturating_add(1);
            return Err(SessionError::WrongPeer {
                expected: self.host_endpoint,
                actual: source,
            });
        }

        let message_session = message.session_id();
        if message_session != self.session_id {
            self.metrics.invalid_messages = self.metrics.invalid_messages.saturating_add(1);
            return Err(SessionError::InvalidSession {
                expected: self.session_id,
                actual: message_session,
            });
        }

        match message {
            ControlMessage::JoinPending { .. } => {
                self.state = ListenerState::PendingApproval;
                Ok(vec![SessionAction::Event(SessionEvent::StateChanged(
                    self.state,
                ))])
            }
            ControlMessage::JoinAccepted { listener_id, .. } => {
                self.listener_id = Some(listener_id);
                self.state = ListenerState::PendingApproval;
                self.metrics.join_accepted = self.metrics.join_accepted.saturating_add(1);
                Ok(vec![SessionAction::Event(SessionEvent::StateChanged(
                    self.state,
                ))])
            }
            ControlMessage::JoinRejected { reason, .. } => {
                self.state = ListenerState::Rejected;
                self.metrics.join_rejected = self.metrics.join_rejected.saturating_add(1);
                Ok(vec![SessionAction::Event(SessionEvent::JoinRejected {
                    endpoint: source,
                    reason,
                })])
            }
            ControlMessage::ListenerApproved { listener_id, .. } => {
                self.ensure_listener_id(listener_id)?;
                self.state = ListenerState::Active;
                self.metrics.approvals = self.metrics.approvals.saturating_add(1);
                self.metrics.active_listeners = 1;
                Ok(vec![SessionAction::Event(SessionEvent::StateChanged(
                    self.state,
                ))])
            }
            ControlMessage::ListenerRemoved { listener_id, .. } => {
                self.ensure_listener_id(listener_id)?;
                self.state = ListenerState::Removed;
                self.metrics.removals = self.metrics.removals.saturating_add(1);
                self.metrics.active_listeners = 0;
                Ok(vec![SessionAction::Event(SessionEvent::ListenerRemoved {
                    endpoint: source,
                    listener_id,
                })])
            }
            ControlMessage::Heartbeat { .. } => {
                self.metrics.heartbeats = self.metrics.heartbeats.saturating_add(1);
                Ok(vec![SessionAction::Event(
                    SessionEvent::HeartbeatReceived {
                        endpoint: source,
                        listener_id: self.listener_id.ok_or(SessionError::InvalidState {
                            expected: "joined",
                            actual: self.state,
                        })?,
                    },
                )])
            }
            ControlMessage::ClockSyncResponse {
                client_sent_micros,
                host_received_micros,
                host_sent_micros,
                ..
            } => {
                let client_received_micros =
                    client_received_micros.ok_or(SessionError::MissingClientReceiveTimestamp)?;
                let sample = ClockSample {
                    client_sent_micros,
                    host_received_micros,
                    host_sent_micros,
                    client_received_micros,
                };
                let round_trip_micros = sample
                    .round_trip_micros()
                    .ok_or(SessionError::InvalidClockSample)?;
                if round_trip_micros < 0 {
                    return Err(SessionError::InvalidClockSample);
                }
                let offset_micros = sample
                    .offset_micros()
                    .ok_or(SessionError::InvalidClockSample)?;
                let estimate_changed = self.clock.observe(sample);
                self.metrics.clock_sync_responses =
                    self.metrics.clock_sync_responses.saturating_add(1);
                Ok(vec![SessionAction::Event(SessionEvent::ClockSyncUpdated {
                    offset_micros,
                    round_trip_micros,
                    estimate_changed,
                })])
            }
            ControlMessage::SessionEnded { .. } => {
                self.state = ListenerState::SessionEnded;
                self.metrics.active_listeners = 0;
                Ok(vec![SessionAction::Event(SessionEvent::StateChanged(
                    self.state,
                ))])
            }
            ControlMessage::JoinRequest { .. } | ControlMessage::ClockSyncRequest { .. } => {
                self.metrics.invalid_messages = self.metrics.invalid_messages.saturating_add(1);
                Ok(vec![SessionAction::Event(SessionEvent::MessageIgnored {
                    endpoint: source,
                    reason: IgnoreReason::InvalidDirection,
                })])
            }
        }
    }

    fn ensure_listener_id(&self, actual: ListenerId) -> Result<(), SessionError> {
        match self.listener_id {
            Some(expected) if expected == actual => Ok(()),
            Some(expected) => Err(SessionError::ListenerIdMismatch { expected, actual }),
            None => Err(SessionError::UnknownListener(actual)),
        }
    }
}

/// Kept as a compile-time assertion that the session layer and transport agree
/// on a single control protocol version at this milestone.
const _: u8 = CONTROL_PROTOCOL_VERSION;

#[cfg(test)]
mod tests {
    use super::*;
    use audiosync_transport::ControlMessage;

    fn session_id() -> SessionId {
        [0x11; 16]
    }

    fn token() -> SessionToken {
        SessionToken::new([0x22; 16])
    }

    fn wrong_token() -> SessionToken {
        SessionToken::new([0x33; 16])
    }

    fn host_endpoint() -> SocketAddr {
        "127.0.0.1:40000".parse().expect("test address is valid")
    }

    fn listener_endpoint(port: u16) -> SocketAddr {
        SocketAddr::from(([127, 0, 0, 1], port))
    }

    fn outbound_message(actions: &[SessionAction]) -> ControlMessage {
        actions
            .iter()
            .find_map(|action| match action {
                SessionAction::Outbound { message, .. } => Some(message.clone()),
                SessionAction::Event(_) => None,
            })
            .expect("actions contain an outbound message")
    }

    #[test]
    fn valid_join_creates_pending_listener_and_approval_activates_it() {
        let now = Instant::now();
        let endpoint = listener_endpoint(40_001);
        let mut host = HostSession::new(session_id(), token(), 2);

        let join_actions = host.handle_message(
            endpoint,
            ControlMessage::JoinRequest {
                session_id: session_id(),
                token: token(),
            },
            now,
        );
        let listener_id = host
            .listener_id_for(endpoint)
            .expect("valid join creates a listener");

        assert!(matches!(
            outbound_message(&join_actions),
            ControlMessage::JoinAccepted { listener_id: id, .. } if id == listener_id
        ));
        assert!(join_actions.iter().any(|action| matches!(
            action,
            SessionAction::Event(SessionEvent::ListenerJoined {
                endpoint: event_endpoint,
                listener_id: event_id,
            }) if *event_endpoint == endpoint && *event_id == listener_id
        )));
        assert_eq!(
            host.listener_state(listener_id),
            Some(HostListenerState::PendingApproval)
        );

        let approval_actions = host
            .approve_listener(listener_id)
            .expect("pending listener can be approved");
        assert!(matches!(
            outbound_message(&approval_actions),
            ControlMessage::ListenerApproved { listener_id: id, .. } if id == listener_id
        ));
        assert_eq!(
            host.listener_state(listener_id),
            Some(HostListenerState::Active)
        );
        assert_eq!(host.metrics().approvals, 1);
        assert_eq!(host.metrics().active_listeners, 1);
    }

    #[test]
    fn invalid_session_token_and_capacity_are_rejected() {
        let now = Instant::now();
        let first = listener_endpoint(40_001);
        let second = listener_endpoint(40_002);
        let mut host = HostSession::new(session_id(), token(), 1);

        let invalid_token = host.handle_message(
            first,
            ControlMessage::JoinRequest {
                session_id: session_id(),
                token: wrong_token(),
            },
            now,
        );
        assert!(matches!(
            outbound_message(&invalid_token),
            ControlMessage::JoinRejected {
                reason: REJECT_INVALID_TOKEN,
                ..
            }
        ));

        let invalid_session = host.handle_message(
            first,
            ControlMessage::JoinRequest {
                session_id: [0x99; 16],
                token: token(),
            },
            now,
        );
        assert!(matches!(
            outbound_message(&invalid_session),
            ControlMessage::JoinRejected {
                reason: REJECT_UNKNOWN_SESSION,
                ..
            }
        ));

        host.handle_message(
            first,
            ControlMessage::JoinRequest {
                session_id: session_id(),
                token: token(),
            },
            now,
        );
        let full = host.handle_message(
            second,
            ControlMessage::JoinRequest {
                session_id: session_id(),
                token: token(),
            },
            now,
        );
        assert!(matches!(
            outbound_message(&full),
            ControlMessage::JoinRejected {
                reason: REJECT_MAX_LISTENERS,
                ..
            }
        ));
        assert_eq!(host.metrics().join_rejected, 3);
    }

    #[test]
    fn duplicate_join_is_idempotent_and_timeout_reconnect_reuses_id() {
        let started = Instant::now();
        let endpoint = listener_endpoint(40_001);
        let mut host = HostSession::new(session_id(), token(), 1);

        host.handle_message(
            endpoint,
            ControlMessage::JoinRequest {
                session_id: session_id(),
                token: token(),
            },
            started,
        );
        let listener_id = host.listener_id_for(endpoint).expect("listener exists");
        host.approve_listener(listener_id)
            .expect("listener can be approved");

        let duplicate = host.handle_message(
            endpoint,
            ControlMessage::JoinRequest {
                session_id: session_id(),
                token: token(),
            },
            started + Duration::from_millis(10),
        );
        assert_eq!(host.listener_id_for(endpoint), Some(listener_id));
        assert!(duplicate.iter().any(|action| matches!(
            action,
            SessionAction::Outbound {
                message: ControlMessage::ListenerApproved { listener_id: id, .. },
                ..
            } if *id == listener_id
        )));
        assert_eq!(host.metrics().duplicate_joins, 1);
        assert_eq!(host.metrics().reconnects, 0);

        let timeout_actions =
            host.expire_stale(started + Duration::from_secs(5), Duration::from_secs(1));
        assert!(timeout_actions.iter().any(|action| matches!(
            action,
            SessionAction::Event(SessionEvent::ListenerTimedOut {
                listener_id: id,
                ..
            }) if *id == listener_id
        )));
        assert_eq!(
            host.listener_state(listener_id),
            Some(HostListenerState::TimedOut)
        );

        host.handle_message(
            endpoint,
            ControlMessage::JoinRequest {
                session_id: session_id(),
                token: token(),
            },
            started + Duration::from_secs(6),
        );
        assert_eq!(host.listener_id_for(endpoint), Some(listener_id));
        assert_eq!(
            host.listener_state(listener_id),
            Some(HostListenerState::PendingApproval)
        );
        assert_eq!(host.metrics().reconnects, 1);
    }

    #[test]
    fn unknown_endpoint_is_not_allowed_to_heartbeat_or_sync() {
        let mut host = HostSession::new(session_id(), token(), 1);
        let unknown = listener_endpoint(40_009);
        let actions = host.handle_message(
            unknown,
            ControlMessage::Heartbeat {
                session_id: session_id(),
            },
            Instant::now(),
        );
        assert!(matches!(
            actions.as_slice(),
            [SessionAction::Event(SessionEvent::MessageIgnored {
                reason: IgnoreReason::UnknownEndpoint,
                ..
            })]
        ));
        assert_eq!(host.metrics().unknown_endpoints, 1);
        assert!(
            host.respond_to_clock_sync(unknown, 1, 2, 3, Instant::now())
                .is_err()
        );
    }

    #[test]
    fn listener_tracks_approval_removal_and_clock_offset() {
        let now = Instant::now();
        let host_endpoint = host_endpoint();
        let listener_endpoint = listener_endpoint(40_001);
        let mut host = HostSession::new(session_id(), token(), 1);
        let mut listener = ListenerSession::new(session_id(), token(), host_endpoint);

        let join = listener.join_action();
        let join_message = match join {
            SessionAction::Outbound { message, .. } => message,
            SessionAction::Event(_) => unreachable!("join action is outbound"),
        };
        let host_join_actions = host.handle_message(listener_endpoint, join_message, now);
        let listener_id = host
            .listener_id_for(listener_endpoint)
            .expect("host assigned listener ID");
        listener
            .handle_message(host_endpoint, outbound_message(&host_join_actions), None)
            .expect("listener accepts join");
        assert_eq!(listener.state(), ListenerState::PendingApproval);

        let approval = host
            .approve_listener(listener_id)
            .expect("host approves listener");
        listener
            .handle_message(host_endpoint, outbound_message(&approval), None)
            .expect("listener accepts approval");
        assert_eq!(listener.state(), ListenerState::Active);

        let sync_request = listener
            .clock_sync_action(0)
            .expect("active listener can request clock sync");
        let sync_message = match sync_request {
            SessionAction::Outbound { message, .. } => message,
            SessionAction::Event(_) => unreachable!("clock sync action is outbound"),
        };
        let client_sent = match sync_message {
            ControlMessage::ClockSyncRequest {
                client_sent_micros, ..
            } => client_sent_micros,
            _ => unreachable!("clock sync action has request message"),
        };
        host.handle_message(
            listener_endpoint,
            ControlMessage::ClockSyncRequest {
                session_id: session_id(),
                client_sent_micros: client_sent,
            },
            now,
        );
        let response = host
            .respond_to_clock_sync(listener_endpoint, client_sent, 25_005, 25_006, now)
            .expect("host can respond to active listener");
        let response_message = outbound_message(&response);
        listener
            .handle_message(host_endpoint, response_message, Some(11))
            .expect("listener accepts valid clock response");
        assert_eq!(listener.clock_offset_micros(), Some(25_000));
        assert_eq!(listener.best_round_trip_micros(), Some(10));

        let removal = host
            .remove_listener(listener_id)
            .expect("host can remove listener");
        listener
            .handle_message(host_endpoint, outbound_message(&removal), None)
            .expect("listener accepts removal");
        assert_eq!(listener.state(), ListenerState::Removed);
    }

    #[test]
    fn listener_watchdog_marks_timeout_and_preserves_terminal_states() {
        let mut listener = ListenerSession::new(session_id(), token(), host_endpoint());

        // Disconnected listener loses the host before joining completes.
        assert_eq!(listener.mark_timed_out(), Some(ListenerState::Disconnected));
        assert_eq!(listener.state(), ListenerState::TimedOut);
        // Repeated timeouts do not repeat the transition.
        assert_eq!(listener.mark_timed_out(), None);

        // A removed listener keeps the host's terminal decision.
        let mut removed = ListenerSession::new(session_id(), token(), host_endpoint());
        removed.listener_id = Some(ListenerId::new([9; 16]));
        removed.state = ListenerState::Removed;
        assert_eq!(removed.mark_timed_out(), None);
        assert_eq!(removed.state(), ListenerState::Removed);
    }

    #[test]
    fn session_end_is_idempotent_and_notifies_active_listeners() {
        let endpoint = listener_endpoint(40_001);
        let mut host = HostSession::new(session_id(), token(), 1);
        let now = Instant::now();
        host.handle_message(
            endpoint,
            ControlMessage::JoinRequest {
                session_id: session_id(),
                token: token(),
            },
            now,
        );
        let listener_id = host.listener_id_for(endpoint).expect("listener exists");
        host.approve_listener(listener_id)
            .expect("listener can be approved");

        let ended = host.end_session(REJECT_SESSION_ENDED);
        assert!(matches!(
            ended.as_slice(),
            [SessionAction::Outbound {
                message: ControlMessage::SessionEnded {
                    reason: REJECT_SESSION_ENDED,
                    ..
                },
                ..
            }]
        ));
        assert_eq!(host.metrics().active_listeners, 0);
        assert!(host.end_session(REJECT_SESSION_ENDED).is_empty());
    }
}
