# Seeed 7.5-inch UC8179 board runtimes

These are two separate physical board profiles. They share the monochrome
UC8179 controller implementation, while selecting the correct MCU, GPIO routes,
power gate, battery input and buttons. Neither is an alias for an existing board.

| Product / board ID | MCU and physical memory | Screen | SPI SCK / MOSI / CS / D-C / reset / BUSY |
| --- | --- | --- | --- |
| `seeed-trmnl-75-diy` | XIAO ESP32-S3 Plus, 16 MiB flash / 8 MiB PSRAM | 800 × 480 monochrome UC8179 | GPIO 7 / 9 / 44 / 10 / 38 / 4 |
| `seeed-xiao-epaper-75` | XIAO ESP32-C3, 4 MiB flash / no PSRAM | 800 × 480 monochrome UC8179 | GPIO 8 / 10 / 3 / 5 / 2 / 4 |

The kit uses the EE04 display board. The C3 panel uses the XIAO ePaper Driver
Board; the separately sold Breakout Board has a different BUSY connection and
is not substituted here. The kit's host control IDs are `button-d1`,
`button-d2`, `button-d4` on GPIO 2, 3, 5. Its XIAO BOOT is GPIO 0. The C3
profile exposes its actual BOOT GPIO 9; it has no invented three-button keypad.
All these button inputs are active low. Neither panel has touch input.

## Transport and controller behavior

`esp-soc/src/board/seeed_xiao_epaper.rs` opts in to routed GPIO and SPI
transactions. Clock, MOSI and chip select must reach the physical pins, and D/C
must be a routed, enabled GPIO output. A wrong MCU's matrix signal number does
not drive the panel. The connection is write-only: reads return `0xff`.

The controller accepts source-backed 800 × 480 resolution setup, old/new RAM
planes, power-on, refresh, partial windows and deep-sleep commands. A write
alone does not change the published image. Refresh raises active-low BUSY and
publishes the new plane at its scheduled completion. Its 1 ms power-on and
20 ms refresh intervals are deterministic digital model timings, not optical
waveform or hardware latency measurements. Controller reset preserves the
written RAM and visible e-ink image; panel supply loss invalidates RAM while
retaining the bistable visible image.

The reset/RAM distinction matters for the unmodified TRMNL release. Its pinned
`bb_epaper` driver writes plane 0 using command `0x13`, then
`bbepRefresh(REFRESH_FULL)` runs `epd75_init_sequence_full`, whose first item is
`EPD_RESET`, before sending refresh `0x12`. Clearing RAM on that reset erased
every image before refresh. Retaining RAM is derived from this actual driver
sequence; this is not a claim of silicon-level characterization of every
controller power state.

### Kit power and nominal battery

The official rev. 1.0 schematic, sheets 4 and 5, connects `PWR_EN` to XIAO
`U0TXD` (GPIO 43), through U16 TPS22916, with R26 = 100 kΩ to ground. A high
software GPIO enables panel power; an undriven GPIO with the GPIO mux selected
does not. The native UART0 TX route, or noninverted matrix UART0 TX route, is
modeled at its idle-high level. The stock release leaves GPIO 43 on native
UART0 TX: an actual run read its IO_MUX as `0x100`, function 0. Requiring
software GPIO on this net incorrectly disconnected a real UART-powered panel.
UART data-bit supply ripple, break/inversion control, transistor dynamics and
analog brownout are outside this steady-state model.

The same schematic shows ADC_EN GPIO 6 driving U17 TPS22916, R27 = 100 kΩ
pulldown, and equal R28/R29 = 10 kΩ forming the BAT_ADC GPIO 1 divider.
The board provides a nominal 4.0 V battery as a 2.0 V ADC source only while
GPIO 6 is actually driven high. Disabled or unrouted ADC_EN gives 0 V. The
existing `default_analog_inputs` attachment preserves explicit host voltage,
waveform and raw-count inputs. This is a gated nominal source, not a charger,
discharge, USB detection or battery-life model. The C3 board does not inherit
this S3 battery circuit.

### C3 route callback correction

The S3 bus already invokes `gpio_waveform_at` when `uses_gpio_routes()` is
enabled. The C3 bus still checked the older `uses_gpio_waveform()` predicate
at board attachment and after GPIO/IO_MUX writes. Its two existing checks now
use `uses_gpio_routes()`. No per-tick callback or instruction-sized waveform
scheduling is added. The existing default of `uses_gpio_routes()` delegates to
`uses_gpio_waveform()`, preserving older waveform boards' behavior. A real C3
MMIO test failed before this change and passes afterwards.

