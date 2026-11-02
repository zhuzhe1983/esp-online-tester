use super::*;
use super::std_transport::{ConnectPermit, StdTcp, MAX_CONNECTS};
use std::collections::HashMap;
use std::io::Read;
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::AtomicUsize;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex, MutexGuard};

mod counts;

fn flow() -> Tcp {
    Tcp { guest_mac: [2; 6], guest_ip: [10, 0, 2, 15], guest_port: 1234,
        dst_ip: [127, 0, 0, 1], dst_port: 80, inbound: false, state: FlowState::Closed,
        guest_write: GuestWrite::Open, host_closed: false,
        our_seq: 100, guest_seq: 200, guest_window: WINDOW,
        to_host: VecDeque::new(), unacked: VecDeque::new(), last_activity_us: 0 }
}

fn segment(seq: u32, ack: u32, flags: u8, data: &[u8]) -> Vec<u8> {
    segment_ports(1234, 80, seq, ack, flags, data)
}

fn segment_ports(sport: u16, dport: u16, seq: u32, ack: u32, flags: u8, data: &[u8]) -> Vec<u8> {
    let mut seg = vec![0; 20];
    seg[..2].copy_from_slice(&sport.to_be_bytes());
    seg[2..4].copy_from_slice(&dport.to_be_bytes());
    seg[4..8].copy_from_slice(&seq.to_be_bytes());
    seg[8..12].copy_from_slice(&ack.to_be_bytes());
    seg[12] = 0x50; seg[13] = flags;
    seg[14..16].copy_from_slice(&WINDOW.to_be_bytes());
    seg.extend_from_slice(data);
    seg
}

fn socket_pair() -> (TcpStream, TcpStream) {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let a = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    let b = listener.accept().unwrap().0;
    a.set_nonblocking(true).unwrap();
    b.set_read_timeout(Some(std::time::Duration::from_secs(1))).unwrap();
    (a, b)
}

/// A NAT over host sockets holding one TCP flow of the test's own.
fn std_nat(tcp: StdTcp) -> (Nat, FlowId) {
    let mut transport = StdTransport::new();
    let id = transport.insert(tcp);
    (Nat::with_transport(false, Box::new(transport)), id)
}

// The relay is nonblocking; wait for kernel loopback delivery without advancing emulated time.
fn poll_ready(nat: &mut Nat, now_us: u64) -> Vec<Vec<u8>> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
    loop {
        let frames = nat.poll(now_us);
        if !frames.is_empty() { return frames; }
        assert!(std::time::Instant::now() < deadline, "loopback delivery timed out");
        std::thread::yield_now();
    }
}

#[test]
fn syn_data_and_fin_share_retransmission_and_cumulative_ack() {
    let mut c = flow();
    for (flags, data) in [(SYN | ACK, Vec::new()), (PSH | ACK, b"hello".to_vec()), (FIN | ACK, Vec::new())] {
        let seq = c.our_seq;
        let frame = c.send(flags, data.clone(), 10);
        assert!(c.retransmit(RETRANSMIT_US).is_none());
        assert_eq!(c.retransmit(RETRANSMIT_US + 10).unwrap(), frame);
        assert_eq!(u32::from_be_bytes(frame[38..42].try_into().unwrap()), seq);
        assert_eq!(transport_checksum(&c.dst_ip, &c.guest_ip, 6, &frame[34..]), 0);
        c.acknowledge(c.our_seq);
        assert!(c.unacked.is_empty());
    }
    assert_eq!(c.our_seq, 107);
}

#[test]
fn partial_and_wrapping_ack_preserve_unsent_sequence_space() {
    let mut c = flow();
    c.our_seq = u32::MAX - 2;
    c.send(PSH | ACK, b"abcdef".to_vec(), 0);
    c.send(FIN | ACK, Vec::new(), 0);
    c.acknowledge(1000); // ACK beyond what we sent is ignored
    assert_eq!(c.in_flight(), 7);
    c.acknowledge(0);
    assert_eq!(c.unacked[0].data, b"def");
    assert_eq!(c.unacked[0].seq, 0);
    c.acknowledge(4);
    assert!(c.unacked.is_empty());
}

#[test]
fn guest_fin_is_in_order_and_idempotent_including_payload() {
    let mut c = flow();
    c.accept(201, b"", true);
    assert_eq!(c.guest_seq, 200);
    c.accept(200, b"abc", true);
    assert_eq!(c.guest_seq, 204);
    c.accept(200, b"abc", true);
    c.accept(203, b"", true);
    assert_eq!(c.guest_seq, 204);
    assert_eq!(c.to_host.make_contiguous(), b"abc");
    assert!(c.guest_write == GuestWrite::Draining);
}

#[test]
fn full_guest_buffer_does_not_ack_unstored_bytes_or_fin() {
    let mut c = flow();
    c.accept(200, &vec![7; WINDOW as usize], false);
    let next = c.guest_seq;
    c.accept(next, &[8], true);
    assert_eq!(c.guest_seq, next);
    assert_eq!(c.to_host.len(), WINDOW as usize);
    assert!(c.guest_write == GuestWrite::Open);
    let ack = c.segment(ACK, &[], c.our_seq);
    assert_eq!(&ack[48..50], &[0, 0]); // advertised window
}

struct ShortWriter { bytes: Vec<u8>, allowance: usize }
impl Write for ShortWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.allowance == 0 { return Err(ErrorKind::WouldBlock.into()); }
        let n = bytes.len().min(self.allowance).min(2);
        self.bytes.extend_from_slice(&bytes[..n]); self.allowance -= n;
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}

