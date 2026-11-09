//! The native NAT transport, ordinary host sockets. A TCP connect runs on a
//! worker thread (at most `MAX_CONNECTS` at once, process-wide) so the emulator never blocks on it.
//! `--hostfwd` listeners live here too: a browser has no sockets to listen on.

use std::collections::HashMap;
use std::io::{self, ErrorKind, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use super::transport::{Connect, FlowId, NatTransport};
use super::HostFwdSpec;

pub(super) const MAX_CONNECTS: usize = 8;
const MAX_HOSTFWD: usize = 8;

// A permit belongs to the blocking connect worker, not its guest flow. Removing a flow with RST
// must not free a slot while connect_timeout is still running on the host.
static CONNECTING: AtomicUsize = AtomicUsize::new(0);
pub(super) struct ConnectPermit<'a>(&'a AtomicUsize);
impl<'a> ConnectPermit<'a> {
    pub(super) fn acquire(counter: &'a AtomicUsize) -> Option<Self> {
        counter.try_update(Ordering::Relaxed, Ordering::Relaxed, |n| (n < MAX_CONNECTS).then_some(n + 1)).ok().map(|_| Self(counter))
    }
}
impl Drop for ConnectPermit<'_> {
    fn drop(&mut self) { self.0.fetch_sub(1, Ordering::Relaxed); }
}

fn connect(addr: SocketAddr) -> Option<Receiver<io::Result<TcpStream>>> {
    let permit = ConnectPermit::acquire(&CONNECTING)?;
    let (tx, rx) = channel();
    std::thread::Builder::new().name("nat-connect".into()).spawn(move || {
        let _permit = permit;
        let result = TcpStream::connect_timeout(&addr, std::time::Duration::from_secs(4))
            .and_then(|sock| { sock.set_nonblocking(true)?; sock.set_nodelay(true)?; Ok(sock) });
        let _ = tx.send(result);
    }).ok()?;
    Some(rx)
}

pub(super) enum StdTcp { Connecting(Receiver<io::Result<TcpStream>>), Connected(TcpStream), Failed(ErrorKind) }

/// One `--hostfwd` listener and where its connections go.
struct HostFwd {
    listener: TcpListener,
    guest_ip: [u8; 4],
    guest_port: u16,
}

/// `NatTransport` over `std::net`: what `Nat::new` uses.
pub struct StdTransport {
    tcp: HashMap<FlowId, StdTcp>,
    udp: HashMap<FlowId, UdpSocket>,
    forwards: Vec<HostFwd>,
    next: FlowId,
    resolver: [u8; 4],
}

impl Default for StdTransport {
    fn default() -> Self { Self::new() }
}

impl StdTransport {
    pub fn new() -> Self { Self { tcp: HashMap::new(), udp: HashMap::new(), forwards: Vec::new(), next: 0, resolver: host_resolver() } }

    /// Bind one host TCP port; its connections reach the NAT through `tcp_accept`, which delivers
    /// each as a SYN to `spec.guest_ip:guest_port`.
    pub fn listen(&mut self, spec: &HostFwdSpec) -> io::Result<SocketAddr> {
        if self.forwards.len() >= MAX_HOSTFWD {
            return Err(io::Error::other("too many --hostfwd listeners"));
        }
        let listener = TcpListener::bind(spec.bind)?;
        listener.set_nonblocking(true)?;
        let bound = listener.local_addr()?;
        self.forwards.push(HostFwd { listener, guest_ip: spec.guest_ip, guest_port: spec.guest_port });
        Ok(bound)
    }

    fn id(&mut self) -> FlowId {
        self.next = self.next.wrapping_add(1);
        self.next
    }

    /// A flow from a test's own socket or connect result.
    #[cfg(test)]
    pub(super) fn insert(&mut self, tcp: StdTcp) -> FlowId {
        let id = self.id();
        self.tcp.insert(id, tcp);
        id
    }

    fn stream(&mut self, id: FlowId) -> io::Result<&mut TcpStream> {
        match self.tcp.get_mut(&id) {
            Some(StdTcp::Connected(sock)) => Ok(sock),
            Some(StdTcp::Connecting(_)) => Err(ErrorKind::WouldBlock.into()),
            _ => Err(ErrorKind::NotConnected.into()),
        }
    }

