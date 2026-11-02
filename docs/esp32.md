# Classic ESP32

The `esp32` crate models the dual-core LX6 classic ESP32 and loads the ECO3 mask ROM. It boots the real
rev-3 mask ROM through the second-stage bootloader into an unchanged application.
It reports silicon revision v3.0, the ECO3 that ROM belongs to: eFuse CHIP_VER_REV1 and REV2,
and bit 31 of SYSCON_DATE, so an image built with `CONFIG_ESP32_REV_MIN_3` boots.
The shared Xtensa core enables DFP context registers only for this target.
Double-precision arithmetic and PSRAM are not modeled.

## Clock model

Bus time uses a 240 MHz reference. SOC_CLK_SEL and CPUPERIOD_SEL select
40 MHz XTAL or 80/160/240 MHz PLL CPU clocks; the half-reference-tick
remainder survives clock switches. APB uses 40 MHz on XTAL and 80 MHz on PLL,
with a sixth-tick accumulator that preserves phase across source changes.
RC_FAST retains APB:CPU 1:1 (IDF v5.5.4
`components/esp_hw_support/port/esp32/rtc_clk.c:391-404`), but its frequency
and unsupported APLL selections are approximated as 40 MHz rather than
modeling oscillator calibration. Classic UART receive is event-driven and
has no periodic tick callback.

```sh
cargo build --release -p esp32sim
./target/release/esp32sim --chip esp32 --board esp32dev --boot rom \
  --rom web/wasm/fw/esp32_rev300_rom.elf \
  --bootloader web/wasm/fw/public/classic-hello-bootloader.bin \
  --ptable web/wasm/fw/public/classic-hello-ptable.bin \
  --app web/wasm/fw/public/classic-hello_world.bin --max-seconds 3 --no-dump
```

`hello_world_classic` compares the ROM banner, bootloader and application output
and interrupt totals/per-source counts against committed goldens. The ROM comes from Espressif's
ROM ELF package: ESP-IDF's copy is found by itself, otherwise set `ESP32SIM_ROM_DIR` to its absolute directory.
The three firmware binaries were built from the existing CC0 hello_world source
with ESP-IDF 5.5.4, the pioarduino 55.03.38-1 platform and the esp32dev board.
See the [reproducible build recipe](../examples/hello_world-classic/README.md).

