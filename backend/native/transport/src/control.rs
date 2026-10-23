use audiosync_core::{PROTOCOL_VERSION, SessionId};
use std::fmt;

/// Four-byte marker at the beginning of every local control datagram.
pub const CONTROL_MAGIC: [u8; 4] = *b"ASCT";

/// Version carried by the control-message wire format.
pub const CONTROL_PROTOCOL_VERSION: u8 = PROTOCOL_VERSION;

/// Number of bytes in the fixed control-message header.
pub const CONTROL_HEADER_LEN: usize = 4 + 1 + 1 + 16 + 2;

/// Maximum number of bytes accepted for one control datagram.
pub const MAX_CONTROL_DATAGRAM_SIZE: usize = 1_024;

/// Maximum payload after the fixed control-message header.
pub const MAX_CONTROL_PAYLOAD_SIZE: usize = MAX_CONTROL_DATAGRAM_SIZE - CONTROL_HEADER_LEN;

/// Number of bytes in a session token.
pub const SESSION_TOKEN_LEN: usize = 16;

/// Number of bytes in a listener identifier.
pub const LISTENER_ID_LEN: usize = 16;

macro_rules! fixed_bytes_type {
    ($name:ident, $length:ident) => {
        #[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
        #[repr(transparent)]
        pub struct $name([u8; $length]);

        impl $name {
            pub const fn new(bytes: [u8; $length]) -> Self {
                Self(bytes)
            }

            pub const fn as_bytes(&self) -> &[u8; $length] {
                &self.0
            }

            pub const fn into_bytes(self) -> [u8; $length] {
                self.0
            }
        }

        impl AsRef<[u8]> for $name {
            fn as_ref(&self) -> &[u8] {
                &self.0
            }
        }

        impl From<[u8; $length]> for $name {
            fn from(bytes: [u8; $length]) -> Self {
                Self::new(bytes)
            }
        }

        impl From<$name> for [u8; $length] {
            fn from(value: $name) -> Self {
                value.into_bytes()
            }
        }
    };
}

fixed_bytes_type!(SessionToken, SESSION_TOKEN_LEN);
fixed_bytes_type!(ListenerId, LISTENER_ID_LEN);

const JOIN_REQUEST: u8 = 1;
const JOIN_PENDING: u8 = 2;
const JOIN_ACCEPTED: u8 = 3;
const JOIN_REJECTED: u8 = 4;
const LISTENER_APPROVED: u8 = 5;
const LISTENER_REMOVED: u8 = 6;
const HEARTBEAT: u8 = 7;
const CLOCK_SYNC_REQUEST: u8 = 8;
const CLOCK_SYNC_RESPONSE: u8 = 9;
const SESSION_ENDED: u8 = 10;

/// Local control messages exchanged while a session is being established and
/// maintained.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ControlMessage {
    JoinRequest {
        session_id: SessionId,
        token: SessionToken,
    },
    JoinPending {
        session_id: SessionId,
    },
    JoinAccepted {
        session_id: SessionId,
        listener_id: ListenerId,
    },
    JoinRejected {
        session_id: SessionId,
        reason: u8,
    },
    ListenerApproved {
        session_id: SessionId,
        listener_id: ListenerId,
    },
    ListenerRemoved {
        session_id: SessionId,
        listener_id: ListenerId,
    },
    Heartbeat {
        session_id: SessionId,
    },
    ClockSyncRequest {
        session_id: SessionId,
        client_sent_micros: i64,
    },
    ClockSyncResponse {
        session_id: SessionId,
        client_sent_micros: i64,
        host_received_micros: i64,
        host_sent_micros: i64,
    },
    SessionEnded {
        session_id: SessionId,
        reason: u8,
    },
}

impl ControlMessage {
    /// Returns the session to which this control message belongs.
    pub const fn session_id(&self) -> SessionId {
        match self {
            Self::JoinRequest { session_id, .. }
            | Self::JoinPending { session_id }
            | Self::JoinAccepted { session_id, .. }
            | Self::JoinRejected { session_id, .. }
            | Self::ListenerApproved { session_id, .. }
            | Self::ListenerRemoved { session_id, .. }
            | Self::Heartbeat { session_id }
            | Self::ClockSyncRequest { session_id, .. }
            | Self::ClockSyncResponse { session_id, .. }
            | Self::SessionEnded { session_id, .. } => *session_id,
        }
    }

