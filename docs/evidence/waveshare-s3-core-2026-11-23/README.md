# Waveshare S3 core boards

Four exact board profiles now boot original manufacturer-linked firmware from the
S3 ROM. Zero's Arduino example emits its real color sequence; Nano's original
test image emits real PWM changes. Zero, Pico and DEV-KIT-N8R8 additionally run
unchanged MicroPython images with UART commands that cause guest LED writes and
read BOOT press/release values. No display or extra peripheral is invented.

| Product | Flash / PSRAM | BOOT | Modeled LED wiring |
| --- | --- | --- | --- |
| ESP32-S3-Zero | 4 / 2 MiB | GPIO0, active low | GPIO21, GRB addressable LED |
| ESP32-S3-Pico | 16 / 2 MiB | GPIO0, active low | GPIO21, RGB addressable LED |
| ESP32-S3-Nano | 16 / 8 MiB | No documented physical BOOT key | Discrete active-low R46/G0/B45; active-high indicator48 |
| ESP32-S3-DEV-KIT-N8R8 | 8 / 8 MiB | GPIO0, active low | GPIO38, RGB addressable LED |

The source pages are the official
[Zero](https://docs.waveshare.com/ESP32-S3-Zero),
[Pico](https://docs.waveshare.com/ESP32-S3-Pico),
[Nano](https://docs.waveshare.com/ESP32-S3-Nano) and
[DEV-KIT-N8R8](https://docs.waveshare.com/ESP32-S3-DEV-KIT-N8R8) documentation,
their linked schematics and exact examples. Per-board `sources-*.json` files
preserve new HTTP response dates, byte counts and SHA-256 values. Original source
bodies stay outside Git. No source lookup here certifies other capacity or header
variants.

## What the adapter models

Each exact board has its own memory and input contract. Addressable LED data is
accepted only on its documented GPIO. The existing WS2812-class decoder handles
RMT and physically routed software GPIO pulses, including its reset/latch
interval. Pico and DEV's official `rgb.py` examples explicitly select
`ORDER=(0,1,2,3)`, while Zero's official Arduino BlinkRGB example uses the GRB
convention. Pulse limits and intensity conversion are digital display policies,
not a measurement of the LED's timing tolerance or optical brightness.

Nano's LED is discrete: it follows physical GPIO enable, mux, inversion and lazy
PWM duty snapshots. GPIO0 is an LED channel, so a synthetic BOOT button on it
would be incorrect. Its actual RESET button belongs to the platform reset action,
not an ordinary guest GPIO button. Analog electrical behavior, radios and the
USB hub/bridge internals remain outside this profile.

### Source conflicts retained

Nano's schematic names DL1 `SMLP34RGB2W3` but draws common-anode pin 2. ROHM's
[SMLP34RGB Rev.011 drawing](https://fscdn.rohm.com/en/products/databook/datasheet/opto/led/chip_multi/smlp34rgb-e.pdf)
uses pin 1 as common anode. That package-pin discrepancy remains unresolved.
The board's active-low nets are established by its schematic; color names use
the official Waveshare demo's Arduino pins 14/15/16 and the
[pinned Nano variant mapping](https://github.com/espressif/arduino-esp32/blob/bc8ab15634eb0402d39e1e259a5a4a29be3e6e1c/variants/arduino_nano_nora/pins_arduino.h)
to GPIO46/0/45. This supports the software display contract, not certification
of that footprint or physical color calibration.

Pico's product text simultaneously identifies S3R2/2 MiB PSRAM and describes
GPIO33–37 as Octal-PSRAM pins. The profile uses the exact stated chip capacity;
it does not claim those pads as independently verified expansion I/O. Its linked
MicroPython image uses a 4 MiB flash layout, which is a software subset of the
board's physical 16 MiB Flash.

## Firmware identity

[firmware-sources.json](firmware-sources.json) identifies the original container
URL/hash and each exact ZIP member/hash/offset. The roles are deliberately distinct:

- Zero: official **Arduino example**, three original segments at `0`, `8000h`
  and `10000h`. The archive's `combine/target.bin` is actually chip ID 5
  (ESP32-C3), so it is rejected before boot. The valid S3 segments are not
  repackaged into a purported manufacturer merged image.
- Nano: official **test firmware**, three original segments at the same offsets.
- Pico: manufacturer-linked MicroPython v1.23.0 / 2024-06-02, unchanged merged
  image at offset 0. The same linked image provides Zero's additional input test.
- DEV-KIT-N8R8: manufacturer-linked MicroPython v1.20.0 / 2023-04-26, unchanged
  merged image at offset 0, selected from the exact N8R8 archive directory.

None of these examples, interpreter images or project-authored test commands is
relabeled as factory firmware.

## Native acceptance

[verification.json](verification.json) records the current compiler, ROM and
probe hashes, source hashes, commands, original firmware identities and measured
outcomes. Five runs each execute eight guest seconds with zero software resets:

| Run | Observed result |
| --- | --- |
| Zero Arduino example | Eight real LED frames; white, off, red, green and blue states agree with the source loop |
| Zero linked MicroPython | Guest emits RGB `[17,34,51]` then `[90,30,10]`; BOOT replies `1`, `0`, `1` |
| Pico linked MicroPython | Same two guest-emitted RGB values and actual BOOT replies |
| Nano original test | 546 derived LED state updates from guest GPIO/PWM; no addressable LED stream |
| DEV-KIT-N8R8 linked MicroPython | Same two guest-emitted RGB values and actual BOOT replies |

The MicroPython commands are project-authored external inputs delivered over
UART0, with a real carriage-return Enter. They do not modify the image, inject
pixels/LED colors, change guest memory or skip code. The firmware executes its
own drivers and prints `CORE_BOOT_PRESSED 0` and `CORE_BOOT_RELEASED 1`.

Two failed transport attempts are retained: generic serial selected USB while
the REPL was on UART0, and UART0 with LF echoed commands without submitting them.
Their queued inputs and echoed text were not counted as acceptance. The final
probe converts UART0 script line endings to CR; it does not alter the common
emulator's script parser.

Three focused tests pass for exact capacities/controls/color order, physically
routed Nano GPIO/PWM, and addressable GPIO pulse/latch behavior. Probe Clippy
passes with warnings denied. Browser/WASM and physical-board verification remain
separate integration tasks.

## Reproduction

```sh
cargo test --release --manifest-path docs/evidence/waveshare-s3-core-2026-11-23/probe/Cargo.toml
cargo clippy --release --manifest-path docs/evidence/waveshare-s3-core-2026-11-23/probe/Cargo.toml --all-targets -- -D warnings
cargo build --release --manifest-path docs/evidence/waveshare-s3-core-2026-11-23/probe/Cargo.toml
WAVESHARE_INPUT_SCRIPT=docs/evidence/waveshare-s3-core-2026-11-23/inputs-pico.txt \
  <probe-binary> waveshare-s3-pico <esp32s3_rev0_rom.elf> 8 <output-prefix> 0:<original-micropython.bin>
```

For Zero's example or Nano's test, omit the input script and pass the three
unmodified ZIP members as `0:<bootloader> 8000:<partitions> 10000:<app>`.
Use exact sources and hashes from `firmware-sources.json`. Raw console and sample
logs remain in ignored `output/waveshare-s3-core/`; the committed receipt retains
their hashes and sizes, without unrelated host or personal information.
[integration.json](integration.json) supplies central registration data; this
commit does not change central CLI/WASM/catalog registration.
