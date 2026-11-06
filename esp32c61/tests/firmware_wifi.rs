//! Caller-supplied official image/ROM; no firmware or host address is embedded.
use esp_soc::SocBus;
use std::io::{Read, Write};
use std::net::{Ipv4Addr, TcpListener};
use std::time::{Duration, Instant};

fn python(m: &mut esp32c61::Machine, script: &str, marker: &str) {
    m.console.all.clear(); m.bus.serial_input(&[1]); m.run(2_000_000);
    for chunk in script.as_bytes().chunks(64) { m.bus.serial_input(chunk); m.run(1_000_000); }
    m.bus.serial_input(&[4]);
    for _ in 0..200 {
        m.run(10_000_000);
        if String::from_utf8_lossy(&m.console.all).contains(marker) { break; }
    }
    let output = String::from_utf8_lossy(&m.console.all);
    assert!(output.contains(marker), "missing {marker}; pc={:08x}; {}\n{output}", m.cores[0].pc, m.bus.report());
    assert!(!output.contains("Traceback"), "guest failure: {output}");
    println!("{output}"); m.run(1_000_000); m.bus.serial_input(&[2]); m.run(1_000_000);
}

#[test]
#[ignore = "needs C61_ROM_ELF, COREMATRIX_FIRMWARE and C61_TEST_HTTP_BIND (host IPv4 reachable by guest NAT)"]
fn external_official_uiflow_wifi_dns_ntp_http() { let _ = official_wifi(None); }
#[test]
#[ignore = "needs C61_ROM_ELF, COREMATRIX_FIRMWARE and C61_TEST_HTTP_BIND (host IPv4 reachable by guest NAT)"]
fn external_official_uiflow_wpa2_wifi_dns_ntp_http() { let _ = official_wifi(Some("c61-test-password")); }

#[test]
#[ignore = "needs C61_ROM_ELF, COREMATRIX_FIRMWARE and C61_TEST_HTTP_BIND (host IPv4 reachable by guest NAT)"]
fn external_official_uiflow_rejects_wrong_wpa2_password() {
    let mut m = official_wifi(Some("c61-test-password"));
    let previous_replay = m.bus.periph.wifi_mac.link.ap().unwrap().wpa.replay;
    python(&mut m, "w.disconnect()\nw.active(False)\nw.active(True)\nw.connect('c61-test','wrong-password')\ntime.sleep_ms(2500)\nassert not w.isconnected(),w.ifconfig()\nprint('C61_WRONG_PASSWORD_REJECTED',w.status())\n", "C61_WRONG_PASSWORD_REJECTED");
    assert!(m.bus.periph.wifi_mac.link.ap().unwrap().wpa.replay > previous_replay, "wrong-password attempt did not reach the four-way handshake");
    assert_ne!(m.bus.periph.wifi_mac.link.ap().unwrap().wpa.state, esp_soc::wifi::WpaState::Installed);
}

