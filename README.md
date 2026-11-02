<p align="center">
  <img src="docs/assets/esp32sim-logo-icon.png" alt="esp32sim logo: a smiling chip in an emulator window" width="240">
</p>

# esp32sim

esp32sim is an ESP32 emulator written in Rust. It runs real ESP32 firmware on your computer or in
a browser tab, without the hardware.

It boots the chip's own mask ROM, the real second-stage bootloader and your unmodified application.
It also models the board around the chip: displays, touch, LEDs, buttons, audio, camera and WiFi.

Try it in your browser, nothing to install: https://mikroverk.github.io/esp32sim/

`--chip esp32 --board ai-thinker-cam --cam-image image.ppm` supplies the AI-Thinker
ESP32-CAM's OV2640 with a BMP/PPM still image. Capture supports raw RGB565/YUV422
into internal DRAM at a fixed approximate 10 FPS; the usual JPEG+PSRAM
CameraWebServer path is unsupported. See the [camera scope and example](docs/esp32.md#the-camera---board-ai-thinker-cam).

## Supported chips

| Chip | CPU | Status | Boards |
| --- | --- | --- | --- |
| ESP32-S3 | 2 x Xtensa LX7 | mature | Atech 14-port kit, Waveshare Touch-LCD-4B, Touch-AMOLED-1.8, ESP32-S3-CAM |
| ESP32-C3 | RISC-V RV32IMC | draft | console only |
| ESP32-C6 | RISC-V RV32IMAC | draft | Waveshare ESP32-C6-LCD-1.47, with the 802.15.4 radio |
| ESP32 (classic) | 2 x Xtensa LX6 | early | LilyGO TTGO T-Display, AI-Thinker ESP32-CAM (raw capture), Ulanzi TC001 |
| ESP32-P4 (rev < 3; rev ≥ 3: hello_world, ESP-Hosted) | 2 x RISC-V RV32IMAFC | draft | console only; the C6 co-processor over SDIO |

## Features

- The full boot chain: mask ROM, bootloader, FreeRTOS, then your app. No patched firmware.
- WiFi with the unmodified Espressif WiFi library. Firmware joins a virtual access point, gets
  DHCP, DNS and NTP, and can reach the real network, HTTPS included.
- Board models, so firmware can draw on a display, read touch, light LEDs and play sound.
- Speed: the S3 has a block cache and a JIT, and runs hello_world about 30 times faster than real
  time.
- A WebAssembly build of the same emulator for the browser.
- Comparisons with real chips: the S3 in JTAG lock-step, the C3 and C6 by console diff. See
  [hardware testing](docs/hardware-testing.md).

## Demos

These run on the [demo site](https://mikroverk.github.io/esp32sim/):

- Linux 6.11 on the ESP32-S3 ([svermigo/Linux-on-esp32-S3](https://github.com/svermigo/Linux-on-esp32-S3)), with a login shell.
- The Atech kit's Pocket Synth and its C64 SID jukebox ([atech-firmware](https://github.com/joakimeriksson/atech-firmware)).
- An LVGL touch panel at 480x480 with a SID player.
- [pocket-tank](https://github.com/mediacutlet/pocket-tank), where a small transformer model steers the fish.
- An 802.15.4 energy scanner, and two Contiki-NG motes running RPL/IPv6, on the ESP32-C6.
- MicroPython on a classic ESP32 T-Display.

You can also load your own firmware for any of the chips.

## Quick start

No Rust needed: the [releases](https://github.com/mikroverk/esp32sim/releases) carry prebuilt binaries
for macOS and Linux, and [docs/quick-start.md](docs/quick-start.md) takes your own build from the tarball
to a run in three steps. From a checkout:

```sh
cargo build --release
tools/fetch-demo-assets.sh --no-linux       # downloads the mask ROMs, which are not in the repo

FW=web/wasm/fw/public                       # demo firmware included in the repo
./target/release/esp32sim --boot rom --rom web/wasm/fw/esp32s3_rev0_rom.elf \
    --bootloader $FW/atech-bootloader.bin --ptable $FW/atech-ptable.bin --app $FW/atech-firmware.bin \
    --script $FW/atech-script1.txt --wav out.wav --tft-png tft.png --max-seconds 5
```

This boots the Atech synth firmware, plays a short script, and saves the audio to `out.wav` and
the display to `tft.png`. Use `esp32sim-c3`, `esp32sim-c6`, `esp32sim-p4` or `--chip esp32` for the other chips.

[Getting started](docs/getting-started.md) covers the rest: your own ESP-IDF builds, each chip,
WiFi, scripts, the browser build and debugging.

## Documentation

- [Quick start](docs/quick-start.md): install the release, fetch the mask ROM, run your own build, read the result
- [Getting started](docs/getting-started.md): running firmware on each chip, WiFi, scripts, the browser build
- [CLI reference](docs/cli.md): every flag, environment variable and output file
- [Boards](docs/boards.md): what each board models, and how to add one
- [Architecture](docs/architecture.md): how the emulator is put together
- [Peripherals](docs/peripherals.md): which hardware blocks are modelled, and how completely
- [Hardware testing](docs/hardware-testing.md): how the emulator is compared with real chips
- [All documents](docs/README.md): chip notes, design decisions and plans

## Contributing

Bug fixes, peripherals, boards and new chips are welcome. [CONTRIBUTING.md](CONTRIBUTING.md) lists
what a pull request needs; these are the same checks CI runs. Security problems go through
[SECURITY.md](SECURITY.md), not the public issue tracker.

## Credits and license

esp32sim is released under the [MIT License](LICENSE).

Authors: Joakim Eriksson, RISE ([@joakimeriksson](https://github.com/joakimeriksson)),
Alice ([@aliceisjustplaying](https://github.com/aliceisjustplaying)) and
Mattia Fiumara, Schematik ([@mfiumara](https://github.com/mfiumara)), together with
[other contributors](https://github.com/mikroverk/esp32sim/graphs/contributors).

The emulator was written from the ESP-IDF register headers, the Xtensa core configuration that
ships with ESP-IDF, the RISC-V specification, the chips' technical reference manuals and observed
firmware behaviour. QEMU was consulted only to confirm instruction semantics; no code was copied.

esp32sim is unofficial and not affiliated with Espressif Systems.

Third-party code, data and demo files keep their own licenses:

- pocket-tank ([mediacutlet/pocket-tank](https://github.com/mediacutlet/pocket-tank), MIT). Its
  bootloader, partition table and app are in `web/wasm/fw/public/` with `pocket-tank-LICENSE.txt`;
  its model is downloaded.
- MicroPython ([micropython.org](https://micropython.org/download/ESP32_GENERIC/), MIT): the
  official ESP32_GENERIC v1.29.0 image used by the T-Display demo, downloaded by
  `tools/fetch-micropython.sh` and not in the repo. The demo's app and filesystem are in
  `examples/ttgo-tdisplay`, with Russ Hughes's st7789py driver and fonts (MIT); the notice is in
  `web/wasm/fw/public/tdisplay-NOTICE.txt`.
- Mask ROM ELFs ([espressif/esp-rom-elfs](https://github.com/espressif/esp-rom-elfs), Apache-2.0),
  downloaded, not in the repo.
- xterm.js ([xtermjs/xterm.js](https://github.com/xtermjs/xterm.js), MIT), downloaded, not in the repo.
- The Linux image ([svermigo/Linux-on-esp32-S3](https://github.com/svermigo/Linux-on-esp32-S3),
  GPL-3.0), downloaded, not in the repo.
- ESP-IDF ADC calibration data (Espressif, Apache-2.0): the curve-fitting coefficients and S3 ADC1
  calibration from #165, in `esp-periph/src/sar_adc.rs`; see `esp-periph/LICENSE-ADC`.
