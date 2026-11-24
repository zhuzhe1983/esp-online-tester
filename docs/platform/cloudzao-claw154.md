# CloudZao Claw 1.54

This adds the independent `cloudzao-claw-154` ESP32-S3 board identity. The public
brand is CloudZao; the manufacturer's repository, SKU, firmware board name and
upstream artifacts retain `Metalio-Claw-1.54` for traceability. The implementation
supports a display/input subset. A bounded probe of the unmodified official image
reaches `app_main()` and a nonblank display, then remains in ML307 detection.
Complete application functionality and physical hardware remain unverified.

## Source and board contract

The fixed source is [CloudZao/Metalio-Claw-1.54 at
ba2fcbde5cd7c42e6f684aea64bed63951309684](https://github.com/CloudZao/Metalio-Claw-1.54/tree/ba2fcbde5cd7c42e6f684aea64bed63951309684).
The product knowledge JSON records fetched URLs, UTC timestamps, byte counts,
SHA-256 hashes and source IDs for the README, board configuration, driver code,
repository tree, photograph, license and relevant ESP-IDF register definitions.

| Area | Official source contract | Implemented scope |
| --- | --- | --- |
| MCU / memory | ESP32-S3; 16 MiB Flash; 8 MiB Octal PSRAM | Separate S3 board identity and matching memory configuration |
| Display | ST7789, 240 × 240; SPI3; MOSI 10, SCLK 9, CS 14, DC 8 | Routed guest SPI writes reach RGB565 framebuffer |
| Display control | RESET 18; active-high backlight 13 | Reset, GPIO/PWM backlight; backlight does not erase GRAM |
| Touch | CST816S at 0x15; SDA 41, SCL 42 | Routed I²C register polling; both axes mirrored as the upstream driver expects |
| Touch reset / IRQ | Both are `GPIO_NUM_NC` | No invented interrupt or reset GPIO |
| Front navigation | Previous, home, next in `touch_feed.cc` | Source-derived raw contacts; release is observable in the touch registers |
| Buttons | BOOT 0; active-low power key 47 | GPIO input press/release; power button does not shut down the simulated machine |
| Power hardware | Hold 48, USB detection 1, battery ADC logic | Not implemented by this board adapter |
| Audio | ES8311; I²S MCLK 5, BCLK 6, WS 4, DOUT 15, DIN 17; PA 21 | Source facts only; no fabricated codec-register responses |
| Cellular | ML307; TX 12, RX 11 | Source facts only |

The shared ST7789 implementation is reused with this board's own pin profile.
Firmware identity and validation are not inherited from a LILYGO product. Touch
is attached separately because the source board polls it without an IRQ.

The upstream mirror transform is `x_max - raw_x` / `y_max - raw_y`. A host contact
at visible pixel `(x, y)` therefore reports `(240 - x, 240 - y)` after clamping
the visible pixel range to 0…239. The cover contacts use raw X values 60, 40 and
20 and raw Y 600, corresponding to the source's logical display coordinates
180/200/220 and approximately −360. These are representative source-derived
contacts, not a measurement of the physical controller's calibration.

No microSD slot or fitted SC7A20H is claimed: the inspected board source does not
establish either. Whole-application networking, cloud services, speech, cellular,
audio, battery state, power latching and physical flashing remain unverified.

## Model and provenance

The inspected official tree contains no STEP, STL, 3MF, KiCad, DXF, IGES or FCStd
mechanical model and no dimension drawing. `images/product.jpg` and
`images/product_en.jpg` have the same SHA-256, so they provide one independent
view. The exact upstream photograph is retained unmodified with the source
license and a separate rights notice.

An official burner listing named `Metalio 1.54` has another oblique image and
provider ID 35, but does not bind the image to this exact SKU or hardware revision.
Its fetched receipt is recorded in `model-plan.json` as a candidate search result.
It is not used to infer dimensions, side-button wiring or hidden structures.

`tools/build-cloudzao-claw154-model.py` builds a small, offline, deterministic
preview. `model-plan.json` records manually selected image landmarks, a planar
homography, the nominal 1.54-inch square-display scale and every chosen estimate.
The model uses millimetres. The inferred front-cover envelope is approximately
35.7 × 42.8 mm. The 8.5 mm depth parameter is uncalibrated presentation spacing;
it is not a manufacturer specification. The committed geometry's shell and front
cover occupy approximately 35.7 × 42.8 × 8.33 mm, excluding the nominal connector,
separate debug controls, labels and hit regions. Neither value passes a physical
dimension acceptance test.

The live display is a separate 27.659 mm square surface that receives the guest
framebuffer. The eyes in the reference photograph never become simulated output.
The black front cover, navigation marks, silver wall, unobserved plain rear
closure, nominal USB-C face and UNKNOWN board are separate objects. Only shell
objects receive shell material changes. `pcb-main` is a blank simplified board
with UNKNOWN on both faces; it contains no guessed component layout or traces.

The inspection arrangement separates the front cover, UNKNOWN board and back
along Z. These offsets are illustrative viewing positions, not verified assembly
or disassembly dimensions. BOOT and PWR appear in a separate DEBUG INPUT strip
because their physical positions are unknown. The cover navigation regions are
estimated from the photograph. USB-C is visual only, with connections disabled
because the exact connector, mating dimensions and wiring are unverified.

Rebuild into a new directory:

```sh
python tools/build-cloudzao-claw154-model.py --output /tmp/cloudzao-claw154-model
```

This requires Python's standard library only and makes no network requests. The
builder checks the fixed reference-image hash before generating the geometry.
The model metadata contains hashes for every runtime asset.

## Verification and limits

Two board tests and three guest CPU diagnostics passed. The latter execute
authored Xtensa load/store instructions through the S3 machine, GPIO matrix,
SPI3 and I²C0 register paths. The host test does not paint the framebuffer.

```sh
cargo test -p esp32s3 --lib board::cloudzao_claw154::tests
cargo test -p esp32s3 --test cloudzao_claw154
```

The diagnostics cover routed screen pixels, incorrect MOSI wiring, backlight
state without GRAM loss, reset requiring guest reinitialization, correct and
incorrect I²C pins/address, mirrored touch coordinates, cover-key release,
absence of a fake touch IRQ, unsupported ES8311 rejection, BOOT/power press and
release, and release delivery while the machine is paused.

`tools/fixtures/cloudzao-claw154-probe.mjs` exports `cloudzaoClaw154Probe()`, an
original 5,760-byte SRAM application for browser/WASM checks. It writes a red
frame through SPI3, changes to green while BOOT is held, and returns to red on
release. UART markers are `CLOUDZAO CLAW154 SRAM READY`, `BUTTON DOWN`,
`BUTTON UP` and `FRAME COMPLETE`. The generated bytes have SHA-256
`666d7b2e553d6788aafc48282ebfec1fb121b76849295437d098b8f55d4ca4f8`.
It is a diagnostic fixture, not a third-party or official firmware image.

The catalog fragment, knowledge index and knowledge document passed their JSON
schemas with format checking. All source references close over known source IDs.
The model rebuilt byte-for-byte into a fresh directory; geometry is finite and
its stored part bounds match the exported floats. Asset receipts and measured
geometry bounds are recorded in `cloudzao-claw154-verification.json`.

Browser display/input/inspection acceptance is an integration check reported
separately. This document does not claim it from native tests. Independent
multi-view visual acceptance, rear/side geometry, internal layout, actual body
dimensions, manufacturing tolerance and physical touch calibration remain
unverified. No complete-official-firmware or real-device pass is asserted.

## Unmodified official image probe

`tools/probe-cloudzao-claw154-official.mjs` runs the public platform runtime,
actual Worker and WASM against the catalog's exact full official image. It
verifies the complete download hash and length, and verifies the bytes again at
the Worker load boundary. Only the full Flash image and ESP32-S3 ROM are loaded;
merged format and ROM boot are mandatory. There is no application extraction,
app-direct fallback, replacement application, injected guest input, guest patch
or ROM stub. Downloaded firmware, cache and writable Flash remain in RAM.

Run from an integrated checkout with a built WASM and the official S3 ROM:

```sh
ESP32SIM_ROM_DIR="$PWD/roms" node tools/probe-cloudzao-claw154-official.mjs \
  --duration-ms 45000 --output /tmp/cloudzao-claw154-official-receipt.json
```

`ESP32SIM_TEST_ROOT` can point at a different integrated checkout. The script
limits the execution budget to at most 60 seconds and saves only a sanitized JSON
receipt. The UART0 stream is retained separately from mirrored USB output, with
short excerpts and important startup/error lines. URL queries, MAC addresses,
UUIDs, IPv4 addresses and credential-like fields are redacted.

The recorded run is in
[`evidence/cloudzao-claw154-official-probe-20261122.json`](evidence/cloudzao-claw154-official-probe-20261122.json).

| Observation | Recorded result |
| --- | --- |
| Official image | `Metalio-Claw-1.54_2.0.51_20261014_1105.bin`, 16,722,264 bytes |
| Complete-image SHA-256 | `7e4d9ec9bb3a539598ee1f7d68834eae0888d1d8b5a26ea34cf04b54955b7550` |
| Execution path | ROM → second-stage bootloader → OTA application at 0x100000 → `app_main()` |
| Application log | Project `Metalio-Claw-1.54`, version `2.0.51`, ESP-IDF `v5.5.5-832-g2553c5ad432` |
| Execution budget | 45,053 ms wall time; final published guest statistics 27.8 s and 455,307,234 instructions |
| Guest display | 240 × 240 RGBA, 80 received frame events, 16,991 nonblack pixels, 241 distinct RGB colors |
| Final RGBA SHA-256 | `ed5db04f5696469b333a53d4ff711b3f93fa40fdced30f496a1a846dd71fd699` |
| Runtime error / stop | No reported error; actively paused after the wall-time budget |
| Blocking behavior | ML307 baud detection reaches round 22 with `tx_written=36`, `rx_bytes=0` and a silent modem |

This establishes that this exact official image boots into application code and
produces display pixels through the actual board model. It does not establish a
usable cellular application: ML307 is not implemented and its repeated AT
requests receive no response. No official-firmware touch/key interaction was
injected in this probe; the independent authored guest tests cover that input
subset. Audio, cloud services and physical hardware were not validated. The
probe uses Node's HTTP download path, so it is not a browser CORS result. Wall
time and instruction throughput depend on the host and concurrent work.
