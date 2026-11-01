# StackChan K151 internal protocol fixture

This ESP-IDF v5.4.3 application is an internal simulator diagnostic, not M5Stack's factory firmware. It is not in the public firmware selector and has not been tested on physical hardware. The official V1.5.1 factory image remains the product default.

The guest initializes CoreS3 AW9523 reset lines, AXP2101 backlight, a 320×240 ILI9342 RGB565 display, FT6336U touch, PY32 GPIO/RGB, Si12T touch and SCSCL feedback servos. It draws the face through SPI and reads all inputs through hardware controllers. It never receives host-generated serial assertions or fabricated servo feedback.

At boot, serial `CHECK … PASS/FAIL` lines report expander identity, both servo goal/readback values, a wrong TX GPIO, motor supply off, and restored supply. `PLATFORM_READY stackchan` marks the interactive phase. The top front pad sends pan to +40° and red LEDs, middle sends tilt to +60° and green LEDs, back sends pan to −40° and blue LEDs; release returns the relevant axes to zero. These motions use the BSP defaults of raw 460/620, not per-device calibrated zero positions. Screen touch changes the mouth color independently. `TOUCH_PADS` and `TOUCH_SCREEN` report values actually read by the guest.

Build with ESP-IDF v5.4.3 activated:

```sh
idf.py -C examples/platform-stackchan build
tools/wasm-build.sh
node tools/probe-stackchan.mjs --build examples/platform-stackchan/build
```

The probe runs the production worker/WASM path with pristine in-memory device storage. It checks firmware serial output, all twelve RGB values, servo feedback angles, framebuffer changes, simultaneous pad input, touchscreen separation, reset, and recording replay. It prints a JSON receipt with input/WASM hashes. It does not use the network to fetch firmware. Fetch ROMs as described in the platform README first.

The firmware uses a 16 MiB Flash layout and internal RAM; the K151 runtime provisions 8 MiB PSRAM. Build outputs remain ignored. An optional merged image can be produced with:

```sh
python -m esptool --chip esp32s3 merge_bin -o stackchan-diagnostic.bin \
  --flash_mode dio --flash_size 16MB --flash_freq 80m \
  0x0 examples/platform-stackchan/build/bootloader/bootloader.bin \
  0x8000 examples/platform-stackchan/build/partition_table/partition-table.bin \
  0x10000 examples/platform-stackchan/build/platform_stackchan.bin
```

Protocol sources are pinned in [the source audit](../../docs/platform/research/stackchan-runtime-sources-2026-11-01.json): StackChan-BSP `e2f3b02dbe7f28e26a02adc72bef3aa2c545e4a0`, M5GFX `7cb26b44ab31bb41a0a597196dd2c6d868869a21`, and official schematics. Source is MIT; the ESP-IDF runtime is Apache-2.0. The separate factory image uses IDF v5.5.4, so this diagnostic passing alone does not certify factory compatibility.