    /// Encodes this message as one bounded, big-endian control datagram.
    pub fn encode(&self) -> Result<Vec<u8>, ControlMessageError> {
        let (message_type, payload_len) = self.message_type_and_payload_len();
        let datagram_len = CONTROL_HEADER_LEN + payload_len;
        if datagram_len > MAX_CONTROL_DATAGRAM_SIZE {
            return Err(ControlMessageError::DatagramTooLarge {
                max: MAX_CONTROL_DATAGRAM_SIZE,
                actual: datagram_len,
            });
        }

        let mut bytes = Vec::with_capacity(datagram_len);
        bytes.extend_from_slice(&CONTROL_MAGIC);
        bytes.push(CONTROL_PROTOCOL_VERSION);
        bytes.push(message_type);
        bytes.extend_from_slice(&self.session_id());
        bytes.extend_from_slice(&(payload_len as u16).to_be_bytes());

        match self {
            Self::JoinRequest { token, .. } => bytes.extend_from_slice(token.as_bytes()),
            Self::JoinPending { .. } | Self::Heartbeat { .. } => {}
            Self::JoinAccepted { listener_id, .. }
            | Self::ListenerApproved { listener_id, .. }
            | Self::ListenerRemoved { listener_id, .. } => {
                bytes.extend_from_slice(listener_id.as_bytes());
            }
            Self::JoinRejected { reason, .. } | Self::SessionEnded { reason, .. } => {
                bytes.push(*reason);
            }
            Self::ClockSyncRequest {
                client_sent_micros, ..
            } => bytes.extend_from_slice(&client_sent_micros.to_be_bytes()),
            Self::ClockSyncResponse {
                client_sent_micros,
                host_received_micros,
                host_sent_micros,
                ..
            } => {
                bytes.extend_from_slice(&client_sent_micros.to_be_bytes());
                bytes.extend_from_slice(&host_received_micros.to_be_bytes());
                bytes.extend_from_slice(&host_sent_micros.to_be_bytes());
            }
        }

        debug_assert_eq!(bytes.len(), datagram_len);
        Ok(bytes)
    }

    /// Decodes one complete control datagram.
    pub fn decode(bytes: &[u8]) -> Result<Self, ControlMessageError> {
        if bytes.len() > MAX_CONTROL_DATAGRAM_SIZE {
            return Err(ControlMessageError::DatagramTooLarge {
                max: MAX_CONTROL_DATAGRAM_SIZE,
                actual: bytes.len(),
            });
        }
        if bytes.len() < CONTROL_HEADER_LEN {
            return Err(ControlMessageError::Truncated {
                expected: CONTROL_HEADER_LEN,
                actual: bytes.len(),
            });
        }
        if bytes[..CONTROL_MAGIC.len()] != CONTROL_MAGIC {
            return Err(ControlMessageError::BadMagic);
        }

        let version = bytes[4];
        if version != CONTROL_PROTOCOL_VERSION {
            return Err(ControlMessageError::UnsupportedVersion(version));
        }

        let message_type = bytes[5];
        if !is_known_message_type(message_type) {
            return Err(ControlMessageError::UnknownMessageType(message_type));
        }

        let mut session_id = [0_u8; 16];
        session_id.copy_from_slice(&bytes[6..22]);

        let payload_len = u16::from_be_bytes([bytes[22], bytes[23]]) as usize;
        let expected_len = CONTROL_HEADER_LEN + payload_len;
        if expected_len > MAX_CONTROL_DATAGRAM_SIZE {
            return Err(ControlMessageError::DatagramTooLarge {
                max: MAX_CONTROL_DATAGRAM_SIZE,
                actual: expected_len,
            });
        }
        if bytes.len() < expected_len {
            return Err(ControlMessageError::Truncated {
                expected: expected_len,
                actual: bytes.len(),
            });
        }
        if bytes.len() > expected_len {
            return Err(ControlMessageError::InvalidLength {
                expected: expected_len,
                actual: bytes.len(),
            });
        }

        let payload = &bytes[CONTROL_HEADER_LEN..];
        decode_payload(message_type, session_id, payload)
    }

