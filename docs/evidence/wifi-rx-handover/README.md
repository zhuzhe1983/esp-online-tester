# Receive lists handed over one at a time (S3 MAC)

**Question.** An Arduino-ESP32 application (IDF 5.1.2) boots and runs in esp32sim but its station never
associates: the virtual AP answers probe requests, the station never authenticates, and the end-of-run
report reads `... 1 received (314 dropped: no descriptor); AP: ... 0 data frames from the station, state
Idle`. The ESP-IDF station specimen on the same blob associates normally. Why does the receive path stop
after the first frame?

**Answer.** Arduino-ESP32 hands the receive DMA **one short descriptor list at a time** instead of one
long ring, and starts the next list by writing `BASE_RX_DSCR` (MAC `0x60033088`). The model treated a
base write as a no-op unless `rx_next` was still null, so after the first list was consumed the DMA
stayed pointed at an unarmed descriptor for ever and every later frame was dropped. A stopped DMA now
takes the pending base as its new head and retries the frame
(`esp32s3/src/bus.rs::wifi_rx_deliver`, `esp32s3/src/periph.rs` `WifiMac::write`).

## What the two drivers do differently

Both firmwares drive the same modelled MAC (0x60033000 block) through Espressif's closed
`libpp`/`libnet80211` blob, but from different IDF generations: the application is built against
`idf-release_v5.1-3662303f31` (`v5.1.2-185-g3662303f31-dirty`, from the image's own strings), the
station specimen against **IDF 5.5.4**. The receive-list management differs between them; the virtual
AP, channel and blob API do not.

| | IDF station specimen (`examples/wifi-station`) | Arduino-ESP32 application (IDF 5.1) |
| --- | --- | --- |
| `BASE_RX_DSCR` (0x088) writes | 1, at start-up | one per list, changing address |
| descriptor stride / buffer size | 12 bytes, 1600 (`dw0=0x80640640`) | 12 bytes, 2212 (`dw0=0x808a48a4`) |
| list length | long ring, all descriptors armed | 4 descriptors, last one's `next` null |
| restart signal | `0x084` bit 0 after every frame | `0x088` write of the next list's head |
| frames dropped by the model before the fix | 0 | 314 of 315 |

Traced with `--debug wifi` (every MAC register write) and the RX path's drop line:

```
[wifi] wr 0x33+0x088 <- 0x3fcb3f0c            # list 1, one usable descriptor
[wifi] RX -> desc 0x3fcb3f0c 108 bytes beacon (0/8) ...
#   desc 0x3fcb3f0c: dw0=0x808a48a4 (size 2212, owner 1), buf=0x3fcb3f4c, next=0x3fcb3f18
#   desc 0x3fcb3f18: dw0=0, buf=0, next=0      -> the DMA has nothing to fill: stopped
[wifi] wr 0x33+0x088 <- 0x3fcb6570            # list 2, head written *before* the next frame is due
[wifi] RX dropped: rx_next 0xb3f18 is not a usable descriptor (base 0xb6570, last 0x10b3f0c)
#   list 2 at 0x3fcb6570: four armed descriptors (dw0=0x808a48a4, last next=0)
```

The second write arrives while the DMA is still processing the first list, so a model that looks at the
base only when a frame is due has to remember it: hence `WifiMac::rx_handover`, set by a base write,
cleared by the next frame that goes in. That clearing is an **assumption**: it holds for a driver that
publishes the next list's base after the current one is exhausted, which this blob does (1 drop in 570
frames); a driver that published it while two descriptors were still armed would need the latch held
until it is consumed (see `docs/decisions.md`).

## Reproducibility inputs

- **esp32sim:** before, `main` `63670bb1` (the base this branch was cut from; the drop numbers were taken
  there); after, the code commit of this branch —
  `s3: restart a stopped receive DMA at BASE_RX_DSCR`, rebased onto `ddf753a5` (the SHA moves with the
  rebase, `git log --oneline` shows it).
