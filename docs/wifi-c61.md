# ESP32-C61 Wi-Fi digital model

The independent C61 model implements the C61 MAC v3 register interface, bounded HP SRAM TX/RX descriptors, and the PHY handshakes exercised by its pinned libraries. It uses the existing protocol-level virtual AP/network. The official CoreMatrix UIFlow2 2.5.4 image now passes scan, authentication, association, DHCP, DNS, UDP/NTP, a real host-socket HTTP exchange and WLAN disable/reconnect, with both open and WPA2-PSK access points. Its open and WPA2 SoftAP modes also authenticate a virtual client, answer ARP and exchange UDP application payloads. TLS 1.2 certificate/hostname verification and a 4096-byte HTTPS response pass after the test explicitly works around the pinned firmware’s known epoch bug; unmodified UIFlow2 2.5.4 with the correct current RTC still fails certificate expiry checks. This does not establish complete firmware compatibility or physical RF accuracy.

The target image is M5Stack UIFlow2 CoreMatrix 2.5.4, SHA-256 `8d5b8d4e22ebc293937086b2f9f218aa5a5ebda27b5ccb32b3f8f580ee02939b`; its application descriptor names `v5.5.4-dirty`. ESP-IDF v5.5.4 pins `esp32-wifi-lib` at `b87cce81803b85c3639fb14bac6c24cff6d0fbad` and `esp-phy-lib` at `3d57415af6e4c92eff2c4c3463e20a51d7340aba`. Selected unrelocated instruction sequences from those libraries occur unchanged in the image; other short sequences do not, so the entire dirty SDK is not asserted to be identical to the upstream tag. The positive and negative matches, exact URLs, download times, byte counts and hashes are retained in [source-audit.json](evidence/c61-wifi/source-audit.json). No firmware or library binary is committed.

Sources:

- [C61 library objects at the IDF-pinned Wi-Fi revision](https://github.com/espressif/esp32-wifi-lib/tree/b87cce81803b85c3639fb14bac6c24cff6d0fbad/esp32c61).
- [C61 PHY at the IDF-pinned PHY revision](https://github.com/espressif/esp-phy-lib/tree/3d57415af6e4c92eff2c4c3463e20a51d7340aba/esp32c61).
- [ESP-IDF v5.5.4 C61 capabilities](https://github.com/espressif/esp-idf/blob/v5.5.4/components/soc/esp32c61/include/soc/soc_caps.h): `SOC_WIFI_MAC_VERSION_NUM=3`.
- [MAC-v3 public RX control layout](https://github.com/espressif/esp-idf/blob/v5.5.4/components/esp_wifi/include/esp_wifi_he_types.h).
- [C61 interrupt sources](https://github.com/espressif/esp-idf/blob/v5.5.4/components/soc/esp32c61/include/soc/interrupts.h): Wi-Fi MAC source 0, power source 2.

The undocumented registers below were read from the named C61 archive functions with Espressif `riscv32-esp-elf-objdump -dr`. They are library-derived observations, not published register specifications or silicon measurements. The C6 implementation supplied the protocol transport structure, but its PHY offsets and RX layout are deliberately not reused.

| Function / operation | C61 address / format | Relevant difference from C6 |
| --- | --- | --- |
| `phy_freq_chan_en_sw` | `600a001c`: channel index bits 6:0, start bit 17 | C6 uses `600a00c0`, bits 13:7 / 14 |
| `phy_set_chan_freq_sw_start` | `600a0028`: done bit 8 | C6 status at `600a00cc` |
| `phy_iq_est_enable_new` | `600a0450`: enable bit 0 / start bit 1; `600a047c` done bit 16 | C6 at `474` / `4a0` |
| `phy_txdc_cal` | `600a0418`: start bit 0, done bit 22 | Same particular handshake; separately checked |
| `phy_pwdet_wait_idle` | `600a080c`: bits 16:14 must be 7 | C6 status at `814` |
| `hal_init` | `600a4ddc`: request bit 1, ready bit 0; interrupt enables `600a4c40` | Same selected offsets, independent model |
| Interrupt events / W1C | `600a4c48` / `600a4c4c`; TX bit 7, RX bit 14 | `wDev_ProcessFiq` dispatch checked |
| RX base / next / last | `600a4084` / `088` / `08c` | Descriptor low 20 bits; high address from `600a4c70` |
| RX reload | `600a4080` bit 0 | Self-clearing; never rewinds an active usable ring |
| TX queue start | `600a4d6c - 16*n`, queues 0–10, bit 31 | `hal_mac_txq_enable` prefix found in image at `32b802` |
| TX completion / W1C | `600a4cb8` / `600a4cb4` | Low 11 queue bits |
| TX result | `600a5514 - 120*n` | C6 uses `600a54e8 - 116*n` |
| Power events | `600ad8b8` / `600ad8bc` | Unimplemented, remain register storage; C6 uses `600ad0b0` / `0b4` |
| TX DMA | descriptor size/length 14 bits, owner bit 31; prefixed unaggregated mode | `ppTxProtoProc` and `dbg_read_tx_ppdu` skip 8 bytes when ebuf flag `2000` is set; `mac_tx_set_len+11a` reads packet word 0 bits 13:0; frame length includes four-byte FCS reservation (`ppAddTimCount+3a` subtracts 4) |
| RX DMA | 64-byte control header with no optional dumps, then frame/FCS | C6 uses 92 bytes; `get_sublen_offset` returns 56 before the 8-byte length/state words |
| RX metadata | RSSI byte 0, address-match bits 28/29, group byte 11 bit 7, timestamp bytes 12–15, noise byte 20, length/dump bytes 56–59 | Firmware writes channel byte 28; zero CSI/SIG-B data |
| RX interface | word 0 bit 28 selects STA; bit 29 without bit 28 selects SoftAP | `ppRxPkt+98` and `+14a` select different callbacks; actual dual-mode firmware covered |

Calibration samples are ideal zeros, completion is synchronous, and the power detector reports an idle state. These choices allow the guest's calibration code to run; they do not simulate radio amplitude, channel noise, acquisition delays, antenna behavior, interference or transmit power. Wi-Fi power-save/TSF, unprefixed/chained TX, HE aggregation and multiple peer behavior remain unqualified. The current TX path accepts the prefixed, unaggregated descriptor form; the official image's tested protocol/application operations work through it. Pending TX work is bounded to one pointer per hardware queue. Out-of-range SRAM accesses neither read host memory nor panic. The RX hand-over policy retains the shared-model correction described by EX238, but has not been measured on C61 hardware. The guest completes the WPA2 four-way handshake with the shared AP; data uses the digital MAC's plaintext/CCMP-header boundary, not simulated encrypted radio transmission. The shared AP is a test peer: it logs the message-2 MIC comparison but does not reject a mismatch, and it does not verify message-4 MICs. It is not a security boundary or a complete authenticator. The real guest rejects an incorrect password in the negative fixture.

Integration hooks are `ModemBb` at peripheral block `0xa0`, `WifiMac` at `0xa4` (MAC IRQ 0), and optional block `0xa5` register alias with delta `0x1000`. After MMIO/ticks, only call `WifiMac::service(sram, guest_us)` when `has_pending_work()` is true. `StationLink` supplies AP attachment, Ethernet relay, reboot-surviving host network state and reports. Frontend network availability must follow firmware-driven qualification, not the existence of these hooks.

`cargo test -p esp32c61 --lib wifi` passes four focused tests for C61-specific PHY commands, RX metadata and descriptor ownership, event clearing, malformed DMA bounds, and a bidirectional protocol exchange. The latter writes guest SRAM descriptors and MAC registers for probe, authentication and association frames, verifies the AP's response bytes/FCS in the C61 RX layout, then relays Ethernet payloads in both directions.

The separate `firmware_wifi` integration tests boot the supplied official image through the genuine ROM, send Python through USB raw REPL, and assert the guest's results. They are `external_*`/ignored because the binary inputs are not distributed with the repository. Set `C61_ROM_ELF` and `COREMATRIX_FIRMWARE` to the pinned local files, and `C61_TEST_HTTP_BIND` to a host IPv4 interface reachable by the guest NAT (not loopback). Run:

```sh
cargo test --release -p esp32c61 --test firmware_wifi -- --ignored --skip external_official_uiflow_wpa2_tls --nocapture --test-threads=1
```

Each test discovers `c61-test` on channel 6, receives `10.0.2.15` by DHCP, resolves `c61.test` through the local virtual DNS, and validates the 48-byte NTP response. A local TCP server then checks all 4096 request body bytes; the guest checks all 8192 response body bytes. Open and WPA2 cases both recorded 4145 bytes from the guest and 8252 bytes from the host, with one successful TCP connection; both reconnect after WLAN deactivation/reactivation. SoftAP tests then enable `c61-softap` while STA remains active, exchange actual management frames, verify the guest's ARP reply for `192.168.4.1`, deliver a 14-byte UDP payload to the guest socket, and compare its 24-byte response exactly. The SoftAP peer has a fixed test IPv4 address; guest DHCP-server leases are not covered. Firmware-origin mDNS traffic is outside this fixture and can increment unrelated NAT UDP send errors. The official startup prints an independent `ImportError: no module named 'M5Things'` before the MicroPython prompt; the tests run afterward and do not suppress or repair that firmware issue. WPA3 and physical hardware remain unverified at this checkpoint.

The initial SoftAP UDP byte comparison exposed four trailing bytes from the guest's FCS reservation in the relayed Ethernet payload. The MAC now removes that reservation before giving the frame to the protocol AP/client; it does not forward uninitialised FCS bytes as application data. Both STA and SoftAP firmware cases pass afterward. A separate wrong-password test first establishes the working WPA2 connection, reconnects with an incorrect password, verifies that a new four-way handshake was actually attempted, and requires the guest to remain disconnected and the peer not to reach `Installed`.

## TLS qualification and the pinned image’s time bug

The TLS fixture uses caller-provided `C61_TEST_TLS_ADDRESS` (IPv4:port), `C61_TEST_TLS_CA_DER` and `C61_TEST_TLS_EPOCH` (Unix seconds inside the test certificate validity), with the server name `corematrix.test`. OpenSSL 3.6.4 serves TLS 1.2 `ECDHE-ECDSA-AES128-GCM-SHA256`, using a P-256 self-signed test certificate with `DNS:corematrix.test` SAN. The guest trusts only that supplied certificate and sets `ssl.CERT_REQUIRED`. It completes both Finished messages and checks the decrypted 4096-byte response against `bytes(range(256))*16`. A second connection with `wrong.invalid` must fail hostname verification.

UIFlow2 2.5.4 has an upstream time-conversion bug: a correct 2026 guest RTC is interpreted by TLS as 2056, so a current certificate is rejected as expired. [Official UIFlow issue #120](https://github.com/m5stack/uiflow-micropython/issues/120) records the manufacturer’s diagnosis and planned 2.5.5 fix. The fixture first reproduces the expiry rejection, then subtracts the 1970-to-2000 offset (946684800 seconds) **only from this test guest’s RTC** and creates a fresh SSL context. Certificate validity, signature and hostname checks remain enabled. It restores the correct guest RTC afterward. This is an explicit firmware-specific qualification step, not an emulator clock change or a claim that ordinary HTTPS works without the upstream fix.

To reproduce, generate a temporary certificate/key, run the fixture in one terminal and the test in another. Supply an interface address reachable by guest NAT and an unused port; the helper neither selects an external service nor creates credentials automatically:

```sh
openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:P-256 -noenc -days 2 \
  -keyout /tmp/c61-test-key.pem -out /tmp/c61-test-cert.pem \
  -subj /CN=corematrix.test -addext subjectAltName=DNS:corematrix.test
openssl x509 -in /tmp/c61-test-cert.pem -outform DER -out /tmp/c61-test-cert.der
python3 tools/fixtures/corematrix-wifi/tls-server.py \
  --bind "$C61_TEST_HTTP_BIND" --port "$C61_TEST_TLS_PORT" \
  --cert /tmp/c61-test-cert.pem --key /tmp/c61-test-key.pem
```

```sh
C61_TEST_TLS_ADDRESS="$C61_TEST_HTTP_BIND:$C61_TEST_TLS_PORT" \
C61_TEST_TLS_CA_DER=/tmp/c61-test-cert.der C61_TEST_TLS_EPOCH="$(date +%s)" \
cargo test --release -p esp32c61 --test firmware_wifi \
  -- --ignored --nocapture --test-threads=1
```

Keep the ROM, image and HTTP-bind variables exported as above. Stop the helper with Ctrl-C; it removes its temporary HTTP response directory. Never commit the generated key. The helper serves a fixed Content-Length response with `openssl s_server -HTTP -http_server_binmode`.

The first TLS attempt exposed missing C61 ECC at guest PC `420ff224` / MMIO `6008b00c`. The independent C61 ECC implementation and integration (`56541d89`, `5d67a6ae`) enabled actual P-256 certificate-point checks and ECDH. Eleven real handshake operations were independently compared with OpenSSL-backed curve arithmetic: seven point verifications and four scalar multiplications, including the final shared secret, matched. Private operands/traces are not retained.

The next handshake failed when the server rejected the client Finished with `bad_record_mac`. Its SHA DMA chain contained a 64-byte SRAM block plus 384 bytes at `42340e97`, which the MMU mapped to PSRAM; assuming external RAM always starts at `43000000` lost the latter data. `fac00a34` adds bounded MMU-aware SHA DMA reads and external descriptor owner updates. The corrected handshake and HTTPS body pass. Independent [crypto known-answer evidence](platform/evidence/corematrix-crypto-20261106/README.md) also records 109 passing hash/HMAC/PRF/AES checks after this fix and the prior 8192-byte SHA-256 mismatch. Its Python-composed HMAC/PRF checks are not direct mbedTLS entry-point tests.

An exploratory OpenSSL `-www` response decrypted 1805 HTTP bytes, then an extra read ended in guest `OSError: -116` instead of EOF despite a server close-notify. The qualified response uses the HTTP Content-Length and does not reinterpret that timeout as EOF; general TLS close-notify/EOF behavior remains unqualified. A Python stdlib SSL server variant timed out during the corrected-time handshake while the pinned OpenSSL command completed; no Python-server interoperability claim is made.

An initial HTTP fixture failed because macOS accepted sockets inherited nonblocking mode; the host closed after its first read returned `WouldBlock`. Explicitly setting the accepted socket to blocking mode fixed the fixture, after which both firmware cases passed without changing the MAC/DMA model. This negative result did not establish a descriptor-chain bug. No CPU performance claim is made here; chip integration benchmarks are separate.

To reproduce the source comparison, download the exact URLs in the receipt into a scratch directory, verify each SHA-256, and run `riscv32-esp-elf-objdump -dr libpp.a` and `... libphy.a`. The receipt's `patternHex` is the little-endian instruction range at `symbolOffset`; search the pinned image for those bytes and compare all listed offsets (including empty match lists). The ROM and live firmware startup must then be tested separately.
