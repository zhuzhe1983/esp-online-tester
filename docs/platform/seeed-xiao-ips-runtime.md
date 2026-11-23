# Seeed XIAO ESP32-S3 IPS display runtime

The three distinct execution boards are `seeed-xiao-ips-147`,
`seeed-xiao-ips-114` and `seeed-xiao-ips-096`. They execute unmodified ESP32-S3
firmware with 16 MiB flash and 8 MiB PSRAM. They are **partial** USB-powered
digital board models: routed LCD writes, physical user buttons, GPIO/LEDC
backlight, and the 1.47-inch panel's AXS5106L single-contact I2C interface.

No IMU, PDM microphone transducer, battery/charger, SD transport, deep sleep or
wake circuitry is modeled by these boards. A firmware's `[MIC] OK` line only
means its I2S driver initialized; it is not proof of microphone input. The
default zero ADC input is not a battery reading. An SD card prepared by the
platform does not make the 1.47-inch board's guest SD driver functional.

## Source-backed wiring

The [official board configurations](https://github.com/Seeed-Studio/Seeed_GFX2/blob/556df7cb0b4a097d9b81026a26fd5511f9c934d6/src/board/configs/XIAO_LCD_Board_Configs.h),
[panel configurations](https://github.com/Seeed-Studio/Seeed_GFX2/blob/556df7cb0b4a097d9b81026a26fd5511f9c934d6/src/panel/configs/Seeed_Panel_Configs.h),
the [XIAO ESP32-S3 Plus variant](https://github.com/espressif/arduino-esp32/blob/master/variants/XIAO_ESP32S3_Plus/pins_arduino.h)
(observed Git blob `ba7d1a5069dba59cfd8d9031c3e6d3a72bd4bc13`), and
[official factory source](https://github.com/Seeed-Projects/Display-Gadgets/tree/ba7f69f223fb90b1766175afee4d26a045663a97/code/example)
give the following routes. The factory binaries print the same numeric GPIOs.

| Signal | 1.47-inch touch | 1.14-inch | 0.96-inch |
|---|---|---|---|
| Display/controller | 172 × 320, JD9853A compatible DCS | 135 × 240, ST7789 | 80 × 160, ST7789 |
| LCD SCK / MOSI / CS | GPIO7 / GPIO9 / GPIO3 | same | same |
| LCD D/C / RESET / backlight | GPIO4 / GPIO13 / GPIO12 | same | same |
| User button 1 / 2 / 3, active low | GPIO11 / GPIO42 / absent | GPIO43 / GPIO44 / GPIO11 | GPIO43 / GPIO44 / absent |
| Canonical platform controls | `usr1`, `usr2` | `button-a`, `button-b`, `button-c` | `usr1`, `usr2` |
| I2C SDA / SCL | GPIO5 / GPIO6 | GPIO5 / GPIO6 Grove port | GPIO5 / GPIO6 |
| Touch | AXS5106L, 7-bit address 0x63, IRQ GPIO44 | absent | absent |
| SD interface (not modeled) | CS43, MISO8; shares SCK7/MOSI9 | absent | absent |

The board registry is shared by native CLI and WASM. Platform chip selection
uses ESP32-S3 chip ID 9 and its ROM for these IDs; no compatibility alias to
another product is involved. `btnA`/`btnB`/`btnC` and `usr1`/`usr2`/`usr3` are
also accepted by CLI input scripts where the corresponding physical key exists.

## LCD contract and boundaries

Both hardware SPI and actual GPIO software SPI feed the shared `DcsPanel`
command decoder. The 1.47-inch official factory firmware uses software SPI;
the others use hardware SPI. GPIO matrix/IO_MUX routing, asserted CS, a driven
D/C pin and deasserted RESET are checked. Moving MOSI to the wrong route cannot
paint pixels. There is no LCD MISO connection, so receive bytes are 0xff.
Establishing a route while SCK is already high does not create a false clock
edge, and losing a software SPI route discards an incomplete byte.

The implemented subset includes RGB565 RAM writes, column/row windows, MADCTL
orientation, inversion, sleep/display enable and reset. Seeed explicitly
implements JD9853A through a
[ST7789-compatible initialization](https://github.com/Seeed-Studio/Seeed_GFX2/blob/556df7cb0b4a097d9b81026a26fd5511f9c934d6/src/driver/tft/Driver_JD9853A.cpp);
this is the basis for sharing the DCS decoder. Vendor analog, gamma, scan and
electrical timing registers are not modeled. Display frames are guest-written
pixels, scaled by GPIO12 or the existing S3 LEDC duty model.

The native glass profiles are:

| Board | Native MADCTL / inversion | Logical window in controller coordinates |
|---|---|---|
| 1.47 | 0x48 (MX+BGR), inversion off | column offset 34, row offset 0 |
| 1.14 | 0x00 (RGB), inversion on | column offset 52, row offset 40 |
| 0.96 | rotation 2, 0xc8 (MX+MY+BGR), inversion off | column offset 24, row offset 0 |

The 0.96-inch factory binary uses MADCTL 0xc0 and pre-swaps its UI colors. The
newer source's `V_RED = C_BLUE` and other aliases explain why those colors still
render correctly on the BGR glass. The runtime applies the physical color
order rather than changing the firmware palette.

**0.96-inch orientation limitation:** the official
[ST7789 driver](https://github.com/Seeed-Studio/Seeed_GFX2/blob/556df7cb0b4a097d9b81026a26fd5511f9c934d6/src/driver/tft/Driver_ST7789.cpp)
gives the same 24/0 offsets to portrait rotations 0 and 2. Those addresses do
not select the same physical window in a full 240 × 320 ST7789 GRAM. Until the
glass/controller addressing is documented, this board keeps the standard
240 × 320 decoder and the official rotation-2 visible window. Other 0.96-inch
scan layouts are not verified. It does not invent a 128 × 160 controller RAM
to conceal the contradictory source offsets.

## AXS5106L touch and shared reset

The [official touch driver](https://github.com/Seeed-Studio/Seeed_GFX2/blob/556df7cb0b4a097d9b81026a26fd5511f9c934d6/src/touch/Touch_AXS5106L.cpp)
and the [factory BSP](https://github.com/Seeed-Projects/Display-Gadgets/blob/ba7f69f223fb90b1766175afee4d26a045663a97/code/example/147_ESP32/0703_DashBoard_147_ESP32/esp_lcd_touch_axs5106l/src/esp_lcd_touch_axs5106l.cpp)
read 14 bytes from register 0x01: byte 1 is contact count, bytes 2–5 contain
12-bit X/Y. The host supplies physical screen coordinates; the controller
reports raw X = 171 − screen X, raw Y = screen Y. Firmware performs the mirror
itself. Register contents are never replaced with already transformed UI
coordinates. Only one contact is implemented. Unsupported register reads
return 0xff; a vendor identity is not fabricated.

GPIO44 receives an active-low notification on each host contact report,
including movement, and returns high on release. Repeated reports retrigger
the falling edge used by the official factory BSP. This is an event-level
IRQ model; scan period, pulse width, contact filtering and autonomous repeat
reports during an unmoving hold are not calibrated. A polling firmware can
read the held contact at any time. The factory BSP briefly retains its last
IRQ result and can display release during an unrefreshed long hold; callers
that need a continuing hold should send continuing contact reports.

The [official 1.47-inch schematic](https://github.com/Seeed-Projects/Display-Gadgets/blob/ba7f69f223fb90b1766175afee4d26a045663a97/resources/schematic/XIAO%201.47%27%27%20IPS%20Display%20%28ESP32-S3%29%20Schematic.pdf)
page 4 shows LCD_RST connected to CTP_RST through R9 (0 Ω), with optional R7
(10 kΩ) and C1 unpopulated. The factory probes 0x63 **before** configuring
GPIO13, then explicitly resets touch and LCD. In this USB-powered digital
profile an undriven reset is treated as deasserted, inferred from that official
startup sequence; this is not a claim that the unpopulated external resistor
exists. Actively driving GPIO13 low blanks/resets the LCD, releases IRQ and
makes touch I2C NACK. Power-up analog behavior remains outside the model.

## Reproduce execution and input checks

The following uses the four flash offsets in each official `README_flash.txt`:
bootloader 0x0, partitions 0x8000, boot_app0 0xe000 and application 0x10000.
The harness verifies the ZIP SHA-256, each member SHA-256 and the ROM SHA-256,
builds an erased 16 MiB flash image, and boots through the actual ROM reset
vector. No application patches or boot stubs are used.

```sh
cargo test -p esp32s3 seeed_xiao_ips --lib
cargo test -p esp32s3 --test seeed_xiao_ips
cargo clippy -p esp32s3 --all-targets -- -D warnings
cargo build --release -p esp32sim --bin esp32sim
python tools/probe-seeed-xiao-ips.py \
  --rom target/roms/esp32s3_rev0_rom.elf \
  --firmware-dir target/seeed-firmware --download \
  --output target/seeed-xiao-ips-probe
```

`tools/fetch-rom-elfs.sh target/roms` fetches and verifies the official ROMs.
The probe additionally requires Python with Pillow. It checks factory boot
markers, nonblank framebuffer, firmware-reported brightness changes, actual
frame dimming and complete backlight blanking, the 1.14-inch third-button
header change, and 1.47-inch touch coordinates/release. Unit tests cover four
distinct corner colors, reset/sleep/display enable, unrouted SPI rejection,
software SPI, and hardware I2C with deliberately swapped pins.

Factory application descriptors identify ESP-IDF
`v5.5.2-729-g87912cd291`. The integration test register offsets and S3 signal IDs
were checked against
[gpio_reg.h](https://github.com/espressif/esp-idf/blob/87912cd291/components/soc/esp32s3/register/soc/gpio_reg.h),
[spi_reg.h](https://github.com/espressif/esp-idf/blob/87912cd291/components/soc/esp32s3/register/soc/spi_reg.h),
[i2c_reg.h](https://github.com/espressif/esp-idf/blob/87912cd291/components/soc/esp32s3/register/soc/i2c_reg.h)
and [gpio_sig_map.h](https://github.com/espressif/esp-idf/blob/87912cd291/components/soc/esp32s3/include/soc/gpio_sig_map.h)
at that firmware revision. They agree with the existing v5.5.1 transport tests.

The official 0.96-inch ZIP's executable identifies itself as v1.13.1, whereas
the current similarly named 0715 source says v1.13.3. The pinned binary hash
and observed executable version take precedence in the run receipt. Source
and binary revision equivalence is not asserted.

Compact receipts live in `docs/platform/evidence/seeed-xiao-ips-20261122/`.
The 2026-11-22 native run passed all 12 scenarios across the three boards.
At the firmware's 75% brightness setting, mean framebuffer RGB was 73.85%,
73.13% and 73.33% of each baseline (0.96 / 1.14 / 1.47); quantization and
changed UI text account for the difference from the nominal duty. Every
backlight-off capture had zero nonblack pixels. The 1.47-inch factory BSP
reported and displayed `(20,301)`, then returned to release. The 1.14-inch
third key changed its header to `Seeed`.

Full console streams, binaries and screenshots are generated only in the
chosen ignored output directory, with their hashes retained in the receipt.
CLI's final board report precedes its lazy PWM snapshot; framebuffer checks
use the refreshed display produced by `--tft-png`, not the cached report duty.
These are native execution checks. Browser/WASM and catalog integration are
separate acceptance checks, and none of these results substitutes for tests
on physical hardware.