The core includes DRAM, IRAM, RTC RAM, flash MMUs, DPORT interrupt routing and
secondary-core control, eFuse, SPI flash, three UARTs, GPIO/IO_MUX, RTC control,
and timer groups including LACT. `none` and `esp32dev` select the bare module.
The default board option also selects the bare module for this chip.
`ttgo-tdisplay` (alias `tdisplay`) is the LilyGO TTGO T-Display, see below.
`ulanzi-tc001` (alias `tc001`) is the Ulanzi TC001 pixel clock, a 32×8 WS2812 matrix on RMT
([boards.md](boards.md#ulanzi-tc001--ulanzi-tc001-pixel-clock-classic-esp32)).
WASM accepts the `esp32` chip identifier. Boot applications through ROM.

T0/T1 and LACT use 64-bit counters and the shared timer stepping logic.
LACT sleep-time RTC stepping and timer-group watchdog execution are not modeled. Peripheral extensions are separate changes in the stack.

Classic peripheral extensions include I2C0/1, sixteen LEDC channels,
SPI2/3 with controller-local DMA, and eight RMT channels. SPI and RMT feed
the existing board/display/WS2812 interfaces. `esp32dev-loopback` supplies a loopback board fixture; `ttgo-tdisplay`
hangs an ST7789 on SPI3 (below). Hardware fade and RMT receive
are not implemented; transaction-level SPI has no bit-level timing.

Classic Wi-Fi uses the shared `StationLink` access point, network and Ethernet
relay. An inline DPORT clock gate guards service. A DPORT MAC reset keeps the
host link and its counts. MAC/PHY register behavior is inferred;
calibration completion is modeled without RF arithmetic or hardware timing.

Classic ADC1/ADC2 accept the existing `adc` and `adcwave` script inputs and
host raw-count API. DAC25/26 expose nominal DC millivolts in the report.
`0 touchpad 4 1` touches GPIO4's capacitive pad; `0 touchpad 4 0` releases it.
Only the ten touch GPIOs (0, 2, 4, 12-15, 27, 32, 33) are accepted. This is
separate from panel coordinates supplied by `touch`. ADC calibration
and touch counts are nominal models; see EX227 for limits.

Classic `--ble` uses the shared VHCI/HCI controller with an ELF containing
controller symbols and function sizes. It installs a guest FreeRTOS task
using the same windowed-Xtensa trampoline as S3. This is a virtual controller,
with the shared one-link, MTU-23 and no-pairing limits; see EX228.

Classic I2S0/1 RX accepts per-controller PCM (`i2s_input`) and pin-routed
sources (`attach_pcm_sources`) through the shared host API from #201/#202; a
route that matches no source slot reads the per-controller input. Standard
16/24/32-bit RX uses native DMA into internal DRAM; 24-bit samples occupy 32-bit
words, as IDF allocates them on classic. The descriptor owner check follows
LC_CONF CHECK_OWNER (off at reset, as IDF leaves it). APLL, slave clocks and
bit-level I2S timing are not modeled. An inline DPORT clock/channel gate guards
receive work. See EX229 for checks and limits.

## The camera: `--board ai-thinker-cam`

The AI-Thinker ESP32-CAM (alias `esp32cam`) models OV2640 SCCB configuration,
physical GPIO routing and I2S0 RX DMA. Supply a BMP/PPM still with `--cam-image`;
absent input produces black frames. Supported raw sensor output is RGB565 or
YUV422 (grayscale requests use the YUV wire format), at a fixed approximate 10 FPS.
Either I2C controller routed to GPIO26/27 reaches the one sensor.

`PIXFORMAT_JPEG` takes host-encoded input: `--cam-image photo.jpg` (or loader
kind 7 in the browser) with a baseline JPEG whose SOF0 size equals the sensor's
configured output size (640 × 480 for `FRAMESIZE_VGA`). Its bytes reach
`fb->buf` unchanged and `fb->len` is the file length; the model never encodes,
decodes or scales. A malformed, non-baseline or wrongly sized JPEG is refused
when supplied and the previous input stays. After `set_framesize` the host must
supply a JPEG of the new size; until then JPEG mode delivers no frame and prints
one `[camera] JPEG output … has no host JPEG of that size` notice. Frame buffers
may be in internal DRAM or, with `--psram-mb 4`, in PSRAM. Other output prints one
`[camera] unsupported sensor output` notice per board instance.
There is no adjustable FPS, JPEG encoding, ISP fidelity or per-PCLK timing claim.
Losing a route mid-frame raises IN_DSCR_ERR as a conservative model policy to
prevent splicing partial frames, not as a measured hardware fault mapping.

For the reproducible public-driver fixture (two non-zero RGB565 frame hashes):

```sh
python3 examples/classic-camera/expected-frame.py
# Supply a BMP/PPM of your choice, or the deterministic source used by the CLI test.
target/release/esp32sim --chip esp32 --board ai-thinker-cam --boot rom \
  --rom roms/esp32_rev300_rom.elf \
  --bootloader esp32/tests/fixtures/classic-camera/bootloader/bootloader.bin \
  --ptable esp32/tests/fixtures/classic-camera/partition_table/partition-table.bin \
  --app esp32/tests/fixtures/classic-camera/classic_camera.bin \
  --cam-image image.ppm --max-seconds 3 --no-dump
```

The [build recipe](../examples/classic-camera/README.md) pins IDF and the camera
driver. Its guest fixture uses 10 MHz XCLK / FIFO mode 1. FIFO mode 3 (normally
selected at 20 MHz by Arduino) has exact-vector unit coverage only, not a
20 MHz compiled-driver fixture. See [EX232](experiments.md#ex232) and its receipt.
The JPEG fixture (20 MHz XCLK, quality 12, VGA then QVGA, DRAM and PSRAM
framebuffers) is [EX256](experiments.md#ex256) with its
[recipe](../examples/classic-camera/jpeg.md).

## The board: `--board ttgo-tdisplay`

The LilyGO TTGO T-Display (ESP32-D0WDQ6-V3, 4 MB flash, no PSRAM) wires an ST7789V 1.14"
135×240 panel to VSPI through the GPIO matrix (SCLK 18, MOSI 19, no MISO) with CS 5, D/C 16,
RST 23 and the backlight on GPIO 4; two buttons, GPIO 35 (right, input only, pulled up on the
board) and GPIO 0 (left, BOOT); the battery divider on GPIO 34 with its enable on GPIO 14.
`esp32/src/board/tdisplay.rs` is the board: a `DcsPanel::st7789()` fed the bytes of every SPI
transaction whose routes are SCLK 18 / MOSI 19 while GPIO 5 is low (MicroPython's `SPI(2)` and
TFT_eSPI both use VSPI, but the selection is by pins, so HSPI routed to the same GPIOs works
too), D/C and RST from the GPIO edges, the buttons as released inputs (`press boot 150`,
`press right 150` in a script).

The glass shows RAM columns 52..186 and rows 40..279 of the controller's 240×320, as a portrait
135×240 frame (`--tft-png`, the page's panel). That window is derived from how the drivers
address it: st7789py's 135×240 table puts rotation 0 (MADCTL 0x00) at column offset 52, row
offset 40, and rotation 2 (MADCTL 0xC0, MX|MY) at offsets 53/40, which is the same rectangle
mirrored around column 239 and row 319. Rotation 1 (MADCTL 0x60, the usual landscape) draws
its 240×135 with the top edge on the glass's right: the page rotates the frame 270° for this
board by default (`?rotate=`, the panel's selector). Red and blue are exchanged unless MADCTL's
BGR bit is set, as on the Waveshare C6 module; INVON is not applied (it compensates the IPS
polarity). The orientation of the glass relative to the module (which edge is "up") is taken
from the drivers' rotation 0, not measured on a board.

Checked end to end with official MicroPython v1.29.0 (IDF 5.5.2) and a pure-Python ST7789
application: the `machine.SPI(2)` driver's CPU and DMA transactions through SPI3, the window and
pixel writes landing where the driver addresses them, Wi-Fi join and HTTPS fetch through the
virtual access point. That application is [`examples/ttgo-tdisplay`](../examples/ttgo-tdisplay/README.md),
and the browser runs it as the `tdisplay-prices` demo (`run.html?wasm&fw=tdisplay-prices`): the
official MicroPython image fetched by `tools/fetch-micropython.sh`, the app's LittleFS image in
`web/wasm/fw/public/`, demo prices instead of a fetch because the browser has no sockets.

