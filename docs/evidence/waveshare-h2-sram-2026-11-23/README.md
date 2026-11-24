# Independent ESP32-H2 SRAM runtime

Two exact Waveshare boards now have a native, bounded H2 runtime: `waveshare-h2-zero` and `waveshare-h2-dev-kit-n4`. Both have 4 MiB flash and no PSRAM. This is an independent `esp32h2` crate, with no dependency on the C6 SoC or C6 ROM. The firmware examples in the manufacturer repository remain references; none is marked execution-accepted.

## What actually ran

`native-execution.ndjson` records two separate `esp_soc::Machine<H2>` runs of the source-visible program in `esp32h2/tests/common/mod.rs`. Each run retires 10,000 instructions with zero exceptions, prints `H2 SRAM chip16 RGB8` through UART0, transfers one GRB frame on the physical GPIO8 RMT route, reads released BOOT GPIO9, and writes an LP SRAM marker. The LED result is RGB `[52,18,86]`. `native-tests.log` also verifies held BOOT across host reset/reload, and proves that routing the same waveform to GPIO10 produces no onboard LED update.

The 1,168-byte `h2-sram-digital-probe.bin` is newly authored diagnostic code, not a manufacturer image or default catalogue firmware. SHA-256: `aac919b8971d0ce681b1d53b1a064186c58686f10c889001d77e15cde79765d0`. It has chip ID 16, entry `0x40800000`, two SRAM segments, XOR checksum and appended SHA-256. It can be staged at flash offset 0 and booted with `boot_app(0)`; the common host `LoadKind::App` contract may instead stage it at `0x10000` and call `boot_app(0x10000)`. These are host copies into SRAM; neither path executes a bootloader.

## Primary evidence and exact map

Original source bytes and HTTP receipts are retained in `docs/platform/research/waveshare-h2-runtime-sources/manifest.json` (29 receipts). ESP-IDF is fixed at tag v5.5.1, commit `fcae32885b0296b32044cb99ecbdc50d98dddb83`. The retained H2 TRM is version 1.2 and the datasheet is version 1.3.

