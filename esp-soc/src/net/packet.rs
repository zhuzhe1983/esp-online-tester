//! Packet construction shared by the local subnet and its host NAT transport.

pub(crate) const GATEWAY_MAC: [u8; 6] = [0x02, 0x53, 0x49, 0x4d, 0x00, 0x02];

pub(crate) fn checksum(data: &[u8], init: u32) -> u16 {
    let mut sum = init;
    let (words, tail) = data.as_chunks::<2>();
    for word in words { sum += u16::from_be_bytes(*word) as u32; }
    if let [last] = tail { sum += (*last as u32) << 8; }
    while sum >> 16 != 0 { sum = (sum & 0xffff) + (sum >> 16); }
    !(sum as u16)
}

pub(crate) fn transport_checksum(src: &[u8; 4], dst: &[u8; 4], proto: u8, data: &[u8]) -> u16 {
    let mut sum = proto as u32 + data.len() as u32;
    for address in [src, dst] {
        sum += u16::from_be_bytes([address[0], address[1]]) as u32;
        sum += u16::from_be_bytes([address[2], address[3]]) as u32;
    }
    checksum(data, sum)
}

pub(crate) fn ethernet(dst: &[u8; 6], src: &[u8; 6], ethertype: u16, payload: &[u8]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(14 + payload.len());
    frame.extend_from_slice(dst);
    frame.extend_from_slice(src);
    frame.extend_from_slice(&ethertype.to_be_bytes());
    frame.extend_from_slice(payload);
    frame
}

/// Build an IPv4 packet without options.
pub(crate) fn ip_packet(proto: u8, src: &[u8; 4], dst: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let total = 20 + payload.len();
    let mut packet = Vec::with_capacity(total);
    packet.extend_from_slice(&[0x45, 0]);
    packet.extend_from_slice(&(total as u16).to_be_bytes());
    packet.extend_from_slice(&[0, 0, 0x40, 0, 64, proto, 0, 0]);
    packet.extend_from_slice(src);
    packet.extend_from_slice(dst);
    let check = checksum(&packet, 0);
    packet[10..12].copy_from_slice(&check.to_be_bytes());
    packet.extend_from_slice(payload);
    packet
}

pub(crate) fn udp_packet(src: &[u8; 4], dst: &[u8; 4], sport: u16, dport: u16, payload: &[u8]) -> Vec<u8> {
    let len = 8 + payload.len();
    let mut packet = Vec::with_capacity(len);
    packet.extend_from_slice(&sport.to_be_bytes());
    packet.extend_from_slice(&dport.to_be_bytes());
    packet.extend_from_slice(&(len as u16).to_be_bytes());
    packet.extend_from_slice(&[0, 0]);
    packet.extend_from_slice(payload);
    let check = transport_checksum(src, dst, 17, &packet);
    packet[6..8].copy_from_slice(&if check == 0 { 0xffff } else { check }.to_be_bytes());
    packet
}

/// The question of a DNS query: its name, its type and where the question ends
/// (the header and the question are `&query[..end]`). None for a response or a malformed query.
pub(crate) struct DnsQuestion { pub name: String, pub qtype: u16, pub end: usize }

pub(crate) fn dns_question(q: &[u8]) -> Option<DnsQuestion> {
    if q.len() < 12 || u16::from_be_bytes([q[2], q[3]]) & 0x8000 != 0 { return None; }
    let mut i = 12;
    let mut name = String::new();
    while i < q.len() && q[i] != 0 {
        let l = q[i] as usize;
        if l > 63 || i + 1 + l > q.len() { return None; }
        if !name.is_empty() { name.push('.'); }
        name.push_str(&String::from_utf8_lossy(&q[i + 1..i + 1 + l]));
        i += 1 + l;
    }
    if i + 5 > q.len() { return None; }
    Some(DnsQuestion { name, qtype: u16::from_be_bytes([q[i + 1], q[i + 2]]), end: i + 5 })
}

/// The response to `query` (its header and first question, `dns_question`'s
/// `end`): `rcode` 0 with an A record per address for an A question (none for any other type), or
/// the error `rcode` without records.
pub(crate) fn dns_reply(query: &[u8], qtype: u16, addrs: &[[u8; 4]], ttl: u32, rcode: u8) -> Vec<u8> {
    let addrs = if qtype == 1 && rcode == 0 { addrs } else { &[] };
    let mut r = Vec::with_capacity(query.len() + 16 * addrs.len());
    r.extend_from_slice(&query[0..2]);                               // transaction id
    r.extend_from_slice(&[0x81, 0x80 | (rcode & 0x0f), 0, 1]);       // response, recursion available
    r.extend_from_slice(&(addrs.len() as u16).to_be_bytes());        // answer count
    r.extend_from_slice(&[0, 0, 0, 0]);
    r.extend_from_slice(&query[12..]);
    for addr in addrs {
        r.extend_from_slice(&[0xc0, 0x0c, 0, 1, 0, 1]);              // the question's name, A, IN
        r.extend_from_slice(&ttl.to_be_bytes());
        r.extend_from_slice(&[0, 4]);
        r.extend_from_slice(addr);
    }
    r
}