#[test]
fn short_write_and_would_block_preserve_exact_remaining_bytes() {
    let mut writer = ShortWriter { bytes: Vec::new(), allowance: 3 };
    let mut queue = VecDeque::from(b"abcdefgh".to_vec());
    assert_eq!(flush_pending(&mut writer, &mut queue).unwrap(), 3);
    assert_eq!(queue.make_contiguous(), b"defgh");
    assert_eq!(flush_pending(&mut writer, &mut queue).unwrap(), 0);
    writer.allowance = 100;
    assert_eq!(flush_pending(&mut writer, &mut queue).unwrap(), 5);
    assert!(queue.is_empty());
    assert_eq!(writer.bytes, b"abcdefgh");
}

#[test]
fn fin_waits_for_host_write_drain_and_remains_until_acknowledged() {
    let (socket, mut host) = socket_pair();
    let (mut nat, id) = std_nat(StdTcp::Connected(socket));
    let mut c = flow(); c.state = FlowState::Connected(id);
    nat.tcp.push(c);
    let reply = nat.tcp_in(&[2; 6], &[10, 0, 2, 15], &[127, 0, 0, 1], &segment(200, 100, ACK | FIN, b"hello"), 0);
    assert_eq!(reply.len(), 1);
    nat.poll(1);
    let mut received = Vec::new(); host.read_to_end(&mut received).unwrap();
    assert_eq!(received, b"hello");
    host.shutdown(std::net::Shutdown::Write).unwrap();
    let sent = poll_ready(&mut nat, 2);
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0][47], ACK | FIN);
    assert_eq!(nat.poll(RETRANSMIT_US + 2), sent);
    assert_eq!(nat.tcp.len(), 1);
    nat.tcp_in(&[2; 6], &[10, 0, 2, 15], &[127, 0, 0, 1], &segment(206, 101, ACK, &[]), RETRANSMIT_US + 3);
    nat.poll(RETRANSMIT_US + 4);
    assert!(nat.tcp[0].state == FlowState::TimeWait);
    let reply = nat.tcp_in(&[2; 6], &[10, 0, 2, 15], &[127, 0, 0, 1], &segment(205, 101, ACK | FIN, &[]), RETRANSMIT_US + 5);
    assert_eq!(reply.len(), 1);
    assert_eq!(u32::from_be_bytes(reply[0][42..46].try_into().unwrap()), 206);
    nat.poll(RETRANSMIT_US + 5 + TIME_WAIT_US);
    assert!(nat.tcp.is_empty());
}

#[test]
fn dropped_syn_ack_is_resent_on_timer_and_duplicate_syn() {
    let (socket, _host) = socket_pair();
    let (tx, rx) = channel(); tx.send(Ok(socket)).unwrap();
    let (mut nat, id) = std_nat(StdTcp::Connecting(rx));
    let mut c = flow(); c.state = FlowState::Connecting(id);
    nat.tcp.push(c);
    let syn_ack = nat.poll(0);
    assert_eq!(syn_ack.len(), 1);
    assert_eq!(nat.poll(RETRANSMIT_US), syn_ack);
    assert_eq!(nat.tcp_in(&[2; 6], &[10, 0, 2, 15], &[127, 0, 0, 1], &segment(199, 0, SYN, &[]), RETRANSMIT_US + 1), syn_ack);
    nat.tcp_in(&[2; 6], &[10, 0, 2, 15], &[127, 0, 0, 1], &segment(200, 101, ACK, &[]), RETRANSMIT_US + 2);
    assert!(nat.poll(RETRANSMIT_US * 2).is_empty());
}

#[test]
fn host_reads_stop_at_the_unacknowledged_window() {
    let (socket, mut host) = socket_pair();
    host.write_all(&vec![7; WINDOW as usize * 2]).unwrap();
    let (mut nat, id) = std_nat(StdTcp::Connected(socket));
    let mut c = flow(); c.state = FlowState::Connected(id);
    nat.tcp.push(c);
    let frames = poll_ready(&mut nat, 0);
    assert_eq!(frames.iter().map(|frame| frame.len() - 54).sum::<usize>(), WINDOW as usize);
    assert_eq!(nat.tcp[0].in_flight(), WINDOW as usize);
    assert!(nat.poll(1).is_empty());
    assert_eq!(nat.tcp[0].in_flight(), WINDOW as usize);
}

#[test]
fn zero_window_probe_recovers_a_lost_window_update() {
    let (socket, mut host) = socket_pair();
    host.write_all(b"hello").unwrap();
    let (mut nat, id) = std_nat(StdTcp::Connected(socket));
    let mut c = flow(); c.state = FlowState::Connected(id); c.guest_window = 0;
    nat.tcp.push(c);
    let probe = poll_ready(&mut nat, 0);
    assert_eq!(probe.len(), 1);
    assert_eq!(&probe[0][54..], b"h");
    assert_eq!(nat.poll(RETRANSMIT_US), probe);
    // The repeated byte reaches the reopened window even though its window update was lost.
    nat.tcp_in(&[2; 6], &[10, 0, 2, 15], &[127, 0, 0, 1], &segment(200, 101, ACK, &[]), RETRANSMIT_US + 1);
    let rest = poll_ready(&mut nat, RETRANSMIT_US + 2);
    assert_eq!(rest.len(), 1);
    assert_eq!(&rest[0][54..], b"ello");
}

