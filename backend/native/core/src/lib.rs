//! Platform-neutral protocol and timing primitives for AudioSync.
//!
//! The first milestone deliberately contains no operating-system audio code.
//! This keeps the packet format, clock model and jitter-buffer behavior
//! testable on every development machine before Windows and Android adapters
//! are introduced.

use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::collections::{BTreeMap, VecDeque};
use std::fmt;

pub const PROTOCOL_VERSION: u8 = 2;
pub const AUDIO_PACKET_MAGIC: [u8; 4] = *b"ASAU";
pub const MAX_AUDIO_PAYLOAD_BYTES: usize = 65_535;
const AUDIO_PACKET_HEADER_LEN: usize = 4 + 1 + 1 + 16 + 4 + 8 + 4;
/// Bytes of truncated HMAC-SHA256 carried at the end of every audio packet.
pub const AUDIO_PACKET_TAG_BYTES: usize = 16;
/// Key length for the per-session audio authentication token.
pub const AUDIO_PACKET_TOKEN_BYTES: usize = 16;
const CLOCK_SAMPLE_WINDOW: usize = 8;

pub type SessionId = [u8; 16];

type PacketMac = Hmac<Sha256>;

/// An Opus audio frame together with the information needed to schedule it.
///
/// Version 2 appends a 16-byte truncated HMAC-SHA256 tag, keyed by the
/// session token, over the whole datagram. Hosts sign every packet before
/// sending; listeners drop datagrams whose tag does not verify.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioPacket {
    pub version: u8,
    pub flags: u8,
    pub session_id: SessionId,
    pub sequence: u32,
    pub timestamp_micros: u64,
    pub payload: Vec<u8>,
    auth_tag: [u8; AUDIO_PACKET_TAG_BYTES],
}

