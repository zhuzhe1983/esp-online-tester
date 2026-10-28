//! Line-framed, bounded Ethernet NAT backend used by tools/network-helper.mjs.
use super::{packet::*, VirtualNet};
use crate::nat::{HostFwdSpec, Nat};
use std::io::{self, BufRead, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::mpsc::{sync_channel, TryRecvError};
use std::time::{Duration, Instant};

const CLIENT_MAC: [u8; 6] = [2, 0x53, 0x49, 0x4d, 0, 3];
const XID: [u8; 4] = [0x45, 0x53, 0x50, 1];
const MAX_MAPS: usize = 8;
const MAX_PEERS: usize = 32;
struct UdpPeer { host: SocketAddr, port: u16, touched: u64 }
struct UdpMap { id: u32, sock: UdpSocket, guest_port: u16, peers: Vec<UdpPeer> }
struct TcpMap { id: u32, host_port: u16 }
struct Backend {
    net: VirtualNet, client: bool, enabled: bool, leased: bool, offered: Option<([u8; 4], [u8; 4])>,
    address: [u8; 4], guest: [u8; 4], client_mac: [u8; 6], last_dhcp: Option<u64>,
    udp: Vec<UdpMap>, tcp: Vec<TcpMap>, next_port: u16, frames_in: u64, frames_out: u64,
}
fn ip_text(ip: [u8; 4]) -> String { Ipv4Addr::from(ip).to_string() }
fn hex(bytes: &[u8]) -> String { bytes.iter().map(|b| format!("{b:02x}")).collect() }
fn unhex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) || text.len() > 3036 || !text.bytes().all(|c| c.is_ascii_hexdigit()) { return None; }
    text.as_bytes().as_chunks::<2>().0.iter().map(|b| u8::from_str_radix(std::str::from_utf8(b).ok()?, 16).ok()).collect()
}
type UdpPacket<'a> = ([u8; 4], [u8; 4], u16, u16, &'a [u8]);
fn ipv4_udp(frame: &[u8]) -> Option<UdpPacket<'_>> {
    if frame.len() < 42 || frame[12..14] != [8, 0] { return None; }
    let p = &frame[14..]; let ihl = usize::from(p[0] & 15) * 4;
    let total = usize::from(u16::from_be_bytes([p[2], p[3]]));
    if p[0] >> 4 != 4 || p[9] != 17 || ihl < 20 || total > p.len() || total < ihl + 8 || p[6] & 0x3f != 0 || p[7] != 0 { return None; }
    let u = &p[ihl..total]; let len = usize::from(u16::from_be_bytes([u[4], u[5]]));
    if !(8..=u.len()).contains(&len) { return None; }
    Some((p[12..16].try_into().ok()?, p[16..20].try_into().ok()?, u16::from_be_bytes([u[0], u[1]]), u16::from_be_bytes([u[2], u[3]]), &u[8..len]))
}
fn option(payload: &[u8], code: u8) -> Option<&[u8]> {
    if payload.len() < 240 || payload[236..240] != [99, 130, 83, 99] { return None; }
    let mut i = 240;
    while i < payload.len() {
        let key = payload[i]; i += 1;
        if key == 255 { break; }
        if key == 0 { continue; }
        let len = usize::from(*payload.get(i)?); i += 1;
        let value = payload.get(i..i + len)?; i += len;
        if key == code { return Some(value); }
    }
    None
}
impl Backend {
    fn new(client: bool) -> Self {
        let mut net = VirtualNet::new(false); net.nat = Some(Nat::new(false));
        Self { net, client, enabled: !client, leased: !client, offered: None,
            address: if client { [0; 4] } else { [10, 0, 2, 2] }, guest: if client { [0; 4] } else { [10, 0, 2, 15] },
            client_mac: CLIENT_MAC, last_dhcp: None, udp: Vec::new(), tcp: Vec::new(), next_port: 55000, frames_in: 0, frames_out: 0 }
    }
    fn status(&self) -> String {
        let nat = self.net.nat.as_ref().unwrap();
        format!("{{\"enabled\":{},\"ready\":{},\"address\":\"{}\",\"guestAddress\":\"{}\",\"addressSource\":\"{}\",\"counters\":{{\"framesIn\":{},\"framesOut\":{},\"tcpOpened\":{},\"tcpRefused\":{},\"udpFlows\":{},\"bytesOut\":{},\"bytesIn\":{}}}}}",
            self.enabled, self.leased && self.net.sta_mac != [0; 6], ip_text(self.address), ip_text(self.guest), if self.client {"dhcp"} else {"virtual-router"}, self.frames_in, self.frames_out, nat.tcp_opened, nat.tcp_refused, nat.udp_flows, nat.bytes_to_host, nat.bytes_to_guest)
    }
    fn dhcp(&self) -> Vec<u8> {
        let mut p = vec![0u8; 240]; p[..4].copy_from_slice(&[1, 1, 6, 0]); p[4..8].copy_from_slice(&XID);
        p[10..12].copy_from_slice(&[0x80, 0]); p[28..34].copy_from_slice(&self.client_mac); p[236..240].copy_from_slice(&[99,130,83,99]);
        p.extend_from_slice(&[53, 1, if self.offered.is_some() {3} else {1}, 61, 7, 1]); p.extend_from_slice(&self.client_mac);
        if let Some((address, server)) = self.offered { p.extend_from_slice(&[50,4]); p.extend_from_slice(&address); p.extend_from_slice(&[54,4]); p.extend_from_slice(&server); }
        p.extend_from_slice(&[55,3,1,3,6,255]); p.resize(300,0);
        ethernet(&[255;6], &self.client_mac, 0x0800, &ip_packet(17,&[0;4],&[255;4],&udp_packet(&[0;4],&[255;4],68,67,&p)))
    }
    fn handle(&mut self, frame: &[u8], now: u64) -> Vec<Vec<u8>> {
        if !self.enabled || !(14..=1518).contains(&frame.len()) { return Vec::new(); }
        self.frames_in += 1;
        if self.client {
            if let Some((source, _, 67, 68, p)) = ipv4_udp(frame) {
                if p.len() < 240 || p[0] != 2 || p[4..8] != XID || p[28..34] != self.client_mac { return Vec::new(); }
                let address: [u8;4] = p[16..20].try_into().unwrap();
                let server: [u8;4] = option(p,54).and_then(|x| x.try_into().ok()).unwrap_or(source);
                match option(p,53) {
                    Some([2]) => { self.offered = Some((address,server)); self.last_dhcp = Some(now); return vec![self.dhcp()]; }
                    Some([5]) if address != [0;4] && self.offered == Some((address,server)) => {
                        self.leased = true; self.address = address; self.guest = server;
                        self.net.gw_ip = address; self.net.sta_ip = server; self.net.gw_mac = self.client_mac;
                        self.net.sta_mac.copy_from_slice(&frame[6..12]);
                        self.net.nat.as_mut().unwrap().gateway_ip = address;
                    }
                    Some([6]) => { self.leased = false; self.offered = None; self.last_dhcp = None; }
                    _ => {}
                }
                return Vec::new();
            }
            if !self.leased { return Vec::new(); }
        }
        if let Some((source, destination, sport, dport, payload)) = ipv4_udp(frame) {
            if source == self.guest && destination == self.address {
                for map in &mut self.udp {
                    if sport != map.guest_port { continue; }
                    if let Some(peer) = map.peers.iter_mut().find(|p| p.port == dport) {
                        let _ = map.sock.send_to(payload, peer.host); peer.touched = now; return Vec::new();
                    }
                }
            }
        }
        self.net.handle(frame, now)
    }
    fn poll(&mut self, now: u64) -> Vec<Vec<u8>> {
        if !self.enabled { return Vec::new(); }
        if self.client && !self.leased {
            if self.last_dhcp.is_none_or(|last| now.saturating_sub(last) >= 2_000_000) { self.last_dhcp = Some(now); return vec![self.dhcp()]; }
            return Vec::new();
        }
        let mut out = self.net.poll(now);
        if self.net.sta_mac == [0;6] { return out; }
        let mut payload = [0u8;1473];
        let mut used: Vec<u16> = self.udp.iter().flat_map(|m| m.peers.iter().map(|p|p.port)).collect();
        for map in &mut self.udp {
            map.peers.retain(|p| now.saturating_sub(p.touched) < 120_000_000);
            for _ in 0..16 {
                let (n, host) = match map.sock.recv_from(&mut payload) { Ok(x) => x, Err(_) => break };
                if n > 1472 { continue; }
                let port = if let Some(peer) = map.peers.iter_mut().find(|p|p.host == host) { peer.touched = now; peer.port }
                    else {
                        if map.peers.len() >= MAX_PEERS { continue; }
                        while used.contains(&self.next_port) { self.next_port = if self.next_port == 65000 {55000} else {self.next_port+1}; }
                        let port = self.next_port; used.push(port); self.next_port += 1;
                        map.peers.push(UdpPeer {host,port,touched:now}); port
                    };
                out.push(ethernet(&self.net.sta_mac, &self.net.gw_mac, 0x0800, &ip_packet(17,&self.address,&self.guest,&udp_packet(&self.address,&self.guest,port,map.guest_port,&payload[..n]))));
            }
        }
        out
    }
    fn map(&mut self, id: u32, protocol: &str, host_port: u16, guest_port: u16) -> io::Result<u16> {
        if !self.enabled || !self.leased || self.net.sta_mac == [0;6] { return Err(io::Error::other("interface_not_ready")); }
        if guest_port == 0 || self.tcp.len()+self.udp.len() >= MAX_MAPS || self.tcp.iter().any(|m|m.id==id) || self.udp.iter().any(|m|m.id==id) { return Err(io::Error::other("invalid_mapping")); }
        if protocol == "tcp" {
            let bound = self.net.nat.as_mut().unwrap().listen(&HostFwdSpec {bind:([127,0,0,1],host_port).into(),guest_ip:self.guest,guest_port})?;
            self.tcp.push(TcpMap {id,host_port:bound.port()}); Ok(bound.port())
        } else if protocol == "udp" {
            let sock = UdpSocket::bind((Ipv4Addr::LOCALHOST,host_port))?; sock.set_nonblocking(true)?; let port = sock.local_addr()?.port();
            self.udp.push(UdpMap {id,sock,guest_port,peers:Vec::new()}); Ok(port)
        } else { Err(io::Error::other("invalid_protocol")) }
    }
    fn unmap(&mut self, id: u32) {
        for map in self.tcp.iter().filter(|m|m.id==id) { self.net.nat.as_mut().unwrap().unlisten(map.host_port); }
        self.tcp.retain(|m|m.id!=id); self.udp.retain(|m|m.id!=id);
    }
}
/// Private stdio protocol. The Node server owns authentication, limits and WebSocket framing.
/// Each backend process is one network interface; closing its stdin releases every mapping/flow.
pub fn run() -> io::Result<()> {
    let (client,resolver) = options(std::env::args().skip(1))?; let mut backend = Backend::new(client);
    if let Some(resolver) = resolver { backend.net.nat.as_mut().unwrap().resolver = resolver; }
    let (tx,rx) = sync_channel(64);
    std::thread::spawn(move || {
        let stdin=io::stdin(); let mut reader=stdin.lock();
        loop { let mut line=String::new(); let Ok(n)=reader.by_ref().take(4097).read_line(&mut line) else {break};
            if n==0 || n>4096 || !line.ends_with('\n') {break;}
            if tx.send(line).is_err(){break;}
        }
    });
    let start=Instant::now(); let mut output=io::BufWriter::new(io::stdout().lock()); let mut last_status=0;
    writeln!(output,"status {}",backend.status())?; output.flush()?;
    loop {
        let now=start.elapsed().as_micros().min(u128::from(u64::MAX)) as u64; let mut frames=Vec::new();
        for _ in 0..64 {
            let line=match rx.try_recv(){Ok(line)=>line,Err(TryRecvError::Empty)=>break,Err(TryRecvError::Disconnected)=>return Ok(())};
            let words:Vec<&str>=line.split_whitespace().collect();
            match words.as_slice() {
                ["frame",bytes]=>{if let Some(frame)=unhex(bytes){frames.extend(backend.handle(&frame,now));}},
                ["enable"]=>{backend.enabled=true;backend.last_dhcp=None;},
                ["map",request,protocol,host,guest]=>{
                    if let (Ok(id),Ok(host),Ok(guest))=(request.parse::<u32>(),host.parse::<u16>(),guest.parse::<u16>()) {
                        match backend.map(id,protocol,host,guest) {Ok(port)=>writeln!(output,"reply {id} ok {port}")?,Err(e)=>writeln!(output,"reply {id} error {:?}",e.kind())?}
                    }
                },
                ["unmap",request]=>{if let Ok(id)=request.parse::<u32>(){backend.unmap(id);writeln!(output,"reply {id} ok 0")?;}},
                _=>{}
            }
        }
        frames.extend(backend.poll(now));
        for mut frame in frames {if backend.client && frame.len()>=14{frame[6..12].copy_from_slice(&backend.client_mac);} backend.frames_out+=1;writeln!(output,"frame {}",hex(&frame))?;}
        if now.saturating_sub(last_status)>500_000 {last_status=now;writeln!(output,"status {}",backend.status())?;}
        output.flush()?;std::thread::sleep(Duration::from_millis(2));
    }
}

