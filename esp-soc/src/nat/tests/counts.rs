use super::*;

fn counts(nat: &Nat, active: usize, time_wait: usize) {
    // An immutable borrow is part of the snapshot contract.
    assert_eq!(nat.tcp_flow_counts(), TcpFlowCounts { active, time_wait });
    assert_eq!(nat.tcp_flow_counts(), TcpFlowCounts { active, time_wait });
}

#[test]
fn empty_and_failed_admission_have_no_tcp_occupancy() {
    let mut nat = Nat::new(false);
    counts(&nat, 0, 0);
    nat.tcp_in_with_connect(&[2; 6], &[10, 0, 2, 15], &[127, 0, 0, 1],
        &segment(199, 0, SYN, &[]), 0, |_| None);
    counts(&nat, 0, 0);
    assert!(nat.tcp.is_empty());
    assert_eq!(nat.tcp_opened, 0);
}

#[test]
fn connecting_snapshot_does_not_consume_ready_or_refused_results() {
    for refused in [false, true] {
        let mut nat = Nat::new(false);
        let (tx, rx) = channel();
        nat.tcp_in_with_connect(&[2; 6], &[10, 0, 2, 15], &[127, 0, 0, 1],
            &segment(199, 0, SYN, &[]), 7, |_| Some(rx));
        counts(&nat, 1, 0);
        let (socket, host) = socket_pair();
        if refused { tx.send(Err(ErrorKind::ConnectionRefused.into())).unwrap(); }
        else { tx.send(Ok(socket)).unwrap(); }
        counts(&nat, 1, 0);
        assert!(matches!(nat.tcp[0].transport, Transport::Connecting(_)));
        assert_eq!(nat.tcp[0].last_activity_us, 7);
        assert!(nat.tcp[0].unacked.is_empty());
        assert_eq!((nat.tcp_opened, nat.tcp_refused, nat.bytes_to_guest), (0, 0, 0));
        let frames = nat.poll(8);
        assert_eq!(frames.len(), 1);
        if refused {
            assert_eq!(frames[0][47], RST | ACK);
            counts(&nat, 0, 0);
            assert_eq!(nat.tcp_refused, 1);
        } else {
            assert_eq!(frames[0][47], SYN | ACK);
            counts(&nat, 1, 0);
            assert_eq!(nat.tcp_opened, 1);
        }
        drop(host);
    }
}

#[test]
fn guest_reset_is_excluded_before_poll_without_pruning() {
    let mut nat = Nat::new(false);
    let (socket, _host) = socket_pair();
    let mut c = flow(); c.transport = Transport::Connected(socket);
    nat.tcp.push(c);
    counts(&nat, 1, 0);
    nat.tcp_in(&[2; 6], &[10, 0, 2, 15], &[127, 0, 0, 1], &segment(200, 100, RST, &[]), 1);
    counts(&nat, 0, 0);
    assert_eq!(nat.tcp.len(), 1);
    assert_eq!(nat.tcp[0].last_activity_us, 1);
    nat.poll(2);
    assert!(nat.tcp.is_empty());
}

#[test]
fn disconnected_connector_is_active_until_poll_reports_host_abort() {
    let mut nat = Nat::new(false);
    let (tx, rx) = channel(); drop(tx);
    let mut c = flow(); c.transport = Transport::Connecting(rx);
    nat.tcp.push(c);
    counts(&nat, 1, 0);
    let frames = nat.poll(1);
    assert_eq!(frames[0][47], RST | ACK);
    counts(&nat, 0, 0);
    assert_eq!(nat.tcp_refused, 1);
}

#[test]
fn mixed_snapshot_counts_states_not_cumulative_statistics() {
    let mut nat = Nat::new(false);
    let (_tx, rx) = channel();
    let (socket, _host) = socket_pair();
    for transport in [Transport::Closed, Transport::TimeWait, Transport::Connected(socket), Transport::Connecting(rx)] {
        let mut c = flow(); c.transport = transport; nat.tcp.push(c);
    }
    nat.tcp_opened = 100; nat.tcp_refused = 200;
    counts(&nat, 2, 1);
    assert_eq!(nat.tcp.len(), 4);
    assert_eq!((nat.tcp_opened, nat.tcp_refused), (100, 200));
}

#[test]
fn host_io_failure_closes_flow_only_when_polled() {
    let mut nat = Nat::new(false);
    let (socket, _host) = socket_pair();
    // Deterministically exercise host write-error/RST without a peer-delivery race.
    socket.shutdown(std::net::Shutdown::Both).unwrap();
    let mut c = flow(); c.transport = Transport::Connected(socket);
    c.to_host.extend(b"pending"); nat.tcp.push(c);
    counts(&nat, 1, 0);
    assert_eq!(nat.tcp[0].to_host.len(), 7);
    assert_eq!(nat.bytes_to_host, 0);
    let frames = nat.poll(1);
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0][47], RST | ACK);
    counts(&nat, 0, 0);
}

#[test]
fn half_close_unacked_fin_and_final_ack_retry_remain_occupied() {
    let (socket, mut host) = socket_pair();
    let mut nat = Nat::new(false);
    let mut c = flow(); c.transport = Transport::Connected(socket);
    nat.tcp.push(c);
    nat.tcp_in(&[2; 6], &[10, 0, 2, 15], &[127, 0, 0, 1], &segment(200, 100, ACK | FIN, b"hello"), 0);
    counts(&nat, 1, 0); // guest write is still draining
    nat.poll(1);
    let mut received = Vec::new(); host.read_to_end(&mut received).unwrap();
    assert_eq!(received, b"hello");
    counts(&nat, 1, 0); // guest half closed, host can still reply
    host.shutdown(std::net::Shutdown::Write).unwrap();
    let sent = poll_ready(&mut nat, 2);
    counts(&nat, 1, 0); // both closed, but host FIN unacknowledged
    assert_eq!(nat.poll(RETRANSMIT_US + 2), sent);
    nat.tcp_in(&[2; 6], &[10, 0, 2, 15], &[127, 0, 0, 1], &segment(206, 101, ACK, &[]), RETRANSMIT_US + 3);
    counts(&nat, 1, 0); // only poll enters TimeWait
    nat.poll(RETRANSMIT_US + 4);
    counts(&nat, 0, 1);
    let at = nat.tcp[0].last_activity_us;
    let reply = nat.tcp_in(&[2; 6], &[10, 0, 2, 15], &[127, 0, 0, 1], &segment(205, 101, ACK | FIN, &[]), at);
    assert_eq!(reply.len(), 1);
    counts(&nat, 0, 1);
    assert_eq!(nat.tcp[0].last_activity_us, at);
    nat.poll(at + TIME_WAIT_US - 1);
    counts(&nat, 0, 1);
    nat.poll(at + TIME_WAIT_US);
    counts(&nat, 0, 0);
}

#[test]
fn idle_timeout_requires_poll_and_counts_do_not_refresh_activity() {
    let mut nat = Nat::new(false);
    let (_tx, rx) = channel();
    let mut c = flow(); c.transport = Transport::Connecting(rx); nat.tcp.push(c);
    nat.poll(IDLE_CLOSE_US - 1);
    counts(&nat, 1, 0);
    assert_eq!(nat.tcp[0].last_activity_us, 0);
    nat.poll(IDLE_CLOSE_US);
    counts(&nat, 0, 0);
}