impl AudioPacket {
    pub fn new(
        session_id: SessionId,
        sequence: u32,
        timestamp_micros: u64,
        payload: Vec<u8>,
    ) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            flags: 0,
            session_id,
            sequence,
            timestamp_micros,
            payload,
            auth_tag: [0; AUDIO_PACKET_TAG_BYTES],
        }
    }

    /// Signs this packet in place with the per-session token.
    ///
    /// The tag covers the complete version-2 wire format (header and payload)
    /// so sequence numbers, timestamps, and the session identity cannot be
    /// tampered with independently.
    pub fn sign(&mut self, token: &[u8; AUDIO_PACKET_TOKEN_BYTES]) {
        self.auth_tag = [0; AUDIO_PACKET_TAG_BYTES];
        let wire = self.encode().expect("a packet within limits can encode");
        let mut mac = PacketMac::new_from_slice(token).expect("HMAC accepts a 16-byte key");
        mac.update(&wire);
        let digest = mac.finalize().into_bytes();
        self.auth_tag
            .copy_from_slice(&digest[..AUDIO_PACKET_TAG_BYTES]);
    }

    /// Whether this packet carries a non-zero authentication tag.
    pub fn is_signed(&self) -> bool {
        self.auth_tag != [0; AUDIO_PACKET_TAG_BYTES]
    }

    /// Verifies the authentication tag in constant time.
    pub fn verify(&self, token: &[u8; AUDIO_PACKET_TOKEN_BYTES]) -> bool {
        let mut unsigned = self.clone();
        unsigned.auth_tag = [0; AUDIO_PACKET_TAG_BYTES];
        let Ok(wire) = unsigned.encode() else {
            return false;
        };
        let mut mac = PacketMac::new_from_slice(token).expect("HMAC accepts a 16-byte key");
        mac.update(&wire);
        let digest = mac.finalize().into_bytes();
        let expected = &digest[..AUDIO_PACKET_TAG_BYTES];
        use subtle::ConstantTimeEq;
        expected.ct_eq(&self.auth_tag[..]).into()
    }

    /// Encode a packet using a small, versioned big-endian wire format with
    /// the authentication tag appended.
    pub fn encode(&self) -> Result<Vec<u8>, PacketError> {
        if self.payload.len() > MAX_AUDIO_PAYLOAD_BYTES {
            return Err(PacketError::PayloadTooLarge {
                max: MAX_AUDIO_PAYLOAD_BYTES,
                actual: self.payload.len(),
            });
        }

        let mut bytes = Vec::with_capacity(
            AUDIO_PACKET_HEADER_LEN + self.payload.len() + AUDIO_PACKET_TAG_BYTES,
        );
        bytes.extend_from_slice(&AUDIO_PACKET_MAGIC);
        bytes.push(self.version);
        bytes.push(self.flags);
        bytes.extend_from_slice(&self.session_id);
        bytes.extend_from_slice(&self.sequence.to_be_bytes());
        bytes.extend_from_slice(&self.timestamp_micros.to_be_bytes());
        bytes.extend_from_slice(&(self.payload.len() as u32).to_be_bytes());
        bytes.extend_from_slice(&self.payload);
        bytes.extend_from_slice(&self.auth_tag);
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, PacketError> {
        let minimum = AUDIO_PACKET_HEADER_LEN + AUDIO_PACKET_TAG_BYTES;
        if bytes.len() < minimum {
            return Err(PacketError::TooShort {
                expected: minimum,
                actual: bytes.len(),
            });
        }

        if bytes[0..4] != AUDIO_PACKET_MAGIC {
            return Err(PacketError::BadMagic);
        }

        let version = bytes[4];
        if version != PROTOCOL_VERSION {
            return Err(PacketError::UnsupportedVersion(version));
        }

        let flags = bytes[5];
        let mut session_id = [0_u8; 16];
        session_id.copy_from_slice(&bytes[6..22]);

        let sequence = u32::from_be_bytes(bytes[22..26].try_into().expect("fixed packet field"));
        let timestamp_micros =
            u64::from_be_bytes(bytes[26..34].try_into().expect("fixed packet field"));
        let payload_len =
            u32::from_be_bytes(bytes[34..38].try_into().expect("fixed packet field")) as usize;
        if payload_len > MAX_AUDIO_PAYLOAD_BYTES {
            return Err(PacketError::PayloadTooLarge {
                max: MAX_AUDIO_PAYLOAD_BYTES,
                actual: payload_len,
            });
        }
        let expected_len = AUDIO_PACKET_HEADER_LEN
            .checked_add(payload_len)
            .and_then(|length| length.checked_add(AUDIO_PACKET_TAG_BYTES))
            .ok_or(PacketError::LengthOverflow)?;

        if bytes.len() != expected_len {
            return Err(PacketError::LengthMismatch {
                expected: expected_len,
                actual: bytes.len(),
            });
        }

        let mut auth_tag = [0_u8; AUDIO_PACKET_TAG_BYTES];
        auth_tag.copy_from_slice(&bytes[expected_len - AUDIO_PACKET_TAG_BYTES..expected_len]);

        Ok(Self {
            version,
            flags,
            session_id,
            sequence,
            timestamp_micros,
            payload: bytes[AUDIO_PACKET_HEADER_LEN..expected_len - AUDIO_PACKET_TAG_BYTES].to_vec(),
            auth_tag,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PacketError {
    TooShort { expected: usize, actual: usize },
    BadMagic,
    UnsupportedVersion(u8),
    PayloadTooLarge { max: usize, actual: usize },
    LengthOverflow,
    LengthMismatch { expected: usize, actual: usize },
}

impl fmt::Display for PacketError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort { expected, actual } => {
                write!(
                    formatter,
                    "packet is too short: expected {expected} bytes, got {actual}"
                )
            }
            Self::BadMagic => write!(formatter, "packet magic does not match AudioSync"),
            Self::UnsupportedVersion(version) => {
                write!(formatter, "unsupported protocol version {version}")
            }
            Self::PayloadTooLarge { max, actual } => {
                write!(
                    formatter,
                    "audio payload is too large: maximum {max} bytes, got {actual}"
                )
            }
            Self::LengthOverflow => write!(formatter, "packet length overflows the address space"),
            Self::LengthMismatch { expected, actual } => {
                write!(
                    formatter,
                    "packet length mismatch: expected {expected} bytes, got {actual}"
                )
            }
        }
    }
}

impl std::error::Error for PacketError {}

/// One four-timestamp exchange used to estimate the host/client clock offset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClockSample {
    pub client_sent_micros: i64,
    pub host_received_micros: i64,
    pub host_sent_micros: i64,
    pub client_received_micros: i64,
}