#[test]
fn connector_permits_are_bounded_until_the_worker_drops_them() {
    let counter = AtomicUsize::new(0);
    let mut permits: Vec<_> = (0..MAX_CONNECTS).map(|_| ConnectPermit::acquire(&counter).unwrap()).collect();
    assert!(ConnectPermit::acquire(&counter).is_none());
    permits.pop();
    assert!(ConnectPermit::acquire(&counter).is_some());
}

#[test]
fn time_wait_is_evicted_only_after_a_connect_worker_is_admitted() {
    // Final-ACK retry records cannot monopolize live-flow slots during connection churn.
    // A scripted transport decides admission, so concurrent real workers cannot affect this test.
    let mock = Mock::default();
    let mut nat = mock.nat();
    for port in 0..MAX_FLOWS {
        let mut c = flow(); c.guest_port = port as u16; c.state = FlowState::TimeWait;
        nat.tcp.push(c);
    }
    mock.s().refuse_connect = true;
    nat.tcp_in(&[2; 6], &[10, 0, 2, 15], &[127, 0, 0, 1], &segment(199, 0, SYN, &[]), 0);
    assert_eq!(nat.tcp.len(), MAX_FLOWS);
    assert!(nat.tcp.iter().all(|c| c.state == FlowState::TimeWait));
    mock.s().refuse_connect = false;
    nat.tcp_in(&[2; 6], &[10, 0, 2, 15], &[127, 0, 0, 1], &segment(199, 0, SYN, &[]), 0);
    assert_eq!(nat.tcp.len(), MAX_FLOWS);
    assert!(matches!(nat.tcp.last().unwrap().state, FlowState::Connecting(_)));
}

#[test]
fn udp_at_capacity_reuses_existing_flows_and_evicts_the_least_recent() {
    let receiver = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    receiver.set_read_timeout(Some(std::time::Duration::from_secs(1))).unwrap();
    let port = receiver.local_addr().unwrap().port();
    let mut nat = Nat::new(false);
    let mut bytes = [0; 16];
    for sport in 0..MAX_FLOWS as u16 {
        nat.udp_out(&[2; 6], &[10, 0, 2, 15], sport, &[127, 0, 0, 1], &[127, 0, 0, 1], port, b"one", sport as u64);
        assert_eq!(receiver.recv_from(&mut bytes).unwrap().0, 3);
    }
    nat.udp_out(&[2; 6], &[10, 0, 2, 15], 0, &[127, 0, 0, 1], &[127, 0, 0, 1], port, b"reuse", 100);
    assert_eq!(receiver.recv_from(&mut bytes).unwrap().0, 5);
    assert_eq!(nat.udp_flows, MAX_FLOWS as u64);
    nat.udp_out(&[2; 6], &[10, 0, 2, 15], 1234, &[127, 0, 0, 1], &[127, 0, 0, 1], port, b"new", 101);
    assert_eq!(receiver.recv_from(&mut bytes).unwrap().0, 3);
    assert_eq!(nat.udp.len(), MAX_FLOWS);
    assert_eq!(nat.udp_evicted, 1);
    assert!(nat.udp.iter().any(|f| f.guest_port == 0));
    assert!(!nat.udp.iter().any(|f| f.guest_port == 1));
    assert!(nat.udp.iter().any(|f| f.guest_port == 1234));
    assert_eq!(nat.udp_send_errors, 0);
}

#[test]
fn short_and_invalid_tcp_headers_are_rejected() {
    let mut nat = Nat::new(false);
    for len in 0..64 {
        for offset in 0..16 {
            let mut seg = vec![0; len];
            if len > 12 { seg[12] = offset << 4; }
            assert!(nat.tcp_in(&[2; 6], &[10, 0, 2, 15], &[127, 0, 0, 1], &seg, 0).is_empty());
        }
    }
    assert!(nat.tcp.is_empty());
}

#[test]
fn udp_ignores_other_senders_and_preserves_dns_reply_address() {
    let resolver = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    resolver.set_read_timeout(Some(std::time::Duration::from_secs(1))).unwrap();
    let intruder = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let mut nat = Nat::new(false);
    nat.udp_out(&[2; 6], &[10, 0, 2, 15], 1234, &[127, 0, 0, 1], &[10, 0, 2, 3], resolver.local_addr().unwrap().port(), b"query", 0);
    let mut bytes = [0; 32];
    let (_, from) = resolver.recv_from(&mut bytes).unwrap();
    intruder.send_to(b"forged", from).unwrap();
    resolver.send_to(b"answer", from).unwrap();
    let frames = poll_ready(&mut nat, 1);
    assert_eq!(frames.len(), 1);
    assert_eq!(&frames[0][26..30], &[10, 0, 2, 3]);
    assert_eq!(&frames[0][42..], b"answer");
}

// ---- A scripted transport: the NAT's side of the trait, without host sockets ----

/// What the scripted transport does and what the NAT asked of it.
#[derive(Default)]
struct Script {
    next: FlowId,
    /// `tcp_connect` answers None (no slot)
    refuse_connect: bool,
    connects: Vec<SocketAddrV4>,
    /// `tcp_state` per flow; Pending when unset
    states: HashMap<FlowId, Connect>,
    /// what `tcp_read` returns, in order; an empty chunk is EOF and stays
    reads: HashMap<FlowId, VecDeque<Vec<u8>>>,
    written: HashMap<FlowId, Vec<u8>>,
    /// bytes `tcp_write` still takes before WouldBlock; None takes everything
    write_room: Option<usize>,
    shutdowns: Vec<FlowId>,
    closes: Vec<(FlowId, bool)>,
    udp_opens: Vec<SocketAddrV4>,
    udp_closes: Vec<FlowId>,
    /// `resolve` starts lookups (otherwise None: relay DNS)
    resolves: bool,
    lookups: Vec<(u32, String, u16)>,
    answers: VecDeque<(u32, Resolve)>,
    /// what `tcp_accept` hands over, in order: the flow, the guest address and port
    incoming: VecDeque<(FlowId, [u8; 4], u16)>,
}

