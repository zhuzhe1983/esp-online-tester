//! The host side of the user-mode NAT as a trait, so a host without sockets can carry the guest's
//! flows over its own network.

use std::io;
use std::net::SocketAddrV4;

/// A flow of the transport, chosen by the transport.
pub type FlowId = u32;

/// Where a TCP connection begun by `NatTransport::tcp_connect` stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Connect { Pending, Open, Failed(io::ErrorKind) }

/// The outcome of a name lookup begun by `NatTransport::resolve`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolve {
    /// IPv4 answers and their TTL in seconds (may be empty: NOERROR without records).
    Addrs(Vec<[u8; 4]>, u32),
    /// DNS rcode: 2 SERVFAIL, 3 NXDOMAIN.
    Failed(u8),
}

/// The host side of the user-mode NAT: where guest flows really go. `StdTransport` (std::net
/// sockets, a connect worker thread per SYN) is the native default; a host without sockets can
/// queue the operations for its own network. Reads and writes follow non-blocking std::io rules:
/// `WouldBlock` = try at the next poll, `tcp_read` `Ok(0)` = peer EOF.
///
/// The NAT calls `tcp_close` exactly once for every flow `tcp_connect` or `tcp_accept` returned,
/// and `udp_close` once for every flow `udp_open` returned, when it forgets the flow.
pub trait NatTransport {
    /// Begin a TCP connection; None when no more may be opened now (the guest's SYN is dropped
    /// and its retransmission tries again).
    fn tcp_connect(&mut self, dst: SocketAddrV4) -> Option<FlowId>;
    fn tcp_state(&mut self, id: FlowId) -> Connect;
    fn tcp_read(&mut self, id: FlowId, buf: &mut [u8]) -> io::Result<usize>;
    fn tcp_write(&mut self, id: FlowId, data: &[u8]) -> io::Result<usize>;
    /// The guest sent FIN and every byte before it was written.
    fn tcp_shutdown(&mut self, id: FlowId);
    /// Forget the flow; `abort` resets it instead of closing it.
    fn tcp_close(&mut self, id: FlowId, abort: bool);
    fn udp_open(&mut self, dst: SocketAddrV4) -> io::Result<FlowId>;
    fn udp_send(&mut self, id: FlowId, data: &[u8]) -> io::Result<usize>;
    fn udp_recv(&mut self, id: FlowId, buf: &mut [u8]) -> io::Result<usize>;
    fn udp_close(&mut self, id: FlowId);
    /// Where DNS datagrams go when the transport does not resolve names itself.
    fn resolver(&self) -> [u8; 4];
    /// Start a name lookup (qtype 1 A, 28 AAAA). None: relay DNS datagrams to `resolver()`.
    fn resolve(&mut self, _name: &str, _qtype: u16) -> Option<u32> { None }
    /// A finished lookup started by `resolve`.
    fn resolved(&mut self) -> Option<(u32, Resolve)> { None }
    /// A connection from the host side to forward to the guest (`--hostfwd`), already open: its
    /// flow, the guest address and the guest port. The NAT sends the station a SYN from the gateway
    /// once it knows the station's MAC; until the guest's SYN-ACK nothing is read from the flow.
    fn tcp_accept(&mut self) -> Option<(FlowId, [u8; 4], u16)> { None }
}