impl ClockSample {
    fn validate(self) -> Option<()> {
        (self.client_received_micros >= self.client_sent_micros
            && self.host_sent_micros >= self.host_received_micros)
            .then_some(())
    }

    /// The estimated host-minus-client offset for this exchange.
    pub fn offset_micros(self) -> Option<i64> {
        self.validate()?;
        let outbound = self
            .host_received_micros
            .checked_sub(self.client_sent_micros)?;
        let inbound = self
            .host_sent_micros
            .checked_sub(self.client_received_micros)?;
        outbound.checked_add(inbound).map(|sum| sum / 2)
    }

    /// Round-trip time after subtracting the time spent processing the request.
    pub fn round_trip_micros(self) -> Option<i64> {
        self.validate()?;
        let elapsed = self
            .client_received_micros
            .checked_sub(self.client_sent_micros)?;
        let processing = self
            .host_sent_micros
            .checked_sub(self.host_received_micros)?;
        elapsed.checked_sub(processing)
    }
}

/// Keeps the lowest-RTT sample in a rolling window, which is normally the least
/// affected by queueing on a local network while still allowing the estimate to
/// follow clock drift after old samples expire.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ClockEstimator {
    best_round_trip_micros: Option<i64>,
    offset_micros: Option<i64>,
    sample_count: u32,
    samples: VecDeque<(i64, i64)>,
}

impl ClockEstimator {
    pub fn observe(&mut self, sample: ClockSample) -> bool {
        let Some(round_trip) = sample.round_trip_micros() else {
            return false;
        };
        if round_trip < 0 {
            return false;
        }
        let Some(offset) = sample.offset_micros() else {
            return false;
        };

        self.sample_count = self.sample_count.saturating_add(1);
        self.samples.push_back((round_trip, offset));
        if self.samples.len() > CLOCK_SAMPLE_WINDOW {
            self.samples.pop_front();
        }

        let (best_round_trip, best_offset) = self
            .samples
            .iter()
            .copied()
            .min_by_key(|(round_trip, _)| *round_trip)
            .expect("a valid sample was just inserted");
        let changed = self.best_round_trip_micros != Some(best_round_trip)
            || self.offset_micros != Some(best_offset);
        self.best_round_trip_micros = Some(best_round_trip);
        self.offset_micros = Some(best_offset);

        changed
    }

    pub fn offset_micros(&self) -> Option<i64> {
        self.offset_micros
    }

    pub fn best_round_trip_micros(&self) -> Option<i64> {
        self.best_round_trip_micros
    }

    pub fn sample_count(&self) -> u32 {
        self.sample_count
    }
}

/// A bounded packet queue that releases frames in sequence order.
///
/// A missing packet is intentionally not skipped automatically. The playback
/// scheduler decides when a missing frame has exceeded the jitter deadline and
/// calls [`JitterBuffer::skip_missing`]. This keeps transport policy separate
/// from the queue itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JitterBuffer<T> {
    capacity: usize,
    next_sequence: Option<u32>,
    packets: BTreeMap<u32, T>,
    dropped_packets: u64,
    skipped_packets: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PushResult {
    Inserted,
    Duplicate,
    TooLate,
    CapacityDrop,
}

