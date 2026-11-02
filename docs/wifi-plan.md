# WiFi hardware emulation (full, unmodified firmware)

Goal (chosen 2026-08-25): run the **unmodified** Espressif WiFi stack — `esp_wifi` + the closed
`libpp`/`libnet80211`/`libphy` blobs — so firmware associates with an access point and moves IP
traffic, with **no application changes and no shim**. The PHY math is *not* emulated; we fake the
radio/analog registers so the blob's calibration passes, and model the **MAC** (the descriptor
rings and interrupt events) plus a **virtual access point** the MAC "hears".

This is reverse-engineering, done against a specimen: `examples/wifi-station` (the IDF station
example, open network, SSID `esp32sim`). Espressif never documented these peripherals; the register
map matches the classic ESP32's as reverse-engineered by **esp32-open-mac** (their `0x3ff73000` is
our `0x60033000`). Everything below was learned by tracing the blob's own accesses with
`--regstat`, `--trace-fn`, `ESP_EMU_DEBUG_WIFI` and disassembly of the ROM/library functions.

## Status

**Firmware joins the virtual AP and gets an IP address — open *and* WPA2-PSK — with the unmodified
blob and no firmware changes** (`--wifi ssid=esp32sim[,psk=...]`):

```
wpa: WPA: Key negotiation completed with 02:53:49:4d:00:01 [PTK=CCMP GTK=CCMP]
wifi:connected with esp32sim, aid = 1, channel 6, BW20, bssid = 02:53:49:4d:00:01
wifi station: got ip:10.0.2.15
```

What is modelled:
- PHY calibration loops satisfied with faked done-bits; scan, open-system auth, association.
- The 802.11 MAC (TX queue registers, RX descriptor ring, interrupt events) — see below.
- **Receive lists handed over one at a time**, as Arduino-ESP32 writes them: the DMA follows the
  `next` pointers, stops when the next descriptor is not software-armed, and restarts at
  `BASE_RX_DSCR` (0x088) when software hands it the next short list. The IDF station example writes
  the base once and keeps a long ring; an Arduino-ESP32 application (IDF 5.1) arms four descriptors at a
  time, and without the hand-over its station never authenticated — 314 of 315 frames were dropped
  ([decisions](decisions.md), [receipt](evidence/wifi-rx-handover/README.md)).
- A **virtual AP** (`esp-soc/src/wifi.rs`): beacons, probe responses, auth/assoc, and the WPA2
  four-way handshake (PMK from the passphrase, PTK derivation, MIC, GTK delivered AES-key-wrapped).
- The **AES accelerator** (`Aes` in `esp-periph/src/aes.rs`, DMA and block mode) — the supplicant unwraps the
  group key with it, so without this peripheral WPA2 stops dead at message 3.
- Crypto primitives (`esp-periph/src/crypto.rs`): SHA-1, HMAC-SHA1, PBKDF2, the 802.11 PRF, AES for
  every key length in both directions, AES key wrap — all checked against RFC/FIPS/802.11i vectors.
- A **virtual network** (`esp-soc/src/net.rs`): DHCP, ARP, ICMP echo, a DNS responder and an SNTP
  server that hands out the host clock, so `esp_netif` reaches `IP_EVENT_STA_GOT_IP` and firmware
  waiting for time gets it.
- A **user-mode NAT** (`esp-soc/src/nat.rs`, `--net nat`, on by default): TCP and UDP flows are
  terminated in the emulator and relayed over ordinary host sockets, the way Contiki-NG's NAT64
  does it — no libslirp, no root, no tun device. Guest name lookups go to the host's own resolver.
- The **RSA/MPI accelerator** (`Rsa` in `esp-periph/src/rsa.rs`) and **SHA over GDMA including SHA-384/512**.
  mbedTLS routes every public-key operation and every certificate digest through them, so without
  both, TLS hangs in the driver's polling loop or fails certificate verification.

**HTTPS works end to end.** The esp32-screen energy panel boots, joins WPA2, takes a DHCP lease,
syncs its clock, resolves `www.elprisetjustnu.se`, fetches two days of prices over TLS (200, 13.6 kB,
96 slots each) and polls the real Home Assistant on the LAN — its energy history, entity states and
control tiles all live:

```
hass: HA reachable, light.pool_pool = off
prices: -> status 200, 13603 bytes ... fetched 2026-08-25: 96 slots
energy: today: 56.3 kWh over 24 h
```

Bulk traffic is **not** encrypted: the emulated MAC presents plaintext framed as CCMP would be
(protected bit, 8-byte CCMP header with the right key id, 8 bytes of MIC space), which is what
firmware sees when real hardware encrypts and decrypts in place.

Bugs found on the way to a TLS session, each one a place where the model was plausible but wrong:

- **`0x818` on the RSA block is an idle status, not the interrupt latch.** The ISR clears the
  interrupt, then the result path waits for `0x818` to read non-zero — latching it deadlocked
  every interrupt-driven modular exponentiation.
- **IP payloads must be trimmed to the header's total-length field.** The 802.11 frame carries a
  4-byte FCS, which the NAT was feeding to the peer as TCP payload; the guest's real request then
  arrived at a sequence number the connection had already moved past.