- **Application under test:** a private Arduino-ESP32 3.x application for the ESP32-S3, built against
  `idf-release_v5.1-3662303f31` (`v5.1.2-185-g3662303f31-dirty`, read from the image's own strings). The
  image is **not redistributed**, so this receipt carries no app/bootloader/partition-table hash or size
  and no `external_` test can be added; the traces, register values and counters above are what the run
  left behind. Everything the model does is independent of that application's code — it drives the same
  `libpp` blob as the IDF specimen below.
- **Control specimen:** `examples/wifi-station` in this repository, built in-tree with ESP-IDF 5.5.4
  (`idf.py -C examples/wifi-station build`); no prebuilt image is kept, which is why its numbers are
  reproducible from the checkout rather than from a hash.
- **Host:** 4-core Intel Pentium J4205 (1.5 GHz base), Linux 6.12, `--realtime` off. Emulated 25 s per run
  as stated under Numbers; counters are the guest's own, so host speed changes wall time only.

## Numbers

Emulated 25 s, one unmodified release image, `--wifi ssid=bench,chan=6,psk=bench-password`,
`--max-seconds 25`, protocol frames replayed into UART1 from 10 s to 30 s.

| | before | after |
| --- | --- | --- |
| end-of-run `[emu] wifi:` | 166 sent, **1 received, 314 dropped**, AP 244 beacons / 71 probe responses / 0 data frames, `state Idle` | 226 sent, **570 received, 1 dropped**, AP 244 beacons / 22 probe responses / 169 data frames, `state Associated` |
| four-way handshake | never reached | `[wifi] WPA2 four-way handshake complete` |
| `[emu] net:` | 0 DHCP leases | 1 DHCP lease, 2 ARP replies |
| `[emu] nat:` | 0 TCP connections | 1 TCP connection (MQTT), 3 UDP flows, 8541 bytes out |
| application console | `Wifi: -1% (RSSI: 0)`, `MQTT connect failed` | `Wifi: 100% (RSSI: -40)`, `Connecting to mqtt server ...` |
| topics seen on the host | none | one topic carrying the DHCP address (`10.0.2.15`), 144 value topics in the window |

The IDF station specimen is unchanged by the fix: `46 sent, 146 received (0 dropped), state
Associated`, `sta ip: 10.0.2.15`.

## Commands

```sh
# the specimen that never starved, before and after the change (binaries from the browser fixtures)
esp32sim --board none --chip s3 --boot rom --console usb --max-seconds 12 \
  --rom <roms>/esp32s3_rev0_rom.elf --bootloader s3-wifi-bootloader.bin \
  --ptable s3-wifi-ptable.bin --app s3-wifi_station.bin --wifi ssid=esp32sim,psk=esp32sim-pass

# the application that starved: released image with its filesystem partition
esp32sim --chip s3 --boot rom --console usb --debug wifi,wifi-frames --max-seconds 25 \
  --rom <roms>/esp32s3_rev0_rom.elf --bootloader bootloader.bin \
  --ptable partitions.bin --app app.bin \
  --flash-at 0x3d0000=littlefs.img --flash-mb 4 --psram-mb 2 --board none \
  --wifi ssid=bench,chan=6,psk=bench-password --script frames.txt

# the unit tests that pin both halves of the rule
cargo test -p esp32s3 --lib bus::gp_spi_board_tests::air_rx

# against the code before the change, the first of those two tests fails at
#   "the base write restarts the stopped DMA": left 1, right 2
```

## Inputs

The image under test is a third-party released build and is **not** redistributed here; it was not
modified, and every number above comes from that unmodified image. The filesystem partition is a
128 KiB LittleFS image built with `mklittlefs -b 4096 -p 256 -s 131072` from a directory holding the
application's configuration file.

Host: x86_64 Linux, 4 cores, Rust stable, esp32sim release build. The guest's own execution is
cycle-deterministic: repeated runs match on instructions, frames sent and frames delivered. The frames
that carry host-relayed traffic, and the NAT byte counters, move by a frame or two between runs because
the relay runs on a host thread (569 vs 570 frames received in two otherwise identical runs; 113 vs 161
bytes in). The numbers above are from one run of that pair; the two runs quoted in
`docs/experiments.md` (EX238) are before and after the change.

## Limits

- The MQTT evidence needs a broker reachable from the host at the address in the guest's configuration.
  The NAT opens a host socket for that exact address, so `127.0.0.1` in the guest never reaches it
  (lwIP routes it to its own loopback: `0 TCP connections`); a test bench uses the host's LAN address
  instead.
- Only the ESP32-S3 MAC gained the hand-over. The C3, C6 and classic models have their own copies of
  this path and were not changed: no specimen there starves its descriptor list, so a change would be
  unverified on those chips. If one does, the same rule applies.
- The restart is triggered by the *stopped* state, not by the base write alone: rewinding a running DMA
  is still wrong (see `docs/decisions.md`), and the second unit test pins that.
- This says nothing about throughput: one frame per `RX_GAP_US` with a 50 ms stall window when software
  is slow to recycle, as before.