impl<T> JitterBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "a jitter buffer must have capacity");
        Self {
            capacity,
            next_sequence: None,
            packets: BTreeMap::new(),
            dropped_packets: 0,
            skipped_packets: 0,
        }
    }

    pub fn start_at(&mut self, sequence: u32) {
        self.next_sequence = Some(sequence);
        self.packets
            .retain(|candidate, _| !sequence_is_before(*candidate, sequence));
    }

    pub fn push(&mut self, sequence: u32, packet: T) -> PushResult {
        if self
            .next_sequence
            .is_some_and(|next| sequence.wrapping_sub(next) > u32::MAX / 2)
        {
            self.dropped_packets = self.dropped_packets.saturating_add(1);
            return PushResult::TooLate;
        }

        if self.packets.contains_key(&sequence) {
            self.dropped_packets = self.dropped_packets.saturating_add(1);
            return PushResult::Duplicate;
        }

        if self.packets.len() >= self.capacity {
            let base = self
                .next_sequence
                .or_else(|| self.packets.keys().next().copied())
                .expect("a non-empty buffer is required when at capacity");
            let largest = self
                .packets
                .keys()
                .copied()
                .max_by_key(|candidate| sequence_distance(base, *candidate))
                .expect("a non-empty buffer is required when at capacity");

            if sequence_distance(base, sequence) >= sequence_distance(base, largest) {
                self.dropped_packets = self.dropped_packets.saturating_add(1);
                return PushResult::CapacityDrop;
            }
            self.packets.remove(&largest);
            self.dropped_packets = self.dropped_packets.saturating_add(1);
        }

        self.packets.insert(sequence, packet);
        PushResult::Inserted
    }

    pub fn pop_next(&mut self) -> Option<T> {
        let next = self
            .next_sequence
            .or_else(|| self.packets.keys().next().copied())?;
        self.next_sequence = Some(next);
        let packet = self.packets.remove(&next)?;
        self.next_sequence = Some(next.wrapping_add(1));
        Some(packet)
    }

    /// Borrows the next packet without advancing the playback cursor.
    pub fn peek_next(&self) -> Option<&T> {
        let next = self
            .next_sequence
            .or_else(|| self.packets.keys().next().copied())?;
        self.packets.get(&next)
    }

    pub fn skip_missing(&mut self) -> bool {
        let Some(next) = self.next_sequence else {
            return false;
        };
        self.next_sequence = Some(next.wrapping_add(1));
        self.skipped_packets = self.skipped_packets.saturating_add(1);
        true
    }

    pub fn len(&self) -> usize {
        self.packets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.packets.is_empty()
    }

    pub fn next_sequence(&self) -> Option<u32> {
        self.next_sequence
    }

    pub fn dropped_packets(&self) -> u64 {
        self.dropped_packets
    }

    pub fn skipped_packets(&self) -> u64 {
        self.skipped_packets
    }
}

fn sequence_is_before(candidate: u32, reference: u32) -> bool {
    candidate != reference && candidate.wrapping_sub(reference) > u32::MAX / 2
}

