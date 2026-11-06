# ESP32-C61 Wi-Fi digital model

The independent C61 model implements the C61 MAC v3 register interface, bounded HP SRAM TX/RX descriptors, and the PHY handshakes exercised by its pinned libraries. It uses the existing protocol-level virtual AP/network. This is implementation evidence, not a claim of complete firmware compatibility or physical RF accuracy. No C61 scan, association, DHCP or network application result has yet been recorded in this checkpoint.

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
| TX DMA | descriptor size/length 14 bits, owner bit 31; 8-byte packet header | frame length in packet word 0 bits 13:0 |
| RX DMA | 64-byte control header with no optional dumps, then frame/FCS | C6 uses 92 bytes; `get_sublen_offset` returns 56 before the 8-byte length/state words |
| RX metadata | RSSI byte 0, address-match bits 28/29, group byte 11 bit 7, timestamp bytes 12–15, noise byte 20, length/dump bytes 56–59 | Firmware writes channel byte 28; zero CSI/SIG-B data |

Calibration samples are ideal zeros, completion is synchronous, and the power detector reports an idle state. These choices allow the guest's calibration code to run; they do not simulate radio amplitude, channel noise, acquisition delays, antenna behavior, interference or transmit power. Wi-Fi power-save/TSF, HE aggregation and multiple peer behavior remain unqualified. Pending TX work is bounded to one pointer per hardware queue. Out-of-range SRAM accesses neither read host memory nor panic. The RX hand-over policy retains the shared-model correction described by EX238, but has not been measured on C61 hardware.

Integration hooks are `ModemBb` at peripheral block `0xa0`, `WifiMac` at `0xa4` (MAC IRQ 0), and optional block `0xa5` register alias with delta `0x1000`. After MMIO/ticks, only call `WifiMac::service(sram, guest_us)` when `has_pending_work()` is true. `StationLink` supplies AP attachment, Ethernet relay, reboot-surviving host network state and reports. Frontend network availability must follow firmware-driven qualification, not the existence of these hooks.

Validation at this checkpoint: `cargo test -p esp32c61 --test wifi` passes three focused tests for C61-specific PHY commands, RX metadata and descriptor ownership, event clearing, and malformed DMA bounds. Clippy currently stops on two pre-existing C61 scaffold lints in `bus.rs` and `soc.rs`; those files belong to the concurrent chip integration lane. No performance claim is made; no shared CPU or other chip's per-tick path changes here.

To reproduce the source comparison, download the exact URLs in the receipt into a scratch directory, verify each SHA-256, and run `riscv32-esp-elf-objdump -dr libpp.a` and `... libphy.a`. The receipt's `patternHex` is the little-endian instruction range at `symbolOffset`; search the pinned image for those bytes and compare all listed offsets (including empty match lists). The ROM and live firmware startup must then be tested separately.