- **mbedTLS hashes certificates through GDMA, not the block interface** — and reaches for SHA-384,
  which needs the 64-bit core. Digest-shaped garbage came back as `PK verify failed`.
- **The AES accelerator's CTR mode** (block mode 3) was being executed as ECB, which the server
  answered with a fatal alert rather than an error we could read.

Not yet working:
- UDP inbound, multicast and mDNS, so `esp-web.local` and Home Assistant discovery do not cross;
  use IP addresses. TCP servers in the guest are reachable with `--hostfwd`.
- Hardware AES-GCM (block mode 6) and the DS/HMAC peripherals; mbedTLS has not asked for them yet.
- Roaming, power save, 802.11n rates, multiple stations, WPA3/SAE, PMF.

## Scope

Getting *unmodified* firmware to "joined + internet" meant walking the blob's state machine to
CONNECTED with frames a real AP would send — no register or flag shortcuts it — and then giving the
resulting Ethernet traffic somewhere to go. Both halves are done; the shim and OpenCores-Ethernet
routes sketched in docs/networking-plan.md were never needed and are not built.

## What was reverse-engineered

MAC register file at **`0x60033000`** (block 0x33) / `0x60034000` (0x34), WDEV at `0x60035000`:

| Register | Meaning |
| --- | --- |
| `0x088` `WIFI_BASE_RX_DSCR` | RX descriptor ring base (hardware fills from here) |
| `0x08c/0x090` | next / last RX descriptor |
| `0x084` bit 0 | RX descriptor reload (restart at base) |
| `0xc3c` / `0xc40` | MAC interrupt events / clear (bit 7 TX done, bits 14/24 RX data) |
| `0xcb0` / `0xcac` | per-queue TX complete / clear |
| `0xca8` / `0xca4` | per-queue TX error / clear |
| `0xd08 − 8·q` `MAC_TX_PLCP0[q]` | queue q descriptor addr; bit 31 triggers TX |
| `0xd14` | `hal_init` handshake (write bit 1, poll bit 0) |
| `0x040/0x060` (per slot) | MAC address / BSSID filters |
| `0x0d8` | RX policy |
| WDEV `0x0c/0x10/0x14/0x18/0x1c` | TSF counter: latch/load and the 64-bit value |
| WDEV `0x118/0x11c` | power interrupt events / clear |

DMA descriptor (esp32-open-mac `dma_list_item`, **confirmed on silicon 2026-08-25** via JTAG on the
Atech board running this specimen): `size:12 length:12 _:6 has_data:1 owner:1`, then `packet` and
`next` pointers. A filled RX descriptor reads `dw0=0xc0..` — the hardware sets has_data (bit 30) and
**leaves owner (bit 31) set**; the register pointers (`0x088/8c/90`) hold the low 20 bits over
`0x3FC0_0000`, while the in-descriptor `packet`/`next` are full addresses. `rx_last` carries a `0x01`
prefix (bit 24). The 48-byte `rx_ctrl` header begins with the signed RSSI in the low byte of word 0. RX buffer = a 48-byte `wifi_pkt_rx_ctrl` header (rssi, rate, channel,
timestamp, `sig_len`, `rx_state`) + the 802.11 frame + 4-byte FCS; word 0 top bits are the
frame-valid / filter-match flags `wDev_ProcessRxSucData` gates on.

## Hardware ground truth

The real Atech board (ESP32-S3, same silicon) is used as the oracle: flash this specimen, let it
receive real beacons off the air, then over the built-in USB-JTAG (`openocd-esp32` + gdb) `halt` and
read the live WiFi MAC registers and RX descriptor ring — `hw/difftest*.sh` show the openocd/gdb
setup. This confirmed the descriptor bit layout (owner stays set), the masked register-pointer format,
and the 48-byte `rx_ctrl` header. Reflashing is reversible: [atech-firmware](https://github.com/joakimeriksson/atech-firmware) rebuilds the synth,
or `esptool write_flash 0 hw/atech/flash-8M.bin` restores the original dump byte-for-byte.

## Tools added for this work

- `--regstat FILE` — per (address, pc, r/w) access counts with the resolved symbol.
- `--trace-fn PREFIX` — log each entry to functions matching a prefix, with args and caller.
- `--stub SYMBOL[=val]` — synthetic return at a function entry.
- `--wifi SPEC` — attach a virtual AP.
- `--poke` script action; `ESP_EMU_FAKE_READ=addr:or[:and],…` runtime register overrides.
- `ESP_EMU_DEBUG_WIFI` (register trace) / `ESP_EMU_DEBUG_WIFI_FRAMES` (frame decode).

References: esp32-open-mac (github.com/esp32-open-mac/esp32-open-mac, `main/hardware.c`,
`main/mac.c`) and its blog (zeus.ugent.be/blog/23-24/open-source-esp32-wifi-mac/); Ebiroll's and
esp32-open-mac's QEMU forks for the classic ESP32 (Apache-2.0, consulted for behaviour only).