    fn socket(&self, id: FlowId) -> io::Result<&UdpSocket> {
        self.udp.get(&id).ok_or_else(|| ErrorKind::NotConnected.into())
    }
}

impl NatTransport for StdTransport {
    fn tcp_connect(&mut self, dst: SocketAddrV4) -> Option<FlowId> {
        let pending = connect(SocketAddr::V4(dst))?;
        let id = self.id();
        self.tcp.insert(id, StdTcp::Connecting(pending));
        Some(id)
    }

    fn tcp_state(&mut self, id: FlowId) -> Connect {
        let Some(flow) = self.tcp.get_mut(&id) else { return Connect::Failed(ErrorKind::NotConnected) };
        if let StdTcp::Connecting(rx) = flow {
            let result = match rx.try_recv() {
                Ok(result) => result,
                Err(TryRecvError::Disconnected) => Err(ErrorKind::ConnectionAborted.into()),
                Err(TryRecvError::Empty) => return Connect::Pending,
            };
            *flow = match result { Ok(sock) => StdTcp::Connected(sock), Err(e) => StdTcp::Failed(e.kind()) };
        }
        match flow {
            StdTcp::Connecting(_) => Connect::Pending,
            StdTcp::Connected(_) => Connect::Open,
            StdTcp::Failed(kind) => Connect::Failed(*kind),
        }
    }

    fn tcp_read(&mut self, id: FlowId, buf: &mut [u8]) -> io::Result<usize> { self.stream(id)?.read(buf) }
    fn tcp_write(&mut self, id: FlowId, data: &[u8]) -> io::Result<usize> { self.stream(id)?.write(data) }

    fn tcp_shutdown(&mut self, id: FlowId) {
        if let Ok(sock) = self.stream(id) { let _ = sock.shutdown(std::net::Shutdown::Write); }
    }

    /// Dropping the socket closes it; std has no portable abortive close, so `abort` closes too.
    fn tcp_close(&mut self, id: FlowId, _abort: bool) { self.tcp.remove(&id); }

    /// A connected socket, which accepts replies only from its peer.
    fn udp_open(&mut self, dst: SocketAddrV4) -> io::Result<FlowId> {
        let sock = UdpSocket::bind("0.0.0.0:0")?;
        sock.connect(dst)?;
        sock.set_nonblocking(true)?;
        let id = self.id();
        self.udp.insert(id, sock);
        Ok(id)
    }

    fn udp_send(&mut self, id: FlowId, data: &[u8]) -> io::Result<usize> { self.socket(id)?.send(data) }
    fn udp_recv(&mut self, id: FlowId, buf: &mut [u8]) -> io::Result<usize> { self.socket(id)?.recv(buf) }
    fn udp_close(&mut self, id: FlowId) { self.udp.remove(&id); }

    fn resolver(&self) -> [u8; 4] { self.resolver }

    fn tcp_accept(&mut self) -> Option<(FlowId, [u8; 4], u16)> {
        for i in 0..self.forwards.len() {
            loop {
                let fwd = &self.forwards[i];
                match fwd.listener.accept() {
                    Ok((sock, _)) => {
                        if sock.set_nonblocking(true).is_err() || sock.set_nodelay(true).is_err() { continue; }
                        let (guest_ip, guest_port) = (fwd.guest_ip, fwd.guest_port);
                        let id = self.id();
                        self.tcp.insert(id, StdTcp::Connected(sock));
                        return Some((id, guest_ip, guest_port));
                    }
                    Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                    Err(_) => break,
                }
            }
        }
        None
    }
}

fn host_resolver() -> [u8; 4] {
    if let Ok(conf) = std::fs::read_to_string("/etc/resolv.conf") {
        for line in conf.lines() {
            if let Some(rest) = line.trim().strip_prefix("nameserver ") {
                if let Ok(a) = rest.trim().parse::<Ipv4Addr>() { return a.octets(); }
            }
        }
    }
    [1, 1, 1, 1]
}
