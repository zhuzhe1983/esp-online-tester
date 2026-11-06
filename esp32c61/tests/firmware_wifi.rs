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
fn external_official_uiflow_wifi_dns_ntp_http() { official_wifi(None); }
#[test]
#[ignore = "needs C61_ROM_ELF, COREMATRIX_FIRMWARE and C61_TEST_HTTP_BIND (host IPv4 reachable by guest NAT)"]
fn external_official_uiflow_wpa2_wifi_dns_ntp_http() { official_wifi(Some("c61-test-password")); }

fn official_wifi(password: Option<&str>) {
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
}
