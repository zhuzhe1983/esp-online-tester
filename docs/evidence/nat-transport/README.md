# EX258: a pluggable host transport for the user-mode NAT

Base: upstream/main `2e36224d`, then main `dd4796a0` merged in, which brought #278 (`--hostfwd`);
the firmware runs and the checks are on this branch with it, against main `dd4796a0`. The final
source is the commit containing this receipt. Inputs, hashes and the exact commands are in
[inputs.json](inputs.json); the checks that ran and their results in [checks.json](checks.json).

`Nat` terminated guest flows directly on `std::net` sockets, so only a host with sockets could
carry the guest's traffic. A WebSocket relay for the WASM build (roadmap item 9, #303) needs the
same flows over a different network. This puts the host side behind a trait:

- `NatTransport` (`esp-soc/src/nat/transport.rs`): `tcp_connect` / `tcp_state` (`Connect::Pending`,
  `Open`, `Failed(ErrorKind)`) / `tcp_read` / `tcp_write` / `tcp_shutdown` / `tcp_close(abort)`,
  `udp_open` / `udp_send` / `udp_recv` / `udp_close`, `resolver()`, the optional `resolve` /
  `resolved` (`Resolve::Addrs(addrs, ttl)` or `Failed(rcode)`) for a transport that looks names up
  itself, and the optional `tcp_accept` for connections from the host side. Reads and writes follow
  non-blocking `std::io` rules.
- `StdTransport` (`nat/std_transport.rs`) is the old code moved as is: `std::net` sockets, a
  connect worker thread per SYN under the process-wide `ConnectPermit` limit of 8, the resolver
  from `/etc/resolv.conf` (1.1.1.1 without one). `Nat::new(log)` is
  `Nat::with_transport(log, Box::new(StdTransport::new()))`. Neither is in the wasm32 build
  (`#[cfg(not(target_arch = "wasm32"))]`): there the sockets and threads would only fail with
  `Unsupported`, so a browser host picks its transport with `Nat::with_transport`.
- `--hostfwd` (#278) on the trait. `StdTransport::listen` binds the listeners (at most 8), which
  are not part of the trait since a browser cannot listen; `tcp_accept` hands each accepted
  connection to the NAT as an open flow with its guest address and port. The NAT keeps #278's
  rules: accepted flows wait (at most 16, the oldest closed) until the station MAC is known, then
  go to the guest as a SYN from the gateway with a port from 50000 up, nothing is read from them
  before the guest's SYN-ACK, and a guest RST to that SYN counts as refused. The CLI builds the
  `StdTransport`, calls `listen` for each spec and hands it to `Nat::with_transport`; `Nat::hostfwd`
  keeps the report line's listener count.
- The flow's private socket enum became `FlowState { Connecting(id), Connected(id), TimeWait,
  Closed }`. The NAT calls `tcp_close` exactly once per flow (`abort` on a guest RST, a failed
  connect, a host read / write error, `abort_all`; otherwise on TIME_WAIT entry or idle expiry; an
  accepted flow dropped from the full waiting list closes too) and `udp_close` once per UDP flow
  (eviction or idle expiry). `tcp_flow_counts` (#288) counts `FlowState` the way it counted the
  socket enum.
- `Nat::dns_query` hands a guest A / AAAA query to a transport that resolves names itself (at
  most 32 waiting, the oldest gives way, 30 s expiry) and answers from the address the guest
  asked, with the query's transaction id; `false` (the std transport) keeps relaying the datagram
  to `resolver()`. The answer is built by `net::packet::dns_reply`, moved out of
  `VirtualNet::dns`, which uses it too (`dns_question` parses the question for both).
- `Nat::abort_all` drops every flow, resetting the transport's, the accepted ones still waiting for
  the station included; `StationLink::surviving_reboot` calls it, since the rebooted guest's
  sockets are gone.
- `VirtualNet::local_ntp` (default false) answers SNTP from the network clock even with a NAT,
  for a transport that cannot reach an NTP server. It composes with `epoch=`.

Two changes are visible on the native path:

- A failed connect logs the error kind (`connection refused`, `address not available`) instead of
  the OS message.
- A guest reboot aborts every NAT flow and closes its host socket at once. Before, the old guest's
  flows and their sockets stayed until the 120 s idle reaper (or TIME_WAIT) removed them.

Nothing runs per tick: `Nat::poll` runs only while a NAT is attached (`--net nat` with `--wifi`).
Every golden runs without one (`--net none`), so the goldens, unchanged, say nothing about the NAT
path; the evidence for it is the socket tests and the firmware runs below.

## Tests

`esp-soc/src/nat/tests.rs`:

- The 14 socket tests from before run unchanged on `StdTransport` (SYN / data / FIN retransmission
  and cumulative ACK, partial and wrapping ACKs, in-order and idempotent FIN, bounded guest buffer,
  short writes and WouldBlock, FIN after host write drain, SYN-ACK resend, host reads bounded by
  the window, zero-window probe, connect permits, TIME_WAIT eviction, UDP capacity, malformed TCP
  headers, UDP peer filtering), and #278's three `--hostfwd` tests run on `StdTransport::listen`
  in place of `Nat::listen` (the spec parser; a connection held until the station MAC is known,
  then relayed both ways; a guest RST to the forwarded SYN counted as refused).
- 15 new, against a scripted mock transport: a SYN opens a transport flow and guest data is written
  to it; WouldBlock closes the guest window without losing bytes; transport EOF becomes a guest FIN
  and the close is graceful; a refused connect resets the guest and forgets the flow; a full
  transport drops the SYN until its retransmission; a guest RST aborts the transport flow;
  `abort_all` resets every flow; a guest reboot aborts the NAT's flows; a resolving transport
  answers DNS from the guest's resolver address; a failed lookup is NXDOMAIN and an empty one has
  no records; DNS is relayed when the transport does not resolve; at most 32 lookups wait and the
  oldest gives way; `local_ntp` answers SNTP with a NAT; an accepted flow waits for the station,
  is not read before the guest's SYN-ACK, then runs like an outbound one and is closed once; and
  accepted flows past the waiting bound, and those still waiting on `abort_all`, are closed.
- For the accepted flows, five mutations each fail at least one test: no close on `abort_all`, no
  close past the bound, no wait for the station MAC, no refused count for the guest's RST, a read
  before the SYN-ACK.

`nat/tests/counts.rs` (#288's eight tests) asserts the same counts as before. The two that
injected a connect closure (`tcp_in_with_connect`, which this change removes with the socket
enum) now script the mock: a refused admission, and a ready or refused connect that a snapshot
must not consume. The four that need a real host socket (a guest RST, a disconnected connector,
a host I/O failure, half-close and TIME_WAIT) run on `StdTransport`.

`cli/tests/hostfwd.rs` (#278's two tests) passes unchanged: the flag checks, and the listener
bound and logged before the ROM is required.

## Firmware

Both probes are Arduino-ESP32 3.3.12 sketches for the S3, run on main `dd4796a0` and on this
branch on a desktop with internet access (commands in inputs.json).

Outbound, [wifi-net-probe.ino](wifi-net-probe.ino): scan, join the open network `esp32sim`, DHCP,
`WiFi.hostByName("example.org")`, SNTP from `pool.ntp.org`, then `GET http://example.org/`, with
`--wifi ssid=esp32sim --net nat --debug net` for 40 emulated seconds and `nameserver 8.8.8.8` as
the resolver file:

| Build | Console | Reports |
| --- | --- | --- |
| main | `ip=10.0.2.15 gw=10.0.2.2 dns=10.0.2.3`, `dns: example.org -> 104.20.26.136`, `time: 2026-11-02 09:08:01 UTC`, `http GET http://example.org/ -> 200, 577 bytes` | `[emu] nat: 1 TCP connections (0 failed), 3 UDP flows (0 evicted, 0 send errors), 278 bytes out, 1263 bytes in`; `[emu] net: ... 1 frames ignored` |
| this branch | the same lines (`09:08:04 UTC`) | the same two lines |

DNS, SNTP and HTTP went through the transport to the real network on both builds; the time is the
host's wall clock. In both runs lwIP sent its `pool.ntp.org` query twice on the same port and
answered the late reply with an ICMP port unreachable (the frame ignored). An earlier run of the
same probe, on this branch's first head (base `2e36224d`), once saw the first reply in time
instead (248 bytes out, 1169 in, no frame ignored); that is the real network's timing. Instruction counts vary
with it: 87.6 M + 52.7 M on main, 87.7 M + 53.2 M on the branch.

Inbound, [hostfwd-server.ino](hostfwd-server.ino): join `esp32sim`, serve TCP port 80 and answer
each connection with one line. Run with `--wifi ssid=esp32sim --net nat --hostfwd
tcp:127.0.0.1:18080-:80 --realtime` for 40 emulated seconds; once the console says
`listening on 80`, a Python client on the host connects to 127.0.0.1:18080 three times in a row,
then twice at once, sending `hello <tag>`:

| Build | Client | Guest console | NAT report |
| --- | --- | --- | --- |
| main | all five answered (`conn N got 10 bytes: hello <tag>`, 0.02-0.04 s each) | `conn 1..5 from 10.0.2.2` | `5 TCP connections (0 failed, 5 inbound, 1 hostfwd), 0 UDP flows (0 evicted, 0 send errors), 160 bytes out, 55 bytes in` |
| this branch | all five answered (0.02-0.03 s each) | `conn 1..5 from 10.0.2.2` | the same line |

The guest sees each connection come from the gateway, as #278 has it. Which of the two parallel
connections it took first differed between the runs (host timing).

## CPU comparison

Not run: this change touches neither the bus, `device_set!`, interrupt routing nor per-tick
work. Without `--wifi` there is no virtual network and no NAT, so none of the moved code runs on
the hello demos.

## Limits

- `StdTransport` has no portable abortive close: `tcp_close(id, true)` closes like a graceful
  close (as before).
- The resolver file lookup is unchanged (`/etc/resolv.conf`, else 1.1.1.1). Windows resolves the
  path against the current drive; the outbound runs above used a drive mapped (`subst`) to a
  directory holding `etc/resolv.conf`.
- The self-resolving, `local_ntp` and `tcp_accept` contracts are exercised by the mock transport;
  no transport other than `StdTransport` is in this change, and the wasm32 build has no
  transport yet.
- Pure host-side code: no register or silicon claim.