| Implemented region | H2 address or constraint | Primary source |
| --- | --- | --- |
| HP SRAM | `0x40800000..0x40850000`, 320 KiB | [H2 soc.h](https://github.com/espressif/esp-idf/blob/fcae32885b0296b32044cb99ecbdc50d98dddb83/components/soc/esp32h2/include/soc/soc.h) |
| LP SRAM | `0x50000000..0x50001000`, 4 KiB | Same source endpoints and [H2 datasheet](https://www.espressif.com/sites/default/files/documentation/esp32-h2_datasheet_en.pdf); a stale source comment says 16 KiB and is not used |
| UART0/1 | `0x60000000`, `0x60001000` | [H2 reg_base.h](https://github.com/espressif/esp-idf/blob/fcae32885b0296b32044cb99ecbdc50d98dddb83/components/soc/esp32h2/register/soc/reg_base.h), [uart_reg.h](https://github.com/espressif/esp-idf/blob/fcae32885b0296b32044cb99ecbdc50d98dddb83/components/soc/esp32h2/register/soc/uart_reg.h) |
| RMT | `0x60007000`; 2 finite TX channels, 4 blocks × 48 symbols | [H2 rmt_reg.h](https://github.com/espressif/esp-idf/blob/fcae32885b0296b32044cb99ecbdc50d98dddb83/components/soc/esp32h2/register/soc/rmt_reg.h) |
| GPIO and IO MUX | `0x60091000`, `0x60090000`; 19 physical pads | [H2 TRM §6.17.2](https://www.espressif.com/sites/default/files/documentation/esp32-h2_technical_reference_manual_en.pdf), H2 gpio_reg.h/io_mux_reg.h retained in manifest |
| PCR | `0x60096000`; UART and RMT gate/reset subset | [H2 pcr_reg.h](https://github.com/espressif/esp-idf/blob/fcae32885b0296b32044cb99ecbdc50d98dddb83/components/soc/esp32h2/register/soc/pcr_reg.h), fixed H2 uart_ll.h/rmt_ll.h |
| RMT source clock | Board crystal 32 MHz; channel DIV=0 means 256 | [H2 rmt_ll.h](https://github.com/espressif/esp-idf/blob/fcae32885b0296b32044cb99ecbdc50d98dddb83/components/hal/esp32h2/include/hal/rmt_ll.h), retained TRM |

The GPIO register bank has 28 positions, while TRM §6.17.2 exposes only GPIO0–5, GPIO8–14 and GPIO22–27. The nine unavailable positions do not become host-controllable pads. The general IDF valid-mask macro covers register positions; it is not used to invent additional package pins.

RMT follows the actual HAL selection: XTAL is source selector 0 and RC_FAST is selector 1. Some generated PCR field comments mention other clocks or reverse fractional A/B descriptions; the HAL explicitly writes A=numerator, B=denominator and defines `1 + DIV_NUM + A/B`. Tests verify this formula and the retained fractional remainder. RC_FAST, carrier, RX, DMA, wrap, loops and private FSM status are rejected. The model does not fabricate the undocumented numeric FSM state to satisfy a busy-poll.

## Image and revision contract

The public parser is `esp32h2::image::parse(&[u8]) -> Result<esp_soc::image::AppImage, String>`. It validates chip ID 16, 1–16 nonempty aligned SRAM segments, physical SRAM boundaries, no overlaps, an aligned entry inside a loaded segment, XOR/padding and optional SHA-256, and erased trailing bytes. Validation completes before copying any guest SRAM.

The board flash storage remains exactly 4 MiB. A source image may declare 1, 2 or 4 MiB, which fits that hardware; larger or unknown capacity codes are rejected. This distinction is necessary because real H2-Zero examples declare 2 MiB despite the board's 4 MiB hardware. Requests to configure the machine for a different physical flash capacity fail at load/boot. Nonzero PSRAM is rejected.

Neither Waveshare board source establishes a wafer revision. The loader accepts only unrestricted full revision fields (`min=0`, `max=0` or `65535`). The deprecated byte14 is ignored, matching [ESP-IDF bootloader_common_loader.c](https://github.com/espressif/esp-idf/blob/fcae32885b0296b32044cb99ecbdc50d98dddb83/components/bootloader_support/src/bootloader_common_loader.c) and [esp_app_format.h](https://github.com/espressif/esp-idf/blob/fcae32885b0296b32044cb99ecbdc50d98dddb83/components/bootloader_support/include/esp_app_format.h). No fabricated eFuse revision is returned.

`original-image-audit.json` records all 18 original example images from fixed Waveshare commit `7ca9339cde5ac0fa6a2040f0d303b4d92e268d30`, including complete header/segment identities, checksums, partition MD5 and the actual native loader's rejection. Empty OTA partitions are recorded as partition declarations, not invented executable images.

Representative `firmware/ESP-IDF/02_blink_rgb_merged.bin` is 232,832 bytes, SHA-256 `acc05e4b1ff4321deac80e5bebceab5db62e0a8cbd1bb5d68beed133218932d9`. Its bootloader entry is `0x4083c2da`; the app partition starts at `0x10000` with entry `0x40800336`. Both headers identify chip16, declare 2 MiB and limit the maximum full revision to 199 (v1.99). The app also requires flash-mapped segments at `0x42018020` and `0x42000020`. The bytes remain unchanged. The first native rejection is the unverified wafer constraint; the independent segment audit additionally establishes the unimplemented flash-cache dependency.

## Integration API

- `esp32h2::{H2, H2Bus, Machine, machine(mac, flash_size)}`.
- `board::make_board("waveshare-h2-zero" | "waveshare-h2-dev-kit-n4")` returns the exact board; `H2Bus::attach_board(board)` applies its source-backed input defaults.
- `H2Bus::load_ram(address, bytes, memory_size)` validates a raw ELF/RAM segment. It is not an ESP-image parser. `SocBus::write_flash` and `boot_app` stage and validate the complete ESP app image.
- `SocBus::gpio_set_input`, `gpio_release_input`, `uart_input`, `console_take`, `board_ref` and `request_reset` use the usual host interfaces. USB input is not redirected to UART.
- `H2::ROM_BOOT_DEFAULT=false`, `H2::ROM_ELF=""`. ROM loading/execution must be explicitly rejected by frontends. Reset reinitializes this bounded peripheral subset and recopies the selected app, preserving the externally held BOOT level. It does not imply ROM startup.

## Boundaries

Execution uses the existing RV32IMAC CPU at a nominal 96 MHz schedule. This is not instruction or power timing calibration. UART is a logical 128-byte receive FIFO with immediate host TX completion, restricted to 8N1; physical baud/USB/bridge waveforms and receive timeouts are unimplemented. RMT models finite symbol duration and digital GRB output, not electrical calibration or every controller feature. GPIO uses ideal digital drives/pulls; contention and analog pad behavior are unmodelled. Interrupt routing, eFuse, cache/MMU, mask ROM, Bluetooth, IEEE802.15.4, USB and full original applications remain unavailable. ESP32-H2 has no Wi-Fi.

Native tests and clippy with `-D warnings` pass. Browser/CLI/WASM/public parser integration requires separate frontend checks; these native receipts do not claim browser acceptance or physical hardware testing.
