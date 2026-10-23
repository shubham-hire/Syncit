use std::io;
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::time::Duration;

/// A small wrapper that preserves the standard-library UDP socket API used by
/// the session state machine while leaving I/O errors as [`io::Error`] values.
#[derive(Debug)]
pub struct UdpTransport {
    socket: UdpSocket,
}

impl UdpTransport {
    /// Binds a UDP socket to the first address resolved from `addr`.
    pub fn bind<A: ToSocketAddrs>(addr: A) -> io::Result<Self> {
        Ok(Self {
            socket: UdpSocket::bind(addr)?,
        })
    }

    /// Wraps an already-created standard-library UDP socket.
    pub fn from_socket(socket: UdpSocket) -> Self {
        Self { socket }
    }

    /// Returns the local address assigned to this socket.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.socket.local_addr()
    }

    /// Sends a datagram to an address resolved by the standard library.
    pub fn send_to<A: ToSocketAddrs>(&self, buffer: &[u8], addr: A) -> io::Result<usize> {
        self.socket.send_to(buffer, addr)
    }

    /// Receives one datagram and returns its length and sender address.
    pub fn recv_from(&self, buffer: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        self.socket.recv_from(buffer)
    }

    /// Configures the socket's read timeout.
    pub fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.socket.set_read_timeout(timeout)
    }

    /// Switches the socket between blocking and nonblocking mode.
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.socket.set_nonblocking(nonblocking)
    }

    /// Clones the underlying socket using [`UdpSocket::try_clone`].
    pub fn try_clone(&self) -> io::Result<Self> {
        Ok(Self {
            socket: self.socket.try_clone()?,
        })
    }

    /// Borrows the underlying standard-library socket.
    pub fn as_socket(&self) -> &UdpSocket {
        &self.socket
    }

    /// Consumes the wrapper and returns the underlying standard-library socket.
    pub fn into_socket(self) -> UdpSocket {
        self.socket
    }
}

impl AsRef<UdpSocket> for UdpTransport {
    fn as_ref(&self) -> &UdpSocket {
        self.as_socket()
    }
}

impl From<UdpSocket> for UdpTransport {
    fn from(socket: UdpSocket) -> Self {
        Self::from_socket(socket)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn wrapper_sends_receives_and_clones_a_socket() {
        let receiver = UdpTransport::bind("127.0.0.1:0").expect("receiver should bind");
        let receiver_clone = receiver.try_clone().expect("socket should clone");
        let sender = UdpTransport::bind("127.0.0.1:0").expect("sender should bind");

        let destination = receiver.local_addr().expect("receiver has an address");
        assert_eq!(
            sender
                .send_to(b"hello", destination)
                .expect("datagram should send"),
            5
        );

        let mut buffer = [0_u8; 16];
        let (length, source) = receiver_clone
            .recv_from(&mut buffer)
            .expect("datagram should arrive");
        assert_eq!(&buffer[..length], b"hello");
        assert_eq!(source, sender.local_addr().expect("sender has an address"));
    }

    #[test]
    fn wrapper_forwards_socket_configuration() {
        let socket = UdpTransport::bind("127.0.0.1:0").expect("socket should bind");
        socket
            .set_read_timeout(Some(Duration::from_millis(25)))
            .expect("read timeout should be accepted");
        socket
            .set_nonblocking(true)
            .expect("nonblocking mode should be accepted");
        socket
            .set_nonblocking(false)
            .expect("blocking mode should be accepted");
    }
}