fn sequence_distance(base: u32, candidate: u32) -> u32 {
    candidate.wrapping_sub(base)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session_id() -> SessionId {
        [7; 16]
    }

    #[test]
    fn audio_packet_round_trips() {
        let packet = AudioPacket::new(session_id(), 42, 123_456, vec![1, 2, 3, 4]);
        let encoded = packet.encode().expect("small payload should encode");
        assert_eq!(AudioPacket::decode(&encoded), Ok(packet));
    }

    #[test]
    fn audio_packet_signs_and_verifies_with_the_session_token() {
        let token = [0x42_u8; AUDIO_PACKET_TOKEN_BYTES];
        let wrong_token = [0x00_u8; AUDIO_PACKET_TOKEN_BYTES];
        let mut packet = AudioPacket::new(session_id(), 7, 1_000, vec![1, 2, 3]);
        assert!(!packet.is_signed());

        packet.sign(&token);
        assert!(packet.is_signed());

        let encoded = packet.encode().expect("signed packet encodes");
        let decoded = AudioPacket::decode(&encoded).expect("signed packet decodes");
        assert!(decoded.verify(&token));
        assert!(!decoded.verify(&wrong_token));

        // Flipping the last tag byte keeps the packet parseable but breaks
        // verification.
        let mut tampered = encoded.clone();
        let last = tampered.len() - 1;
        tampered[last] ^= 0xFF;
        let tampered = AudioPacket::decode(&tampered).expect("tampered packet still parses");
        assert!(!tampered.verify(&token));

        // Removing tag bytes is a length mismatch, not a verification event.
        let mut truncated = encoded;
        truncated.pop();
        assert!(matches!(
            AudioPacket::decode(&truncated),
            Err(PacketError::LengthMismatch { .. })
        ));
    }

    #[test]
    fn audio_packet_rejects_bad_magic_and_length() {
        let packet = AudioPacket::new(session_id(), 1, 0, vec![9, 8]);
        let mut encoded = packet.encode().expect("small payload should encode");
        encoded[0] = b'X';
        assert_eq!(AudioPacket::decode(&encoded), Err(PacketError::BadMagic));

        let mut encoded = packet.encode().expect("small payload should encode");
        encoded.pop();
        assert!(matches!(
            AudioPacket::decode(&encoded),
            Err(PacketError::LengthMismatch { .. })
        ));
    }

    #[test]
    fn clock_estimator_prefers_the_lowest_round_trip_sample() {
        let mut estimator = ClockEstimator::default();

        assert!(estimator.observe(ClockSample {
            client_sent_micros: 0,
            host_received_micros: 25_005,
            host_sent_micros: 25_006,
            client_received_micros: 11,
        }));
        assert_eq!(estimator.offset_micros(), Some(25_000));
        assert_eq!(estimator.best_round_trip_micros(), Some(10));

        assert!(!estimator.observe(ClockSample {
            client_sent_micros: 100,
            host_received_micros: 25_130,
            host_sent_micros: 25_160,
            client_received_micros: 180,
        }));
        assert_eq!(estimator.offset_micros(), Some(25_000));
        assert_eq!(estimator.sample_count(), 2);
    }

    #[test]
    fn clock_estimator_rejects_impossible_samples() {
        let mut estimator = ClockEstimator::default();
        assert!(!estimator.observe(ClockSample {
            client_sent_micros: 10,
            host_received_micros: 0,
            host_sent_micros: 1,
            client_received_micros: 2,
        }));
        assert_eq!(estimator.sample_count(), 0);

        assert!(!estimator.observe(ClockSample {
            client_sent_micros: 10,
            host_received_micros: 20,
            host_sent_micros: 19,
            client_received_micros: 30,
        }));
        assert_eq!(estimator.sample_count(), 0);
    }

    #[test]
    fn clock_estimator_expires_old_samples_so_it_can_follow_drift() {
        let mut estimator = ClockEstimator::default();
        assert!(estimator.observe(ClockSample {
            client_sent_micros: 0,
            host_received_micros: 1_005,
            host_sent_micros: 1_006,
            client_received_micros: 11,
        }));

        for index in 0..CLOCK_SAMPLE_WINDOW {
            let client_sent = (index as i64 + 1) * 100;
            let _ = estimator.observe(ClockSample {
                client_sent_micros: client_sent,
                host_received_micros: client_sent + 2_005,
                host_sent_micros: client_sent + 2_006,
                client_received_micros: client_sent + 11,
            });
        }

        assert_eq!(estimator.offset_micros(), Some(2_000));
    }

    #[test]
    fn jitter_buffer_reorders_packets() {
        let mut buffer = JitterBuffer::new(4);
        buffer.start_at(10);
        assert_eq!(buffer.push(12, "twelve"), PushResult::Inserted);
        assert_eq!(buffer.push(10, "ten"), PushResult::Inserted);
        assert_eq!(buffer.push(11, "eleven"), PushResult::Inserted);

        assert_eq!(buffer.pop_next(), Some("ten"));
        assert_eq!(buffer.pop_next(), Some("eleven"));
        assert_eq!(buffer.pop_next(), Some("twelve"));
        assert_eq!(buffer.pop_next(), None);
    }

    #[test]
    fn jitter_buffer_can_skip_a_frame_after_a_deadline() {
        let mut buffer = JitterBuffer::new(4);
        buffer.start_at(0);
        buffer.push(0, "zero");
        buffer.push(2, "two");

        assert_eq!(buffer.pop_next(), Some("zero"));
        assert_eq!(buffer.pop_next(), None);
        assert!(buffer.skip_missing());
        assert_eq!(buffer.pop_next(), Some("two"));
        assert_eq!(buffer.skipped_packets(), 1);
    }

    #[test]
    fn jitter_buffer_can_skip_when_no_later_packet_has_arrived() {
        let mut buffer = JitterBuffer::<()>::new(4);
        buffer.start_at(9);

        assert!(buffer.skip_missing());
        assert_eq!(buffer.next_sequence(), Some(10));
        assert_eq!(buffer.skipped_packets(), 1);
    }

    #[test]
    fn jitter_buffer_rejects_duplicates_and_late_packets() {
        let mut buffer = JitterBuffer::new(2);
        buffer.start_at(5);
        assert_eq!(buffer.push(5, "first"), PushResult::Inserted);
        assert_eq!(buffer.push(5, "duplicate"), PushResult::Duplicate);
        assert_eq!(buffer.pop_next(), Some("first"));
        assert_eq!(buffer.push(4, "late"), PushResult::TooLate);
        assert_eq!(buffer.dropped_packets(), 2);
    }

    #[test]
    fn jitter_buffer_handles_sequence_wraparound() {
        let mut buffer = JitterBuffer::new(3);
        buffer.start_at(u32::MAX);
        assert_eq!(buffer.push(u32::MAX, "max"), PushResult::Inserted);
        assert_eq!(buffer.push(0, "zero"), PushResult::Inserted);
        assert_eq!(buffer.push(1, "one"), PushResult::Inserted);

        assert_eq!(buffer.pop_next(), Some("max"));
        assert_eq!(buffer.pop_next(), Some("zero"));
        assert_eq!(buffer.pop_next(), Some("one"));
    }

    #[test]
    fn jitter_buffer_keeps_the_expected_packet_when_full_at_wraparound() {
        let mut buffer = JitterBuffer::new(2);
        buffer.start_at(u32::MAX);
        assert_eq!(buffer.push(u32::MAX, "max"), PushResult::Inserted);
        assert_eq!(buffer.push(0, "zero"), PushResult::Inserted);
        assert_eq!(buffer.push(1, "one"), PushResult::CapacityDrop);

        assert_eq!(buffer.pop_next(), Some("max"));
        assert_eq!(buffer.pop_next(), Some("zero"));
    }

    #[test]
    fn jitter_buffer_skips_a_missing_wrapped_sequence() {
        let mut buffer = JitterBuffer::new(4);
        buffer.start_at(u32::MAX);
        buffer.push(0, "zero");

        assert!(buffer.skip_missing());
        assert_eq!(buffer.pop_next(), Some("zero"));
    }

    #[test]
    fn audio_packet_rejects_oversized_payloads() {
        let packet = AudioPacket::new(session_id(), 1, 0, vec![0; MAX_AUDIO_PAYLOAD_BYTES + 1]);
        assert!(matches!(
            packet.encode(),
            Err(PacketError::PayloadTooLarge { .. })
        ));
    }
}