    fn message_type_and_payload_len(&self) -> (u8, usize) {
        match self {
            Self::JoinRequest { .. } => (JOIN_REQUEST, SESSION_TOKEN_LEN),
            Self::JoinPending { .. } => (JOIN_PENDING, 0),
            Self::JoinAccepted { .. } => (JOIN_ACCEPTED, LISTENER_ID_LEN),
            Self::JoinRejected { .. } => (JOIN_REJECTED, 1),
            Self::ListenerApproved { .. } => (LISTENER_APPROVED, LISTENER_ID_LEN),
            Self::ListenerRemoved { .. } => (LISTENER_REMOVED, LISTENER_ID_LEN),
            Self::Heartbeat { .. } => (HEARTBEAT, 0),
            Self::ClockSyncRequest { .. } => (CLOCK_SYNC_REQUEST, 8),
            Self::ClockSyncResponse { .. } => (CLOCK_SYNC_RESPONSE, 24),
            Self::SessionEnded { .. } => (SESSION_ENDED, 1),
        }
    }
}

fn is_known_message_type(message_type: u8) -> bool {
    matches!(
        message_type,
        JOIN_REQUEST
            | JOIN_PENDING
            | JOIN_ACCEPTED
            | JOIN_REJECTED
            | LISTENER_APPROVED
            | LISTENER_REMOVED
            | HEARTBEAT
            | CLOCK_SYNC_REQUEST
            | CLOCK_SYNC_RESPONSE
            | SESSION_ENDED
    )
}

fn decode_payload(
    message_type: u8,
    session_id: SessionId,
    payload: &[u8],
) -> Result<ControlMessage, ControlMessageError> {
    match message_type {
        JOIN_REQUEST => {
            expect_payload_len(message_type, SESSION_TOKEN_LEN, payload.len())?;
            let mut token = [0_u8; SESSION_TOKEN_LEN];
            token.copy_from_slice(payload);
            Ok(ControlMessage::JoinRequest {
                session_id,
                token: SessionToken::new(token),
            })
        }
        JOIN_PENDING => {
            expect_payload_len(message_type, 0, payload.len())?;
            Ok(ControlMessage::JoinPending { session_id })
        }
        JOIN_ACCEPTED => {
            expect_payload_len(message_type, LISTENER_ID_LEN, payload.len())?;
            let mut listener_id = [0_u8; LISTENER_ID_LEN];
            listener_id.copy_from_slice(payload);
            Ok(ControlMessage::JoinAccepted {
                session_id,
                listener_id: ListenerId::new(listener_id),
            })
        }
        JOIN_REJECTED => {
            expect_payload_len(message_type, 1, payload.len())?;
            Ok(ControlMessage::JoinRejected {
                session_id,
                reason: payload[0],
            })
        }
        LISTENER_APPROVED => {
            expect_payload_len(message_type, LISTENER_ID_LEN, payload.len())?;
            let mut listener_id = [0_u8; LISTENER_ID_LEN];
            listener_id.copy_from_slice(payload);
            Ok(ControlMessage::ListenerApproved {
                session_id,
                listener_id: ListenerId::new(listener_id),
            })
        }
        LISTENER_REMOVED => {
            expect_payload_len(message_type, LISTENER_ID_LEN, payload.len())?;
            let mut listener_id = [0_u8; LISTENER_ID_LEN];
            listener_id.copy_from_slice(payload);
            Ok(ControlMessage::ListenerRemoved {
                session_id,
                listener_id: ListenerId::new(listener_id),
            })
        }
        HEARTBEAT => {
            expect_payload_len(message_type, 0, payload.len())?;
            Ok(ControlMessage::Heartbeat { session_id })
        }
        CLOCK_SYNC_REQUEST => {
            expect_payload_len(message_type, 8, payload.len())?;
            let client_sent_micros = i64::from_be_bytes(
                payload
                    .try_into()
                    .expect("clock-sync request length was validated"),
            );
            Ok(ControlMessage::ClockSyncRequest {
                session_id,
                client_sent_micros,
            })
        }
        CLOCK_SYNC_RESPONSE => {
            expect_payload_len(message_type, 24, payload.len())?;
            let client_sent_micros = i64::from_be_bytes(
                payload[0..8]
                    .try_into()
                    .expect("client timestamp length was validated"),
            );
            let host_received_micros = i64::from_be_bytes(
                payload[8..16]
                    .try_into()
                    .expect("host receive timestamp length was validated"),
            );
            let host_sent_micros = i64::from_be_bytes(
                payload[16..24]
                    .try_into()
                    .expect("host send timestamp length was validated"),
            );
            Ok(ControlMessage::ClockSyncResponse {
                session_id,
                client_sent_micros,
                host_received_micros,
                host_sent_micros,
            })
        }
        SESSION_ENDED => {
            expect_payload_len(message_type, 1, payload.len())?;
            Ok(ControlMessage::SessionEnded {
                session_id,
                reason: payload[0],
            })
        }
        _ => Err(ControlMessageError::UnknownMessageType(message_type)),
    }
}

