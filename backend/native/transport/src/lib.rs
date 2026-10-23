//! UDP transport and versioned local session-control messages for AudioSync.

mod control;
mod udp;

pub use audiosync_core::SessionId;
pub use control::{
    CONTROL_HEADER_LEN, CONTROL_MAGIC, CONTROL_PROTOCOL_VERSION, ControlError, ControlMessage,
    ControlMessageError, LISTENER_ID_LEN, ListenerId, MAX_CONTROL_DATAGRAM_SIZE,
    MAX_CONTROL_PAYLOAD_SIZE, SESSION_TOKEN_LEN, SessionToken,
};
pub use udp::UdpTransport;