fn official_wifi(password: Option<&str>) -> esp32c61::Machine {
    let input = |name| std::fs::read(std::env::var(name).unwrap_or_else(|_| panic!("set {name} to the caller-provided input"))).unwrap();
    let host: Ipv4Addr = std::env::var("C61_TEST_HTTP_BIND").expect("set C61_TEST_HTTP_BIND to a host IPv4 address reachable by guest NAT").parse().unwrap();
    assert!(!host.is_loopback() && !host.is_unspecified(), "guest loopback stays inside lwIP; supply the host interface IPv4");
    let mut m = esp32c61::machine([2, 0, 0, 0, 0, 0x61], 16 << 20);
    m.console.capture = true; m.bus.set_flash_size(16 << 20); m.bus.set_psram_size(8 << 20).unwrap(); m.bus.periph.usb.connected = true;
    m.bus.board = esp32c61::board::make_board("corematrix").unwrap(); m.bus.attach_board_devices();
    let mut ap = esp_soc::wifi::ApConfig::parse("ssid=c61-test,epoch=1700000000").unwrap(); ap.psk = password.map(str::to_string);
    m.bus.attach_wifi(ap, None).unwrap();
    m.load_rom(&input("C61_ROM_ELF")).unwrap(); m.write_flash(0, &input("COREMATRIX_FIRMWARE")).unwrap();
    m.boot_rom(); m.dbg.stop_after_exceptions = 1;
    for _ in 0..30 { m.run(10_000_000); if String::from_utf8_lossy(&m.console.all).contains("MicroPython v1.27.0-dirty") { break; } }
    assert!(String::from_utf8_lossy(&m.console.all).contains("MicroPython v1.27.0-dirty"), "official image did not reach MicroPython: {}", String::from_utf8_lossy(&m.console.all));
    let local_script = r#"import network, time, socket
w=network.WLAN(network.STA_IF)
w.active(True)
scan=w.scan()
assert any(a[0]==b'c61-test' and a[2]==6 for a in scan), scan
print('C61_SCAN',scan)
w.connect('c61-test')
for i in range(50):
 if w.isconnected(): break
 time.sleep_ms(100)
assert w.isconnected(), w.status()
assert w.ifconfig()[0]=='10.0.2.15', w.ifconfig()
print('C61_DHCP',w.ifconfig())
dns=socket.getaddrinfo('c61.test',80)
assert dns[0][-1][0]=='10.0.2.3', dns
print('C61_DNS',dns)
u=socket.socket(socket.AF_INET,socket.SOCK_DGRAM)
u.settimeout(2)
u.sendto(b'\x23'+bytes(47),('10.0.2.3',123))
ntp=u.recv(128)
u.close()
assert len(ntp)==48 and ntp[0]&7==4, ntp
assert int.from_bytes(ntp[40:44],'big')>=3908988800, ntp
print('C61_NTP',len(ntp))
print('C61_NETWORK_LOCAL_PASS')
"#;
    let local_script = local_script.replace("w.connect('c61-test')", &format!("w.connect('c61-test', '{}')", password.unwrap_or("")));
    python(&mut m, &local_script, "C61_NETWORK_LOCAL_PASS");
    if password.is_some() { assert_eq!(m.bus.periph.wifi_mac.link.ap().unwrap().wpa.state, esp_soc::wifi::WpaState::Installed); }
    let net = m.bus.periph.wifi_mac.link.net().unwrap();
    assert_eq!(net.dhcp_acks, 1); assert_eq!(net.dns_answers, 1); assert_eq!(net.ntp_answers, 1);

    // A real local TCP socket verifies both directions, segmentation and binary
    // payload integrity. No remote service, DNS resolver or Internet is required.
    let server = TcpListener::bind((host, 0)).unwrap(); let port = server.local_addr().unwrap().port(); server.set_nonblocking(true).unwrap();
    let request_body: Vec<u8> = (0..4096).map(|i| i as u8).collect();
    let response_body: Vec<u8> = (0..8192).map(|i| 255 - i as u8).collect();
    let worker = std::thread::spawn(move || {
        let until = Instant::now() + Duration::from_secs(30);
        let mut peer = loop {
            match server.accept() {
                Ok((peer, _)) => break peer,
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock && Instant::now() < until => std::thread::sleep(Duration::from_millis(1)),
                Err(err) => panic!("HTTP accept failed: {err}"),
            }
        };
        peer.set_nonblocking(false).unwrap(); peer.set_read_timeout(Some(Duration::from_secs(10))).unwrap(); peer.set_write_timeout(Some(Duration::from_secs(10))).unwrap();
        let mut request = Vec::new(); let mut bytes = [0; 2048];
        loop {
            let n = peer.read(&mut bytes).unwrap(); assert!(n > 0, "premature HTTP EOF"); request.extend_from_slice(&bytes[..n]);
            if let Some(end) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                if request.len() >= end + 4 + request_body.len() {
                    assert!(request.starts_with(b"POST /c61-wifi HTTP/1.0\r\n")); assert_eq!(&request[end + 4..], request_body); break;
                }
            }
            assert!(request.len() < 8192, "HTTP header/payload exceeded fixture bounds");
        }
        peer.write_all(format!("HTTP/1.0 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", response_body.len()).as_bytes()).unwrap();
        peer.write_all(&response_body).unwrap();
    });
    m.bus.periph.wifi_mac.link.net_mut().unwrap().nat = Some(esp_soc::nat::Nat::new(false));
    let script = format!(r#"s=socket.socket()
s.settimeout(5)
s.connect(('{host}',{port}))
body=bytes(range(256))*16
s.sendall(b'POST /c61-wifi HTTP/1.0\r\nContent-Length: 4096\r\n\r\n'+body)
response=b''
while True:
 chunk=s.recv(1024)
 if not chunk: break
 response+=chunk
s.close()
header,body=response.split(b'\r\n\r\n',1)
assert header.startswith(b'HTTP/1.0 200 OK'), header
assert body==bytes(range(255,-1,-1))*32, len(body)
print('C61_HTTP',4096,len(body))
print('C61_NETWORK_HTTP_PASS')
"#);
    python(&mut m, &script, "C61_NETWORK_HTTP_PASS"); worker.join().unwrap();
    let nat = m.bus.periph.wifi_mac.link.net().unwrap().nat.as_ref().unwrap();
    assert_eq!(nat.tcp_opened, 1); assert!(nat.bytes_to_host >= 4096 && nat.bytes_to_guest >= 8192);
    println!("{}", m.bus.report());
    let reconnect = format!(r#"w.disconnect()
w.active(False)
assert not w.active() and not w.isconnected()
time.sleep_ms(200)
w.active(True)
w.connect('c61-test','{}')
for i in range(50):
 if w.isconnected(): break
 time.sleep_ms(100)
assert w.isconnected(),w.status()
assert w.ifconfig()[0]=='10.0.2.15',w.ifconfig()
print('C61_RECONNECT',w.ifconfig())
print('C61_NETWORK_RECONNECT_PASS')
"#, password.unwrap_or(""));
    python(&mut m, &reconnect, "C61_NETWORK_RECONNECT_PASS");
    if password.is_some() { assert_eq!(m.bus.periph.wifi_mac.link.ap().unwrap().wpa.state, esp_soc::wifi::WpaState::Installed); }
    m
}

#[test]
#[ignore = "needs the firmware WiFi inputs plus C61_TEST_TLS_ADDRESS, C61_TEST_TLS_CA_DER and C61_TEST_TLS_EPOCH; server name corematrix.test"]
fn external_official_uiflow_wpa2_tls() {
    let address: std::net::SocketAddrV4 = std::env::var("C61_TEST_TLS_ADDRESS").expect("set C61_TEST_TLS_ADDRESS to caller-owned TLS fixture IPv4:port").parse().unwrap();
    let cert = std::fs::read(std::env::var("C61_TEST_TLS_CA_DER").expect("set C61_TEST_TLS_CA_DER to the fixture's trusted certificate DER")).unwrap();
    let epoch: u64 = std::env::var("C61_TEST_TLS_EPOCH").expect("set C61_TEST_TLS_EPOCH to Unix seconds within the fixture certificate's validity").parse().unwrap();
    let cert_hex: String = cert.iter().map(|b| format!("{b:02x}")).collect();
    let mut m = official_wifi(Some("c61-test-password"));
    // UIFlow2 issue #120, confirmed by its maintainer for a 2.5.5 fix: the
    // image's TLS time callback adds the 1970-to-2000 epoch offset twice.
    // Reproduce the original failure, then adjust only this guest test's RTC;
    // the emulator clock and certificate/hostname verification stay unchanged.
    let script = format!(r#"import ssl, machine
t=time.gmtime({epoch})
machine.RTC().datetime((t[0],t[1],t[2],t[6],t[3],t[4],t[5],0))
ctx=ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
ctx.verify_mode=ssl.CERT_REQUIRED
ctx.load_verify_locations(cadata=bytes.fromhex('{cert_hex}'))
s=socket.socket()
s.settimeout(10)
s.connect(('{host}',{port}))
try:
 ctx.wrap_socket(s,server_hostname='corematrix.test')
 raise AssertionError('expected pinned UIFlow2 TLS epoch bug')
except ValueError as e:
 assert 'certificate validity has expired' in str(e),e
 print('C61_TLS_ORIGINAL_EPOCH_BUG')
s.close()
t=time.gmtime({epoch}-946684800)
machine.RTC().datetime((t[0],t[1],t[2],t[6],t[3],t[4],t[5],0))
ctx=ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
ctx.verify_mode=ssl.CERT_REQUIRED
ctx.load_verify_locations(cadata=bytes.fromhex('{cert_hex}'))
s=socket.socket()
s.settimeout(10)
s.connect(('{host}',{port}))
tls=ctx.wrap_socket(s,server_hostname='corematrix.test')
print('C61_TLS_HANDSHAKE')
tls.write(b'GET /c61-wifi HTTP/1.0\r\nHost: corematrix.test\r\n\r\n')
response=b''
while b'\r\n\r\n' not in response:
 chunk=tls.read(128)
 assert chunk
 response+=chunk
header,body=response.split(b'\r\n\r\n',1)
assert header.startswith(b'HTTP/1.0 200 OK'),header
assert b'Content-Length: 4096' in header,header
while len(body)<4096:
 chunk=tls.read(min(1024,4096-len(body)))
 assert chunk
 body+=chunk
tls.close()
assert body==bytes(range(256))*16,len(body)
print('C61_TLS_HTTP',len(body))
s=socket.socket()
s.settimeout(10)
s.connect(('{host}',{port}))
try:
 ctx.wrap_socket(s,server_hostname='wrong.invalid')
 raise AssertionError('TLS accepted the wrong hostname')
except ValueError as e:
 assert 'CN' in str(e) or 'name' in str(e).lower(),e
 print('C61_TLS_WRONG_HOST_REJECTED')
s.close()
t=time.gmtime({epoch})
machine.RTC().datetime((t[0],t[1],t[2],t[6],t[3],t[4],t[5],0))
print('C61_NETWORK_TLS_PASS')
"#, host = address.ip(), port = address.port());
    python(&mut m, &script, "C61_NETWORK_TLS_PASS");
    println!("{}", m.bus.report());
}

#[test]
#[ignore = "needs C61_ROM_ELF, COREMATRIX_FIRMWARE and C61_TEST_HTTP_BIND (host IPv4 reachable by guest NAT)"]
fn external_official_uiflow_softap_arp_udp() {
    official_softap(None);
}
#[test]
#[ignore = "needs C61_ROM_ELF, COREMATRIX_FIRMWARE and C61_TEST_HTTP_BIND (host IPv4 reachable by guest NAT)"]
fn external_official_uiflow_wpa2_softap_arp_udp() {
    official_softap(Some("c61-softap-password"));
}
fn official_softap(password: Option<&str>) {
    let mut m = official_wifi(None);
    m.bus.configure_softap_client(Some(esp_soc::wifi::softap::ClientConfig { ssid: Some("c61-softap".into()), password: password.map(str::to_string) })).unwrap();
    let config = format!("ap=network.WLAN(network.AP_IF)\nap.active(True)\nap.config(ssid='c61-softap',security={},key='{}')\nprint('C61_SOFTAP',ap.ifconfig())\nu=socket.socket(socket.AF_INET,socket.SOCK_DGRAM)\nu.settimeout(2)\nu.bind(('192.168.4.1',18080))\nprint('C61_SOFTAP_READY')\n", if password.is_some() { 3 } else { 0 }, password.unwrap_or(""));
    python(&mut m, &config, "C61_SOFTAP_READY");
    let mut sent = false; let mut reply = false;
    for _ in 0..300 {
        m.bus.poll_softap_client(); m.run(5_000_000);
        if m.bus.softap_client_status().contains("\"state\":\"connected\"") && !sent {
            let mac = esp_soc::wifi::softap::CLIENT_MAC;
            let mut frame = vec![255; 6]; frame.extend_from_slice(&mac); frame.extend_from_slice(&[8, 6, 0, 1, 8, 0, 6, 4, 0, 1]);
            frame.extend_from_slice(&mac); frame.extend_from_slice(&[192, 168, 4, 2]); frame.extend_from_slice(&[0; 6]); frame.extend_from_slice(&[192, 168, 4, 1]);
            m.bus.receive_softap_frame(&frame).unwrap(); sent = true;
        }
        for frame in m.bus.take_softap_frames() {
            if frame.len() >= 42 && frame[12..14] == [8, 6] && frame[20..22] == [0, 2] && frame[28..32] == [192, 168, 4, 1] { reply = true; }
        }
        if reply { break; }
    }
    let status = m.bus.softap_client_status(); println!("{status}");
    assert!(sent, "SoftAP association failed: {status}"); assert!(reply, "guest SoftAP did not answer ARP: {status}");
    // RFC 791 / 768: IPv4 UDP with no options; an IPv4 UDP zero checksum is
    // permitted. This goes through the virtual peer's 802.11 TX and guest lwIP.
    let payload = b"c61-softap-udp"; let mut ip = vec![0x45, 0, 0, (28 + payload.len()) as u8, 0, 0, 0, 0, 64, 17, 0, 0, 192, 168, 4, 2, 192, 168, 4, 1];
    let mut sum: u32 = ip.as_chunks::<2>().0.iter().map(|v| u16::from_be_bytes(*v) as u32).sum();
    while sum >> 16 != 0 { sum = (sum & 0xffff) + (sum >> 16); }
    ip[10..12].copy_from_slice(&(!(sum as u16)).to_be_bytes());
    ip.extend_from_slice(&18081u16.to_be_bytes()); ip.extend_from_slice(&18080u16.to_be_bytes());
    ip.extend_from_slice(&((8 + payload.len()) as u16).to_be_bytes()); ip.extend_from_slice(&[0, 0]); ip.extend_from_slice(payload);
    let mut frame = vec![2, 0, 0, 0, 0, 0x62]; frame.extend_from_slice(&esp_soc::wifi::softap::CLIENT_MAC); frame.extend_from_slice(&[8, 0]); frame.extend_from_slice(&ip);
    m.bus.receive_softap_frame(&frame).unwrap();
    python(&mut m, "data,peer=u.recvfrom(128)\nassert data==b'c61-softap-udp',data\nu.sendto(b'c61-reply:'+data,peer)\nu.close()\nprint('C61_SOFTAP_UDP_PASS')\n", "C61_SOFTAP_UDP_PASS");
    let returned = m.bus.take_softap_frames(); let expected = [b"c61-reply:".as_slice(), payload].concat();
    assert!(returned.iter().any(|frame| frame.len() >= 42 && frame[12..14] == [8, 0] && frame[23] == 17 && frame[34..36] == 18080u16.to_be_bytes() && frame[42..] == expected), "guest UDP response missing: {returned:02x?}");
}