Web app-image validation, ROM selection and local build-directory assembly
share `isC3Board()` so the C3 panel cannot accidentally load an S3 ROM or accept
an S3 app. The WASM and CLI factories use their existing per-chip board lookup.

## Pinned official TRMNL release

TRMNL firmware v1.8.19 was published on 2026-11-13 from
`usetrmnl/trmnl-firmware` commit
`450f9ca2c690ef8bf7d9f5a219b71fd49301325d`. Its public release workflow gives
the official distribution bucket and per-board paths. Both downloaded images
are complete merged images for offset zero, with bootloader at `0x0000`,
partition table at `0x8000` and app at `0x10000`. They are not app-only images.

| Board | Official merged binary | Bytes | SHA-256 |
| --- | --- | ---: | --- |
| S3 kit | [FW1.8.19.bin](https://trmnl-fw.s3.us-east-2.amazonaws.com/byod/seeed_OG_DIY_Kit/flash/FW1.8.19.bin) | 1,540,080 | `342acbafe8d7e74a2cd77bba82ea0d09ded2c068ef8386c57e82cec42db45727` |
| C3 panel | [FW1.8.19.bin](https://trmnl-fw.s3.us-east-2.amazonaws.com/byod/seeed_xiao_esp32c3/flash/FW1.8.19.bin) | 1,554,304 | `907beb4253c5fe933da84d09c3c3444af7c86b1e52910d2950269ff4a8c2ec82` |

The C3 image descriptor and boot console identify `trmnl-firmware`,
`v1.8.19`, ESP-IDF 4.4.7. The S3 Arduino image descriptor contains its bundled
library-builder metadata (`arduino-lib-builder`, IDF v4.4.7-dirty, March 2024),
so the release URL plus exact content hash identify that release; the bundled
library build date must not be presented as the product release date.

The S3 release is a current firmware for a product documented as shipping
with TRMNL, not a recovered byte-identical unit's factory image. The C3 product
originally ships a three-page display demo. Its TRMNL release is a compatible
third-party ecosystem firmware, not that factory demo. The merged binaries do
not carry an initial SPIFFS image; the release formats its empty partition at
first boot.

## Verification and remaining limits

The compact receipt in `evidence/seeed-75-20261122/validation.json` records exact
input hashes, tested commands, screenshots' pixel statistics, outcomes and
negative attempts. Third-party binaries, raw logs and PNGs are local probe
artifacts rather than committed evidence.

The S3 kit's unchanged v1.8.19 image reaches its Wi-Fi provisioning screen
after a genuine ROM boot with the default battery source. At 10 virtual seconds
it has completed two refreshes, accepted 192,000 RAM bytes and produced an
800 × 480 image containing 8,639 black pixels. Visual review reads
`TRMNL firmware 1.8.19 (450f9ca)` and the connection instructions/QR code.
The receipt retains hashes and pixel counts; the generated network identifier
and QR payload are not retained. This proves the first-boot provisioning
display, not an authenticated cloud dashboard or an actual network connection.

```sh
cargo test -p esp-soc seeed_xiao_epaper --lib
cargo test -p esp32c3 --test seeed_xiao_epaper
cargo test -p esp32c3 --test pin_transport
node --test tools/platform-app-image-test.mjs tools/platform-local-firmware-test.mjs
cargo clippy -p esp-soc -p esp32c3 -p esp32s3 --all-targets -- -D warnings
```

The board tests cover both physical SPI maps, power/reset gates, scheduled
publication, incorrect-pin rejection, the UART-connected supply, gated battery
and host overrides. The C3 integration test performs actual IO_MUX, GPIO matrix
and GP-SPI MMIO writes, reads BUSY through GPIO_IN, observes black/white pixels,
checks wrong S3 signal rejection, and drives the real BOOT input through the
host GPIO API. The pre-existing C3 pin-transport suite passes 9 tests; its
external firmware test is intentionally not run without that fixture.

Native reproduction, using the unchanged files linked above and Espressif ROM
ELFs obtained by `tools/fetch-rom-elfs.sh`:

```sh
target/debug/esp32sim --chip s3 --board seeed-trmnl-75-diy \
  --rom "$ROM_DIR/esp32s3_rev0_rom.elf" --boot rom \
  --flash-image "$FIRMWARE_DIR/s3-FW1.8.19.bin" --flash-mb 16 --psram-mb 8 \
  --max-seconds 10 --console uart0 --no-dump --adc 1 --tft-png target/trmnl-s3.png

target/debug/esp32sim --chip c3 --board seeed-xiao-epaper-75 \
  --rom "$ROM_DIR/esp32c3_rev3_rom.elf" --boot rom \
  --flash-image "$FIRMWARE_DIR/c3-FW1.8.19.bin" --flash-mb 4 \
  --max-seconds 10 --console uart0 --no-dump --tft-png target/trmnl-c3.png
```

The C3 release currently reaches its real app and SPI initialization, then
resets at 8.749 virtual seconds with `RTCWDT_RTC_RESET`; no nonwhite frame was
verified. This is a blocked full-firmware path, not a successful UI run.
The release enables light sleep while waiting for display updates; the C3
backend currently has no light-sleep state machine, unlike the S3 backend.
That missing behavior is the identified scope limitation, rather than a reason
to bypass the ROM, change the binary or fake a completed screen. Network
provisioning, authenticated cloud content, long-duration sleep/wake, panel
temperature/LUT calibration and every physical button's effect in this release
are not verified by the board transport tests.

Native probes used the debug CLI for functional evidence. They are not speed
benchmarks. The C3 bus change needs the repository's separate same-profile,
alternating release hello median CPU comparison against main; mixed
debug/release elapsed times must not be used as that comparison.

## Primary sources

- [TRMNL kit Arduino cookbook](https://wiki.seeedstudio.com/ogdiy_kit_works_with_arduino/)
  and [ESPHome wiring](https://wiki.seeedstudio.com/ogdiy_kit_works_with_esphome/).
- [C3 panel Arduino cookbook](https://wiki.seeedstudio.com/xiao_075inch_epaper_panel_arduino/)
  and [ESPHome wiring](https://wiki.seeedstudio.com/xiao_075inch_epaper_panel_esphome/).
- [Official kit schematic](https://files.seeedstudio.com/wiki/XIAO_Gadget/TRMNL_Kit_Pic/XIAO_ePaper_driver_board_sch.pdf),
  rev. 1.0, sheets 4/5; 841,866 bytes, SHA-256
  `288a9b9372360e898ac9b702808892cbb0af97878e0d12d0db9a55a6eeeb43e8`.
- [Seeed_GFX2 XIAO ePaper board definitions](https://github.com/Seeed-Studio/Seeed_GFX2/blob/556df7cb0b4a097d9b81026a26fd5511f9c934d6/src/board/configs/XIAO_ePaper_Board_Configs.h).
- [TRMNL v1.8.19 board definitions](https://github.com/usetrmnl/trmnl-firmware/blob/450f9ca2c690ef8bf7d9f5a219b71fd49301325d/src/board_config.cpp),
  [battery ADC](https://github.com/usetrmnl/trmnl-firmware/blob/450f9ca2c690ef8bf7d9f5a219b71fd49301325d/src/battery/adc_battery.cpp),
  [display calls](https://github.com/usetrmnl/trmnl-firmware/blob/450f9ca2c690ef8bf7d9f5a219b71fd49301325d/src/display.cpp),
  [release publisher](https://github.com/usetrmnl/trmnl-firmware/blob/450f9ca2c690ef8bf7d9f5a219b71fd49301325d/.github/workflows/publish-release-firmware.yml).
- [Pinned bb_epaper controller sequences](https://github.com/bitbank2/bb_epaper/blob/8e26f00c1f0ce6be072f22b7945bc5786c4c83e7/src/bb_ep.inl),
  especially `epd75_init_sequence_full`, `bbepRefresh`, `bbepWritePlane`,
  `bbepWakeUp` and `bbepWaitBusy`.
- ESP-IDF [v4.4.7 S3 GPIO signal map](https://github.com/espressif/esp-idf/blob/v4.4.7/components/soc/esp32s3/include/soc/gpio_sig_map.h)
  and [S3 IO_MUX](https://github.com/espressif/esp-idf/blob/v4.4.7/components/soc/esp32s3/include/soc/io_mux_reg.h);
  [C3 GPIO signals](https://github.com/espressif/esp-idf/blob/v4.4.7/components/soc/esp32c3/include/soc/gpio_sig_map.h),
  [GPIO registers](https://github.com/espressif/esp-idf/blob/v4.4.7/components/soc/esp32c3/include/soc/gpio_reg.h)
  and [SPI registers](https://github.com/espressif/esp-idf/blob/v4.4.7/components/soc/esp32c3/include/soc/spi_reg.h).