#[derive(Clone, Default)]
struct Mock(Arc<Mutex<Script>>);

impl Mock {
    fn s(&self) -> MutexGuard<'_, Script> { self.0.lock().unwrap() }
    fn nat(&self) -> Nat { Nat::with_transport(false, Box::new(self.clone())) }
}

impl NatTransport for Mock {
    fn tcp_connect(&mut self, dst: SocketAddrV4) -> Option<FlowId> {
        let mut s = self.s();
        if s.refuse_connect { return None; }
        s.connects.push(dst);
        s.next += 1;
        Some(s.next)
    }
    fn tcp_state(&mut self, id: FlowId) -> Connect { self.s().states.get(&id).copied().unwrap_or(Connect::Pending) }
    fn tcp_read(&mut self, id: FlowId, buf: &mut [u8]) -> io::Result<usize> {
        let mut s = self.s();
        let Some(chunk) = s.reads.get_mut(&id).and_then(|q| q.front_mut()) else { return Err(ErrorKind::WouldBlock.into()) };
        if chunk.is_empty() { return Ok(0); }
        let n = chunk.len().min(buf.len());
        buf[..n].copy_from_slice(&chunk[..n]);
        chunk.drain(..n);
        if chunk.is_empty() { s.reads.get_mut(&id).unwrap().pop_front(); }
        Ok(n)
    }
    fn tcp_write(&mut self, id: FlowId, data: &[u8]) -> io::Result<usize> {
        let mut s = self.s();
        let n = s.write_room.map_or(data.len(), |room| room.min(data.len()));
        if n == 0 { return Err(ErrorKind::WouldBlock.into()); }
        if let Some(room) = &mut s.write_room { *room -= n; }
        s.written.entry(id).or_default().extend_from_slice(&data[..n]);
        Ok(n)
    }
    fn tcp_shutdown(&mut self, id: FlowId) { self.s().shutdowns.push(id); }
    fn tcp_close(&mut self, id: FlowId, abort: bool) { self.s().closes.push((id, abort)); }
    fn udp_open(&mut self, dst: SocketAddrV4) -> io::Result<FlowId> {
        let mut s = self.s();
        s.udp_opens.push(dst);
        s.next += 1;
        Ok(s.next)
    }
    fn udp_send(&mut self, _id: FlowId, data: &[u8]) -> io::Result<usize> { Ok(data.len()) }
    fn udp_recv(&mut self, _id: FlowId, _buf: &mut [u8]) -> io::Result<usize> { Err(ErrorKind::WouldBlock.into()) }
    fn udp_close(&mut self, id: FlowId) { self.s().udp_closes.push(id); }
    fn resolver(&self) -> [u8; 4] { [9, 9, 9, 9] }
    fn resolve(&mut self, name: &str, qtype: u16) -> Option<u32> {
        let mut s = self.s();
        if !s.resolves { return None; }
        let qid = s.lookups.len() as u32 + 1;
        s.lookups.push((qid, name.to_string(), qtype));
        Some(qid)
    }
    fn resolved(&mut self) -> Option<(u32, Resolve)> { self.s().answers.pop_front() }
    fn tcp_accept(&mut self) -> Option<(FlowId, [u8; 4], u16)> { self.s().incoming.pop_front() }
}

const GUEST: [u8; 4] = [10, 0, 2, 15];
const REMOTE: [u8; 4] = [93, 184, 215, 14];

fn guest_tcp(nat: &mut Nat, seg: Vec<u8>, now_us: u64) -> Vec<Vec<u8>> { nat.tcp_in(&[2; 6], &GUEST, &REMOTE, &seg, now_us) }
fn flags(frame: &[u8]) -> u8 { frame[47] }
fn seq_of(frame: &[u8]) -> u32 { u32::from_be_bytes(frame[38..42].try_into().unwrap()) }
fn ack_of(frame: &[u8]) -> u32 { u32::from_be_bytes(frame[42..46].try_into().unwrap()) }
fn window_of(frame: &[u8]) -> u16 { u16::from_be_bytes([frame[48], frame[49]]) }

/// SYN, `Open`, SYN-ACK and the guest's ACK: an established flow, and our next sequence number.
fn open_flow(mock: &Mock, nat: &mut Nat) -> (FlowId, u32) {
    assert!(guest_tcp(nat, segment(199, 0, SYN, &[]), 0).is_empty());
    let id = mock.s().next;
    mock.s().states.insert(id, Connect::Open);
    let syn_ack = nat.poll(1);
    let next = seq_of(&syn_ack[0]).wrapping_add(1);
    assert!(guest_tcp(nat, segment(200, next, ACK, &[]), 2).is_empty());
    (id, next)
}