fn expect_payload_len(
    message_type: u8,
    expected: usize,
    actual: usize,
) -> Result<(), ControlMessageError> {
    if expected == actual {
        Ok(())
    } else {
        Err(ControlMessageError::InvalidPayloadLength {
            message_type,
            expected,
            actual,
        })
    }
}

/// Errors returned when a control datagram cannot be encoded or decoded.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ControlMessageError {
    DatagramTooLarge {
        max: usize,
        actual: usize,
    },
    Truncated {
        expected: usize,
        actual: usize,
    },
    BadMagic,
    UnsupportedVersion(u8),
    UnknownMessageType(u8),
    InvalidLength {
        expected: usize,
        actual: usize,
    },
    InvalidPayloadLength {
        message_type: u8,
        expected: usize,
        actual: usize,
    },
}

/// Short alias for callers that prefer a compact error name.
pub type ControlError = ControlMessageError;

impl fmt::Display for ControlMessageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DatagramTooLarge { max, actual } => {
                write!(
                    formatter,
                    "control datagram is too large: maximum {max} bytes, got {actual}"
                )
            }
            Self::Truncated { expected, actual } => {
                write!(
                    formatter,
                    "control datagram is truncated: expected {expected} bytes, got {actual}"
                )
            }
            Self::BadMagic => write!(formatter, "control datagram magic does not match AudioSync"),
            Self::UnsupportedVersion(version) => {
                write!(formatter, "unsupported control protocol version {version}")
            }
            Self::UnknownMessageType(message_type) => {
                write!(formatter, "unknown control message type {message_type}")
            }
            Self::InvalidLength { expected, actual } => {
                write!(
                    formatter,
                    "control datagram length mismatch: expected {expected} bytes, got {actual}"
                )
            }
            Self::InvalidPayloadLength {
                message_type,
                expected,
                actual,
            } => write!(
                formatter,
                "invalid payload length for control message type {message_type}: expected {expected} bytes, got {actual}"
            ),
        }
    }
}

