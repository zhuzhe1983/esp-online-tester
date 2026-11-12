# Running firmware with the network

How to give an unmodified firmware image a WiFi connection and let it reach the real network.
What is behind it: [wifi-plan.md](wifi-plan.md) (the MAC model and the virtual AP),
[networking-plan.md](networking-plan.md) (the subnet and the NAT).

## The short version

```sh
esp32sim --board none --boot rom --console usb --max-seconds 20 \
    --bootloader build/bootloader/bootloader.bin \
    --ptable build/partition_table/partition-table.bin \
    --app build/your_app.bin --elf build/your_app.elf \
    --wifi "ssid=esp32sim,psk=esp32sim-pass"
```

`--wifi` attaches the access point *and* the network behind it; there is nothing else to turn on.
The station comes up on **10.0.2.15/24**, gateway **10.0.2.2**, resolver **10.0.2.3**, and outbound
TCP/UDP is relayed to the host's own network (`--net nat`, the default).

The SSID and passphrase must match what the firmware is configured to look for — the emulator's AP
adopts whatever you give it. The AP is WPA2-PSK when `psk=` is present and open when it is not.

| Key | Meaning | Default |
| --- | --- | --- |
| `ssid=NAME` | network name the AP beacons | `esp32sim` |
| `psk=PASS` | WPA2-PSK passphrase; omit for an open network | none (open) |
| `chan=N` | channel in beacons and probe responses | 6 |
| `bssid=xx:xx:..` | AP MAC | `02:53:49:4d:00:01` |

`--net none` keeps the emulated subnet (DHCP, DNS, SNTP still answer) but refuses anything past the
gateway with an immediate RST, so applications fail fast instead of hanging — useful when you want a
run to be reproducible and offline.

`--hostfwd tcp:127.0.0.1:8080-:80` (repeatable) listens on the host and completes a TCP handshake
with a server in the guest: a browser on the Mac opens `http://127.0.0.1:8080/` and the station
sees a SYN from 10.0.2.2 to 10.0.2.15:80. Empty host address binds loopback; empty guest address is
the station. The listener is bound when the run starts; SYNs go out once the station's MAC is known
(association). A connection is not held until the guest's server listens: if the SYN arrives before
the firmware calls `listen()`, the guest answers with RST and the host client sees a reset (QEMU's
`hostfwd` behaves the same), so connect once the firmware is listening, or retry. Such a refusal
counts as failed in the `[emu] nat:` report line. Requires `--wifi` and `--net nat`.

The same `--wifi` and `--net` work on the **ESP32-C6** (`esp32sim-c6`): the access point and the
network are the same code, the MAC model is the C6's own ([wifi-c6-plan.md](wifi-c6-plan.md)). What
has been tried is one station on one open or WPA2 network; `examples/c6-wifi-station` has the full command. The C3 also supports `--wifi` with open/WPA2 station mode and the same network backend.

## What the network gives the firmware

- **DHCP** — address, mask, gateway, DNS, so `esp_netif` reaches `IP_EVENT_STA_GOT_IP`.
- **DNS** — with NAT, queries go to the host's first `nameserver` from `/etc/resolv.conf` and the
  answer comes back looking as if 10.0.2.3 produced it. With `--net none`, the built-in responder
  answers every A query with 10.0.2.3 itself, so lookups succeed and the connection is then refused.
- **Time** — with NAT, SNTP requests reach the real server the firmware asked for. With `--net none`,
  the built-in SNTP server answers (stratum 1) so firmware that waits for time still gets it in the
  first seconds: the network's clock plus emulated time since power-on. `--net none,epoch=S` fixes
  that clock (seconds since 1970) and makes such a run the same every time; without it the host's
  clock at the first answer sets it.
- **ICMP echo** — answered inside the emulator for any address, including internet ones; pings never
  leave the host, so they are not a reachability test.
- **TCP/UDP out** — real connections to real hosts: an HTTP API, a Home Assistant instance on the
  LAN, MQTT, HTTPS (TLS runs on the emulated AES/SHA/RSA accelerators).

## Watching what happens

| Switch | Shows |
| --- | --- |
| `ESP_EMU_DEBUG_NET=1` | DHCP/ARP/ICMP/DNS exchanges and every NAT flow |
| `ESP_EMU_DEBUG_WIFI=1` | MAC-level events: descriptors, interrupts, TX queues |
| `ESP_EMU_DEBUG_WIFI_FRAMES=1` | every 802.11 frame on the air, decoded |
| `ESP_EMU_DEBUG_AES/SHA/RSA=1` | each accelerator operation as firmware requests it |
| end-of-run `[emu] wifi/net/nat/crypto` lines | frame, lease, flow, byte and operation counts |

## When it does not connect

- **`Send disconnect event, reason=210`** — the firmware has a passphrase compiled in and refuses an
  open AP (`NO_AP_FOUND_W_COMPATIBLE_SECURITY`). Pass a matching `psk=`.
- **No beacons seen / scan finds nothing** — check the SSID matches exactly, including case.
- **Connects but no IP** — look for the DHCP exchange with `ESP_EMU_DEBUG_NET=1`; if the four-way
  handshake did not finish, the station is not yet decrypting anything.
- **TLS fails or hangs** — the accelerators are the usual suspects; the `[emu] crypto:` counters
  show whether AES, SHA and RSA are all being exercised.
- **A host on the LAN is unreachable** — the NAT connects from the *host's* address, so the target
  must be reachable from the Mac, and any firewall there sees the Mac, not the guest.

## Known limits

- **UDP inbound** is not forwarded. `--hostfwd` is TCP only; multicast and mDNS still do not cross.
- **Multicast and mDNS** do not cross the NAT: `something.local` will not resolve, and Home
  Assistant / ESP-IDF discovery protocols will not see anything. Use IP addresses.
- **UDP reply peers** must match the destination IP and port of the outgoing datagram. The
  [connected UDP relay](../esp-soc/src/nat.rs) does not support TFTP's server-selected transfer
  port or replies from a different address on a multi-homed server. Failed host sends drop
  the datagram without retrying and increment `udp_send_errors`,
  with details under `ESP_EMU_DEBUG_NET=1`. The 64-flow UDP table evicts its least recently
  active flow for a new destination or guest port and increments `udp_evicted`; an evicted
  flow's outstanding replies are lost.
- **One station**, no roaming, no power save, no 802.11n rates, no WPA3/SAE or PMF.
- **Traffic is not really encrypted** over the air — frames are plaintext framed as CCMP, which is
  what firmware sees anyway, but a capture of the emulated air is not a realistic WPA2 capture.