#[test]
fn syn_opens_a_transport_flow_and_guest_data_is_written_to_it() {
    let mock = Mock::default();
    let mut nat = mock.nat();
    assert!(guest_tcp(&mut nat, segment(199, 0, SYN, &[]), 0).is_empty());
    assert_eq!(mock.s().connects, [SocketAddrV4::new(Ipv4Addr::from(REMOTE), 80)]);
    assert!(nat.poll(1).is_empty(), "no SYN-ACK while the transport is still connecting");
    let id = mock.s().next;
    mock.s().states.insert(id, Connect::Open);
    let syn_ack = nat.poll(2);
    assert_eq!(syn_ack.len(), 1);
    assert_eq!(flags(&syn_ack[0]), SYN | ACK);
    assert_eq!(ack_of(&syn_ack[0]), 200);
    assert_eq!(nat.tcp_opened, 1);
    let next = seq_of(&syn_ack[0]).wrapping_add(1);
    let reply = guest_tcp(&mut nat, segment(200, next, ACK | PSH, b"GET /"), 3);
    assert_eq!(ack_of(&reply[0]), 205);
    nat.poll(4);
    assert_eq!(mock.s().written[&id], b"GET /");
    assert_eq!(nat.bytes_to_host, 5);
}

#[test]
fn would_block_closes_the_guest_window_without_losing_bytes() {
    let mock = Mock::default();
    let mut nat = mock.nat();
    let (id, next) = open_flow(&mock, &mut nat);
    mock.s().write_room = Some(0);
    let first: Vec<u8> = (0..3000u32).map(|i| i as u8).collect();
    let second: Vec<u8> = (0..WINDOW as u32 - 3000).map(|i| (i * 7) as u8).collect();
    let reply = guest_tcp(&mut nat, segment(200, next, ACK | PSH, &first), 3);
    assert_eq!(window_of(&reply[0]), WINDOW - 3000);
    assert!(nat.poll(4).is_empty());
    let reply = guest_tcp(&mut nat, segment(3200, next, ACK | PSH, &second), 5);
    assert_eq!(window_of(&reply[0]), 0);
    // A byte beyond the closed window is not acknowledged.
    let reply = guest_tcp(&mut nat, segment(200 + WINDOW as u32, next, ACK | PSH, b"x"), 6);
    assert_eq!(ack_of(&reply[0]), 200 + WINDOW as u32);
    mock.s().write_room = Some(1000);
    let update = nat.poll(7);
    assert_eq!(update.len(), 1, "the drained bytes reopen the window");
    assert_eq!(window_of(&update[0]), 1000);
    mock.s().write_room = None;
    nat.poll(8);
    assert_eq!(mock.s().written[&id], [first, second].concat());
    assert!(mock.s().closes.is_empty());
}

#[test]
fn transport_eof_becomes_a_guest_fin_and_the_close_is_graceful() {
    let mock = Mock::default();
    let mut nat = mock.nat();
    let (id, next) = open_flow(&mock, &mut nat);
    mock.s().reads.insert(id, VecDeque::from([b"hi".to_vec(), Vec::new()]));
    let frames = nat.poll(3);
    assert_eq!(frames.len(), 2);
    assert_eq!((flags(&frames[0]), &frames[0][54..]), (PSH | ACK, &b"hi"[..]));
    assert_eq!(flags(&frames[1]), FIN | ACK);
    let reply = guest_tcp(&mut nat, segment(200, next + 3, ACK | FIN, &[]), 4);
    assert_eq!(ack_of(&reply[0]), 201);
    nat.poll(5);
    assert_eq!(mock.s().shutdowns, [id]);
    assert!(mock.s().closes.is_empty());
    nat.poll(6);
    assert!(nat.tcp[0].state == FlowState::TimeWait);
    assert_eq!(mock.s().closes, [(id, false)]);
}

#[test]
fn refused_connect_resets_the_guest_and_forgets_the_flow() {
    let mock = Mock::default();
    let mut nat = mock.nat();
    guest_tcp(&mut nat, segment(199, 0, SYN, &[]), 0);
    let id = mock.s().next;
    mock.s().states.insert(id, Connect::Failed(ErrorKind::ConnectionRefused));
    let frames = nat.poll(1);
    assert_eq!(frames.len(), 1);
    assert_eq!(flags(&frames[0]), RST | ACK);
    assert_eq!(ack_of(&frames[0]), 200);
    assert_eq!(nat.tcp_refused, 1);
    assert_eq!(mock.s().closes, [(id, true)]);
    assert!(nat.tcp.is_empty());
}

#[test]
fn a_full_transport_drops_the_syn_until_its_retransmission() {
    let mock = Mock::default();
    let mut nat = mock.nat();
    mock.s().refuse_connect = true;
    assert!(guest_tcp(&mut nat, segment(199, 0, SYN, &[]), 0).is_empty());
    assert!(nat.tcp.is_empty());
    assert!(nat.poll(1).is_empty());
    mock.s().refuse_connect = false;
    assert!(guest_tcp(&mut nat, segment(199, 0, SYN, &[]), 2).is_empty());
    assert_eq!(nat.tcp.len(), 1);
    assert_eq!(mock.s().connects.len(), 1);
}

#[test]
fn guest_rst_aborts_the_transport_flow() {
    let mock = Mock::default();
    let mut nat = mock.nat();
    let (id, next) = open_flow(&mock, &mut nat);
    guest_tcp(&mut nat, segment(200, next, RST, &[]), 3);
    assert_eq!(mock.s().closes, [(id, true)]);
    nat.poll(4);
    assert!(nat.tcp.is_empty());
    assert_eq!(mock.s().closes.len(), 1, "a flow is closed once");
}