fn options(mut args: impl Iterator<Item=String>) -> io::Result<(bool, Option<[u8;4]>)> {
    let mut client=false; let mut resolver=None;
    while let Some(arg)=args.next() {
        match arg.as_str() {
            "--client"=>client=true,
            "--resolver"=>resolver=Some(args.next().and_then(|v|v.parse::<Ipv4Addr>().ok()).ok_or_else(||io::Error::new(io::ErrorKind::InvalidInput,"--resolver requires an IPv4 address"))?.octets()),
            _=>return Err(io::Error::new(io::ErrorKind::InvalidInput,"unknown network backend argument")),
        }
    }
    Ok((client,resolver))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_resolver_does_not_use_host_dns() {
        let parse=|args:&[&str]|options(args.iter().map(|s|s.to_string()));
        assert_eq!(parse(&[]).unwrap(),(false,None));
        assert_eq!(parse(&["--resolver","1.1.1.1","--client"]).unwrap(),(true,Some([1;4])));
        assert!(parse(&["--resolver"]).is_err()); assert!(parse(&["--resolver","example.com"]).is_err()); assert!(parse(&["--unknown"]).is_err());
    }
    fn dhcp_reply(kind:u8,address:[u8;4],xid:[u8;4])->Vec<u8>{
        let mut p=vec![0;240];p[..4].copy_from_slice(&[2,1,6,0]);p[4..8].copy_from_slice(&xid);p[16..20].copy_from_slice(&address);p[28..34].copy_from_slice(&CLIENT_MAC);p[236..240].copy_from_slice(&[99,130,83,99]);p.extend_from_slice(&[53,1,kind,54,4,192,168,4,1,255]);
        ethernet(&CLIENT_MAC,&[2,3,4,5,6,7],0x800,&ip_packet(17,&[192,168,4,1],&[255;4],&udp_packet(&[192,168,4,1],&[255;4],67,68,&p)))
    }
    #[test]
    fn softap_client_requires_matching_dhcp_exchange() {
        let mut b=Backend::new(true);assert!(b.poll(0).is_empty());b.enabled=true;
        let discover=b.poll(1);let (_,_,sport,dport,p)=ipv4_udp(&discover[0]).unwrap();assert_eq!((sport,dport),(68,67));assert_eq!(option(p,53),Some([1].as_slice()));
        b.handle(&dhcp_reply(5,[192,168,4,2],XID),2);assert!(!b.leased);
        b.handle(&dhcp_reply(2,[192,168,4,2],[0;4]),2);assert!(b.offered.is_none());
        let request=b.handle(&dhcp_reply(2,[192,168,4,2],XID),3);assert_eq!(option(ipv4_udp(&request[0]).unwrap().4,53),Some([3].as_slice()));
        b.handle(&dhcp_reply(5,[192,168,4,2],XID),4);assert!(b.leased);assert_eq!(b.address,[192,168,4,2]);assert_eq!(b.guest,[192,168,4,1]);assert_eq!(b.net.nat.as_ref().unwrap().gateway_ip,b.address);
    }
    #[test]
    fn udp_mapping_roundtrip_and_cleanup() {
        let mut b=Backend::new(false);b.net.sta_mac=[2;6];let port=b.map(1,"udp",0,5000).unwrap();let host=UdpSocket::bind((Ipv4Addr::LOCALHOST,0)).unwrap();host.set_read_timeout(Some(Duration::from_secs(1))).unwrap();host.send_to(b"request",(Ipv4Addr::LOCALHOST,port)).unwrap();
        let deadline=Instant::now()+Duration::from_secs(1);let frames=loop{let out=b.poll(1);if !out.is_empty(){break out;}assert!(Instant::now()<deadline);std::thread::yield_now();};
        let (source,dest,sport,dport,p)=ipv4_udp(&frames[0]).unwrap();assert_eq!(p,b"request");assert_eq!(dport,5000);
        let reply=ethernet(&GATEWAY_MAC,&[2;6],0x800,&ip_packet(17,&dest,&source,&udp_packet(&dest,&source,dport,sport,b"response")));assert!(b.handle(&reply,2).is_empty());let mut data=[0;32];assert_eq!(host.recv(&mut data).unwrap(),8);assert_eq!(&data[..8],b"response");b.unmap(1);assert!(UdpSocket::bind((Ipv4Addr::LOCALHOST,port)).is_ok());
    }
    #[test]
    fn malformed_frames_and_hex_are_bounded() {
        assert!(unhex("gg").is_none());assert!(unhex("f").is_none());assert!(unhex(&"00".repeat(1519)).is_none());
        let mut b=Backend::new(true);b.enabled=true;for n in 0..240{assert!(b.handle(&vec![0;n],0).is_empty());}assert!(option(&vec![0;240],1).is_none());
    }
}
