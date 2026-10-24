use audiosync_core::{AudioPacket, PacketError};
use std::io;
use std::net::SocketAddr;

use crate::UdpTransport;

trait DatagramSender {
    fn send_datagram(&self, buffer: &[u8], destination: SocketAddr) -> io::Result<usize>;
}

impl DatagramSender for UdpTransport {
    fn send_datagram(&self, buffer: &[u8], destination: SocketAddr) -> io::Result<usize> {
        self.send_to(buffer, destination)
    }
}

/// A single destination failure from a best-effort audio fan-out.
#[derive(Debug)]
pub struct AudioSendFailure {
    pub destination: SocketAddr,
    pub error: io::Error,
}

/// Results from sending one encoded audio packet to a set of listeners.
#[derive(Debug, Default)]
pub struct AudioFanoutReport {
    pub attempted: usize,
    pub sent: usize,
    pub bytes_sent: usize,
    pub failures: Vec<AudioSendFailure>,
}

impl AudioFanoutReport {
    pub const fn is_complete(&self) -> bool {
        self.attempted == self.sent && self.failures.is_empty()
    }

    pub const fn failed(&self) -> usize {
        self.attempted.saturating_sub(self.sent)
    }
}

/// Sends one already-formed [`AudioPacket`] to each active listener endpoint.
///
/// The packet is encoded once. A failure for one listener is recorded in the
/// report and does not prevent the remaining listeners from receiving the same
/// packet. This keeps a slow or disconnected listener from stopping the host's
/// fan-out loop.
#[derive(Debug)]
pub struct AudioPacketFanout<'a> {
    transport: &'a UdpTransport,
}

impl<'a> AudioPacketFanout<'a> {
    pub const fn new(transport: &'a UdpTransport) -> Self {
        Self { transport }
    }

    pub fn send_packet<I>(
        &self,
        packet: &AudioPacket,
        destinations: I,
    ) -> Result<AudioFanoutReport, PacketError>
    where
        I: IntoIterator<Item = SocketAddr>,
    {
        let encoded = packet.encode()?;
        Ok(fanout_encoded(self.transport, &encoded, destinations))
    }
}

fn fanout_encoded<S, I>(sender: &S, encoded: &[u8], destinations: I) -> AudioFanoutReport
where
    S: DatagramSender,
    I: IntoIterator<Item = SocketAddr>,
{
    let mut report = AudioFanoutReport::default();

    for destination in destinations {
        report.attempted = report.attempted.saturating_add(1);
        match sender.send_datagram(encoded, destination) {
            Ok(bytes) if bytes == encoded.len() => {
                report.sent = report.sent.saturating_add(1);
                report.bytes_sent = report.bytes_sent.saturating_add(bytes);
            }
            Ok(bytes) => report.failures.push(AudioSendFailure {
                destination,
                error: io::Error::new(
                    io::ErrorKind::WriteZero,
                    format!(
                        "UDP audio datagram was only partially sent: expected {}, wrote {bytes}",
                        encoded.len()
                    ),
                ),
            }),
            Err(error) => report
                .failures
                .push(AudioSendFailure { destination, error }),
        }
    }

    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use audiosync_core::SessionId;
    use std::time::Duration;

    fn session_id() -> SessionId {
        [0x51; 16]
    }

    #[test]
    fn fanout_sends_one_packet_to_each_destination() {
        let receiver_a = UdpTransport::bind("127.0.0.1:0").expect("first receiver should bind");
        let receiver_b = UdpTransport::bind("127.0.0.1:0").expect("second receiver should bind");
        receiver_a
            .set_read_timeout(Some(Duration::from_millis(250)))
            .expect("first receiver timeout should be configured");
        receiver_b
            .set_read_timeout(Some(Duration::from_millis(250)))
            .expect("second receiver timeout should be configured");

        let sender = UdpTransport::bind("127.0.0.1:0").expect("sender should bind");
        let packet = AudioPacket::new(session_id(), 7, 140_000, vec![1, 2, 3, 4]);
        let fanout = AudioPacketFanout::new(&sender);
        let report = fanout
            .send_packet(
                &packet,
                [
                    receiver_a
                        .local_addr()
                        .expect("first receiver has an address"),
                    receiver_b
                        .local_addr()
                        .expect("second receiver has an address"),
                ],
            )
            .expect("packet should encode");

        assert_eq!(report.attempted, 2);
        assert_eq!(report.sent, 2);
        assert_eq!(report.failed(), 0);
        assert_eq!(
            report.bytes_sent,
            2 * packet.encode().expect("packet encodes").len()
        );
        assert!(report.is_complete());

        for receiver in [&receiver_a, &receiver_b] {
            let mut bytes = [0_u8; 1_024];
            let (length, source) = receiver
                .recv_from(&mut bytes)
                .expect("fanout datagram should arrive");
            assert_eq!(source, sender.local_addr().expect("sender has an address"));
            assert_eq!(AudioPacket::decode(&bytes[..length]), Ok(packet.clone()));
        }
    }

    #[test]
    fn fanout_keeps_sending_after_one_destination_fails() {
        let packet = AudioPacket::new(session_id(), 8, 160_000, vec![5, 6]);
        struct FailingSender;

        impl DatagramSender for FailingSender {
            fn send_datagram(&self, buffer: &[u8], destination: SocketAddr) -> io::Result<usize> {
                if destination.port() == 1 {
                    Err(io::Error::new(
                        io::ErrorKind::ConnectionRefused,
                        "test failure",
                    ))
                } else {
                    Ok(buffer.len())
                }
            }
        }

        let encoded = packet.encode().expect("packet should encode");
        let report = fanout_encoded(
            &FailingSender,
            &encoded,
            [
                SocketAddr::from(([127, 0, 0, 1], 1)),
                SocketAddr::from(([127, 0, 0, 1], 2)),
            ],
        );

        assert_eq!(report.attempted, 2);
        assert_eq!(report.sent, 1);
        assert_eq!(report.failed(), 1);
        assert!(!report.is_complete());
    }
}