#[test]
fn abort_all_resets_every_transport_flow() {
    let mock = Mock::default();
    let mut nat = mock.nat();
    let (open, _) = open_flow(&mock, &mut nat);
    nat.tcp_in(&[2; 6], &GUEST, &[1, 2, 3, 4], &segment(500, 0, SYN, &[]), 3);
    let connecting = mock.s().next;
    let mut done = flow(); done.state = FlowState::TimeWait;
    nat.tcp.push(done);
    nat.udp_out(&[2; 6], &GUEST, 4000, &REMOTE, &REMOTE, 7, b"ping", 4);
    let udp = mock.s().next;
    nat.abort_all();
    assert_eq!(mock.s().closes, [(open, true), (connecting, true)]);
    assert_eq!(mock.s().udp_closes, [udp]);
    assert!(nat.tcp.is_empty() && nat.udp.is_empty());
    assert!(nat.poll(RETRANSMIT_US * 2).is_empty());
}

#[test]
fn a_guest_reboot_aborts_the_nat_flows() {
    let mock = Mock::default();
    let mut net = crate::net::VirtualNet::new(false);
    net.nat = Some(mock.nat());
    let mut link = crate::wifi::StationLink::default();
    link.attach(None, Some(net));
    let nat = link.net_mut().unwrap().nat.as_mut().unwrap();
    nat.tcp_in(&[2; 6], &GUEST, &REMOTE, &segment(199, 0, SYN, &[]), 0);
    let link = link.surviving_reboot();
    assert_eq!(mock.s().closes, [(1, true)]);
    assert!(link.net().unwrap().nat.is_some());
}

#[test]
fn an_accepted_flow_waits_for_the_station_then_runs_like_an_outbound_one() {
    let mock = Mock::default();
    let mut nat = mock.nat();
    mock.s().incoming.push_back((40, GUEST, 80));
    assert!(nat.poll(0).is_empty(), "no SYN before the station MAC is known");
    nat.set_station([2; 6]);
    let syn = nat.poll(1);
    assert_eq!(syn.len(), 1);
    assert_eq!(flags(&syn[0]), SYN);
    assert_eq!((&syn[0][26..30], &syn[0][30..34]), (&[10, 0, 2, 2][..], &GUEST[..]));
    let eph = u16::from_be_bytes([syn[0][34], syn[0][35]]);
    mock.s().reads.insert(40, VecDeque::from([b"GET /".to_vec()]));
    assert!(nat.poll(2).iter().all(|f| flags(f) == SYN), "nothing is read before the guest's SYN-ACK");
    let ack = nat.tcp_in(&[2; 6], &GUEST, &GATEWAY_IP, &segment_ports(80, eph, 1000, seq_of(&syn[0]) + 1, SYN | ACK, &[]), 3);
    assert_eq!((ack.len(), flags(&ack[0])), (1, ACK));
    assert_eq!((nat.tcp_accepted, nat.tcp_opened), (1, 1));
    let data = nat.poll(4);
    assert_eq!((flags(&data[0]), &data[0][54..]), (PSH | ACK, &b"GET /"[..]));
    nat.tcp_in(&[2; 6], &GUEST, &GATEWAY_IP, &segment_ports(80, eph, 1001, 0, RST, &[]), 5);
    nat.poll(6);
    assert_eq!(mock.s().closes, [(40, true)], "an accepted flow is closed once");
}

#[test]
fn accepted_flows_past_the_bound_and_on_abort_all_are_closed() {
    let mock = Mock::default();
    let mut nat = mock.nat();
    mock.s().incoming.extend((1..=MAX_PENDING as u32 + 2).map(|id| (id, GUEST, 80)));
    nat.poll(0);
    assert_eq!(nat.pending.len(), MAX_PENDING);
    assert_eq!(mock.s().closes, [(1, false), (2, false)], "the oldest give way");
    nat.abort_all();
    assert!(nat.pending.is_empty());
    assert_eq!(mock.s().closes.len(), 2 + MAX_PENDING);
    assert!(mock.s().closes[2..].iter().all(|&(_, abort)| abort));
    nat.set_station([2; 6]);
    assert!(nat.poll(1).is_empty(), "nothing left to forward");
}

fn dns_query_msg(txid: u16, name: &str, qtype: u16) -> Vec<u8> {
    let mut q = txid.to_be_bytes().to_vec();
    q.extend_from_slice(&[0x01, 0x00, 0, 1, 0, 0, 0, 0, 0, 0]);
    for label in name.split('.') { q.push(label.len() as u8); q.extend_from_slice(label.as_bytes()); }
    q.push(0);
    q.extend_from_slice(&qtype.to_be_bytes());
    q.extend_from_slice(&[0, 1]);
    q
}

/// A UDP datagram from the guest (port 5353) to `dst`:`dport`, as an Ethernet frame.
fn guest_udp(dst: [u8; 4], dport: u16, payload: &[u8]) -> Vec<u8> {
    let udp = udp_packet(&GUEST, &dst, 5353, dport, payload);
    ethernet(&GATEWAY_MAC, &[2; 6], 0x0800, &ip_packet(17, &GUEST, &dst, &udp))
}

fn mock_net(mock: &Mock) -> crate::net::VirtualNet {
    let mut net = crate::net::VirtualNet::new(false);
    net.nat = Some(mock.nat());
    net
}