impl std::error::Error for ControlMessageError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn session_id() -> SessionId {
        [0x11; 16]
    }

    fn token() -> SessionToken {
        SessionToken::new([0x22; SESSION_TOKEN_LEN])
    }

    fn listener_id() -> ListenerId {
        ListenerId::new([0x33; LISTENER_ID_LEN])
    }

    fn messages() -> Vec<ControlMessage> {
        let session_id = session_id();
        let listener_id = listener_id();
        vec![
            ControlMessage::JoinRequest {
                session_id,
                token: token(),
            },
            ControlMessage::JoinPending { session_id },
            ControlMessage::JoinAccepted {
                session_id,
                listener_id,
            },
            ControlMessage::JoinRejected {
                session_id,
                reason: 7,
            },
            ControlMessage::ListenerApproved {
                session_id,
                listener_id,
            },
            ControlMessage::ListenerRemoved {
                session_id,
                listener_id,
            },
            ControlMessage::Heartbeat { session_id },
            ControlMessage::ClockSyncRequest {
                session_id,
                client_sent_micros: -42,
            },
            ControlMessage::ClockSyncResponse {
                session_id,
                client_sent_micros: -42,
                host_received_micros: 1_000,
                host_sent_micros: 1_005,
            },
            ControlMessage::SessionEnded {
                session_id,
                reason: 9,
            },
        ]
    }

    #[test]
    fn every_control_message_round_trips() {
        for message in messages() {
            let session_id = message.session_id();
            let encoded = message.encode().expect("message should encode");
            assert!(encoded.len() <= MAX_CONTROL_DATAGRAM_SIZE);
            assert_eq!(ControlMessage::decode(&encoded), Ok(message));
            assert_eq!(
                ControlMessage::decode(&encoded).unwrap().session_id(),
                session_id
            );
        }
    }

    #[test]
    fn fixed_size_identifiers_preserve_their_bytes() {
        assert_eq!(token().as_bytes(), &[0x22; SESSION_TOKEN_LEN]);
        assert_eq!(listener_id().into_bytes(), [0x33; LISTENER_ID_LEN]);
    }

    #[test]
    fn decoder_rejects_bad_magic_version_and_unknown_type() {
        let message = ControlMessage::Heartbeat {
            session_id: session_id(),
        };
        let encoded = message.encode().expect("message should encode");

        let mut bad_magic = encoded.clone();
        bad_magic[0] = b'X';
        assert_eq!(
            ControlMessage::decode(&bad_magic),
            Err(ControlMessageError::BadMagic)
        );

        let mut bad_version = encoded.clone();
        bad_version[4] = CONTROL_PROTOCOL_VERSION.saturating_add(1);
        assert_eq!(
            ControlMessage::decode(&bad_version),
            Err(ControlMessageError::UnsupportedVersion(
                CONTROL_PROTOCOL_VERSION.saturating_add(1)
            ))
        );

        let mut unknown_type = encoded;
        unknown_type[5] = 0xFF;
        assert_eq!(
            ControlMessage::decode(&unknown_type),
            Err(ControlMessageError::UnknownMessageType(0xFF))
        );
    }

    #[test]
    fn decoder_rejects_truncated_and_invalid_lengths() {
        let message = ControlMessage::JoinRequest {
            session_id: session_id(),
            token: token(),
        };
        let encoded = message.encode().expect("message should encode");

        assert!(matches!(
            ControlMessage::decode(&encoded[..CONTROL_HEADER_LEN - 1]),
            Err(ControlMessageError::Truncated { .. })
        ));

        let mut truncated_payload = encoded.clone();
        truncated_payload.pop();
        assert!(matches!(
            ControlMessage::decode(&truncated_payload),
            Err(ControlMessageError::Truncated { .. })
        ));

        let mut extra_bytes = encoded.clone();
        extra_bytes.push(0);
        assert!(matches!(
            ControlMessage::decode(&extra_bytes),
            Err(ControlMessageError::InvalidLength { .. })
        ));

        let mut invalid_payload_length = encoded;
        invalid_payload_length[22..24].copy_from_slice(&1_u16.to_be_bytes());
        assert!(matches!(
            ControlMessage::decode(&invalid_payload_length),
            Err(ControlMessageError::InvalidLength { .. })
        ));

        let oversized = vec![0_u8; MAX_CONTROL_DATAGRAM_SIZE + 1];
        assert!(matches!(
            ControlMessage::decode(&oversized),
            Err(ControlMessageError::DatagramTooLarge { .. })
        ));
    }

    #[test]
    fn decoder_rejects_known_messages_with_wrong_payload_lengths() {
        let mut encoded = ControlMessage::Heartbeat {
            session_id: session_id(),
        }
        .encode()
        .expect("message should encode");
        encoded[22..24].copy_from_slice(&1_u16.to_be_bytes());
        encoded.push(0xAA);

        assert!(matches!(
            ControlMessage::decode(&encoded),
            Err(ControlMessageError::InvalidPayloadLength {
                message_type: HEARTBEAT,
                expected: 0,
                actual: 1,
            })
        ));
    }
}