#[test]
fn a_resolving_transport_answers_dns_from_the_guest_resolver_address() {
    let mock = Mock::default();
    mock.s().resolves = true;
    let mut net = mock_net(&mock);
    assert!(net.handle(&guest_udp(net.dns_ip, 53, &dns_query_msg(0xbeef, "example.com", 1)), 0).is_empty());
    assert!(mock.s().udp_opens.is_empty(), "the query is not relayed");
    assert_eq!(mock.s().lookups, [(1, "example.com".to_string(), 1)]);
    assert!(net.poll(1).is_empty(), "the answer waits for the transport");
    mock.s().answers.push_back((7, Resolve::Addrs(vec![[1, 1, 1, 1]], 5))); // nobody asked
    mock.s().answers.push_back((1, Resolve::Addrs(vec![REMOTE], 300)));
    let frames = net.poll(2);
    assert_eq!(frames.len(), 1);
    let f = &frames[0];
    assert_eq!(&f[26..30], &[10, 0, 2, 3]);
    assert_eq!(&f[30..34], &GUEST);
    assert_eq!(&f[34..38], &[0, 53, 0x14, 0xe9]); // 53 -> 5353
    assert_eq!(transport_checksum(&[10, 0, 2, 3], &GUEST, 17, &f[34..]), 0);
    let dns = &f[42..];
    assert_eq!(&dns[..8], &[0xbe, 0xef, 0x81, 0x80, 0, 1, 0, 1]);
    assert_eq!(&dns[dns.len() - 10..], &[0, 0, 1, 44, 0, 4, 93, 184, 215, 14]); // TTL 300, the address
    assert_eq!(net.nat.as_ref().unwrap().dns_answers, 1);
}

#[test]
fn a_failed_lookup_is_nxdomain_and_an_empty_one_has_no_records() {
    let mock = Mock::default();
    mock.s().resolves = true;
    let mut net = mock_net(&mock);
    net.handle(&guest_udp(net.dns_ip, 53, &dns_query_msg(1, "nowhere.invalid", 1)), 0);
    net.handle(&guest_udp(net.dns_ip, 53, &dns_query_msg(2, "example.com", 28)), 0);
    mock.s().answers.push_back((1, Resolve::Failed(3)));
    mock.s().answers.push_back((2, Resolve::Addrs(Vec::new(), 60)));
    let frames = net.poll(1);
    assert_eq!(frames.len(), 2);
    assert_eq!(&frames[0][42..50], &[0, 1, 0x81, 0x83, 0, 1, 0, 0]);
    assert_eq!(&frames[1][42..50], &[0, 2, 0x81, 0x80, 0, 1, 0, 0]);
    assert_eq!(frames[1].len(), 42 + dns_query_msg(2, "example.com", 28).len());
}

#[test]
fn dns_is_relayed_when_the_transport_does_not_resolve() {
    let mock = Mock::default();
    let mut net = mock_net(&mock);
    net.handle(&guest_udp(net.dns_ip, 53, &dns_query_msg(1, "example.com", 1)), 0);
    assert_eq!(mock.s().udp_opens, [SocketAddrV4::new(Ipv4Addr::new(9, 9, 9, 9), 53)]);
    // A resolving transport is asked only for A and AAAA.
    mock.s().resolves = true;
    net.handle(&guest_udp(net.dns_ip, 53, &dns_query_msg(2, "example.com", 15)), 0);
    assert!(mock.s().lookups.is_empty());
    assert_eq!(mock.s().udp_opens.len(), 1, "the same guest flow carries it");
}

#[test]
fn at_most_32_lookups_wait_and_the_oldest_gives_way() {
    let mock = Mock::default();
    mock.s().resolves = true;
    let mut nat = mock.nat();
    for txid in 1..=33u16 {
        assert!(nat.dns_query(&[2; 6], &GUEST, 5353, &[10, 0, 2, 3], &dns_query_msg(txid, "example.com", 1), 0));
    }
    assert_eq!(nat.dns.len(), MAX_DNS_PENDING);
    mock.s().answers.extend([1, 2, 33].map(|qid| (qid, Resolve::Addrs(vec![REMOTE], 60))));
    let frames = nat.poll(1);
    assert_eq!(frames.iter().map(|f| u16::from_be_bytes([f[42], f[43]])).collect::<Vec<_>>(), [2, 33]);
    nat.poll(DNS_PENDING_US);
    assert!(nat.dns.is_empty(), "unanswered lookups expire");
}

#[test]
fn local_ntp_answers_sntp_here_even_with_a_nat() {
    let mut request = [0u8; 48]; request[0] = 0x23;
    let server = [162, 159, 200, 1];
    let mock = Mock::default();
    let mut net = mock_net(&mock);
    assert!(net.handle(&guest_udp(server, 123, &request), 0).is_empty());
    assert_eq!(mock.s().udp_opens, [SocketAddrV4::new(Ipv4Addr::from(server), 123)]);
    let mock = Mock::default();
    let mut net = mock_net(&mock);
    net.local_ntp = true;
    let reply = net.handle(&guest_udp(server, 123, &request), 0);
    assert_eq!(reply.len(), 1);
    assert_eq!(&reply[0][26..30], &server);
    assert_eq!(net.ntp_answers, 1);
    assert!(mock.s().udp_opens.is_empty());
}

/// A NAT over a `StdTransport` with one `--hostfwd` listener, and the address it bound.
fn listening(spec: HostFwdSpec) -> (Nat, SocketAddr) {
    let mut transport = StdTransport::new();
    let bound = transport.listen(&spec).unwrap();
    (Nat::with_transport(false, Box::new(transport)), bound)
}

#[test]
fn hostfwd_spec_parses_the_documented_tcp_form() {
    let spec = HostFwdSpec::parse("tcp:127.0.0.1:8080-:80").unwrap();
    assert_eq!(spec.bind, SocketAddr::from((Ipv4Addr::LOCALHOST, 8080)));
    assert_eq!(spec.guest_ip, [10, 0, 2, 15]);
    assert_eq!(spec.guest_port, 80);
    let spec = HostFwdSpec::parse("tcp::8099-10.0.2.15:443").unwrap();
    assert_eq!(spec.bind, SocketAddr::from((Ipv4Addr::LOCALHOST, 8099)));
    assert_eq!(spec.guest_ip, [10, 0, 2, 15]);
    assert_eq!(spec.guest_port, 443);
    let spec = HostFwdSpec::parse("tcp:0.0.0.0:0-:1").unwrap();
    assert_eq!(spec.bind.ip(), IpAddr::V4(Ipv4Addr::UNSPECIFIED));
    assert_eq!(spec.guest_port, 1);
    for bad in ["udp:127.0.0.1:8080-:80", "tcp:127.0.0.1:8080:80", "tcp:127.0.0.1:8080-:0", "tcp:nope:8080-:80", "tcp::65536-:80", ""] {
        assert!(HostFwdSpec::parse(bad).is_err(), "{bad}");
    }
}

#[test]
fn hostfwd_holds_until_the_station_mac_is_known_then_relays_both_ways() {
    let (mut nat, bound) = listening(HostFwdSpec { bind: SocketAddr::from((Ipv4Addr::LOCALHOST, 0)), guest_ip: [10, 0, 2, 15], guest_port: 80 });
    let connector = std::thread::spawn(move || {
        let sock = TcpStream::connect(bound).unwrap();
        sock.set_nonblocking(true).unwrap();
        sock.set_nodelay(true).unwrap();
        sock
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
    while !connector.is_finished() {
        assert!(nat.poll(0).is_empty(), "no SYN before the station MAC is known");
        assert!(std::time::Instant::now() < deadline, "host connect timed out");
        std::thread::yield_now();
    }
    let mut host = connector.join().unwrap();
    assert!(nat.poll(0).is_empty(), "accepted sockets wait for the station MAC");
    nat.set_station([2; 6]);
    let frames = poll_ready(&mut nat, 1);
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0][47], SYN);
    assert_eq!(&frames[0][26..30], &[10, 0, 2, 2]);
    assert_eq!(&frames[0][30..34], &[10, 0, 2, 15]);
    let eph = u16::from_be_bytes(frames[0][34..36].try_into().unwrap());
    let syn_seq = u32::from_be_bytes(frames[0][38..42].try_into().unwrap());
    let ack = nat.tcp_in(&[2; 6], &[10, 0, 2, 15], &[10, 0, 2, 2], &segment_ports(80, eph, 1000, syn_seq + 1, SYN | ACK, &[]), 2);
    assert_eq!(ack.len(), 1);
    assert_eq!(ack[0][47], ACK);
    assert_eq!(nat.tcp_accepted, 1);
    let again = nat.tcp_in(&[2; 6], &[10, 0, 2, 15], &[10, 0, 2, 2], &segment_ports(80, eph, 1000, syn_seq + 1, SYN | ACK, &[]), 2);
    assert_eq!(again.len(), 1);
    assert_eq!(again[0][47], ACK);
    assert_eq!(nat.tcp_accepted, 1);
    host.write_all(b"GET /").unwrap();
    let req = poll_ready(&mut nat, 3);
    assert_eq!(req.len(), 1);
    assert_eq!(&req[0][54..], b"GET /");
    nat.tcp_in(&[2; 6], &[10, 0, 2, 15], &[10, 0, 2, 2], &segment_ports(80, eph, 1001, syn_seq + 1 + 5, ACK | PSH, b"ok"), 4);
    nat.poll(5);
    let mut got = [0; 8];
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
    let mut n = 0;
    while n < 2 {
        match host.read(&mut got[n..]) {
            Ok(0) => break,
            Ok(k) => n += k,
            Err(e) if e.kind() == ErrorKind::WouldBlock && std::time::Instant::now() < deadline => {
                nat.poll(6);
                std::thread::yield_now();
            }
            Err(e) => panic!("{e}"),
        }
    }
    assert_eq!(&got[..2], b"ok");
}

#[test]
fn hostfwd_counts_a_guest_rst_to_the_forwarded_syn_as_refused() {
    let (mut nat, bound) = listening(HostFwdSpec { bind: SocketAddr::from((Ipv4Addr::LOCALHOST, 0)), guest_ip: [10, 0, 2, 15], guest_port: 4444 });
    let connector = std::thread::spawn(move || TcpStream::connect(bound).unwrap());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
    while !connector.is_finished() {
        nat.poll(0);
        assert!(std::time::Instant::now() < deadline, "host connect timed out");
        std::thread::yield_now();
    }
    let _host = connector.join().unwrap();
    nat.set_station([2; 6]);
    let frames = poll_ready(&mut nat, 1);
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0][47], SYN);
    let eph = u16::from_be_bytes(frames[0][34..36].try_into().unwrap());
    let syn_seq = u32::from_be_bytes(frames[0][38..42].try_into().unwrap());
    assert_eq!(nat.tcp_refused, 0);
    let out = nat.tcp_in(&[2; 6], &[10, 0, 2, 15], &[10, 0, 2, 2], &segment_ports(4444, eph, 0, syn_seq + 1, RST | ACK, &[]), 2);
    assert!(out.is_empty());
    assert_eq!((nat.tcp_refused, nat.tcp_accepted), (1, 0));
}
