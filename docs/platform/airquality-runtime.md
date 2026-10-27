# Air Quality K131 bring-up

Runtime and evidence for original K131. K131-V11 keeps its separate identity: newer Stamp-S3A adds a GPIO38 RGB power switch; the published external display/sensor/control routes match, but this does not establish identical firmware acceptance. Sources and retrieval hashes are in [sources.json](evidence/airquality/sources.json). The original source-only checkpoint remains in history; the implementation below enables only original K131.

The official factory image v0.2.0 is 8,323,072 bytes, SHA-256 `ac47e80ba0b0e6c304e90ba10aa978f367d4369a2e449c1f4a58813a671b3216`, and identifies IDF v4.4.6-dirty. Its bytes remain an ignored remote input. The public AirQUserDemo repository supplies initialization and button behavior, but current HEAD is not asserted to be the exact binary build.

| Circuit | Official source route |
| --- | --- |
| GDEY0154D67 / SSD1681 | SCK5 MOSI6 CS4 DC3 RESET2 BUSY1; 200×200 monochrome |
| Buttons | A=G0, B=G8; WAKE G42 and HOLD G46 belong to power control |
| I²C | SDA11/SCL12: SCD40 0x62, SEN55 0x69, RTC8563 0x51 |
| SEN55 supply | G10: source sets LOW to enable; inspect schematic polarity separately from name |
| Battery/buzzer | G14 ADC battery divider / G9 buzzer |
| Grove | separate SDA13/SCL15; no visual connector placement inferred from I²C routing |

The display datasheet's 0x44 X address uses byte units and two parameter bytes, unlike SSD1677 pixel-address words. It therefore requires a separately identified SSD1681 model. The PDF contains apparent copied default-window values (0x15,0x127), while this glass and initialization explicitly use 200×200; retain that discrepancy rather than silently presenting all register defaults as silicon verified. BUSY polarity also conflicts between a prose paragraph and the command table/official driver; the command table and driver use active HIGH.

The factory program initializes its filesystem, display and Wi-Fi before I²C; it requires SCD40 ready polling before installing button callbacks. Missing sensors cannot be replaced by mocked application output. Implement source-backed digital sensor commands with CRC, readiness and explicit virtual sample values; analog environmental dynamics and network/cloud access remain separate unimplemented scope.

## Implemented digital paths

`airquality` / `m5stack-k131` selects a distinct board, not a renamed PaperMono. It routes SSD1681 transfers only through the published CS/SCK/data pins, uses DC/reset and BUSY edges, and retains the optical image until an update completes. X windows/counters are bytes; Y is a nine-bit row counter. Both monochrome RAM planes are stored, but output uses the BW plane, including normal/inverse/bypass control. Analog waveform LUTs, ghosting, gray/red glass and physical refresh latency are not simulated. BUSY completion is a bounded digital delay (1/20 virtual ms), not a panel timing measurement.

Sensirion command words and CRC-8 (polynomial 0x31, initial 0xff) pass through the actual I²C bus at SDA11/SCL12. SCD40 waits five seconds (or thirty seconds in low-power periodic mode); readiness is consumed only by a complete measurement read. The measurement cadence is free-running, so a delayed host read does not restart it. SEN55 supply follows GPIO10 active LOW: the schematic's ME1502CM5G has an active-low EN. Power removal discards measurement state and subsequent addresses NACK.

The fixed virtual sample profile is explicit: SCD40 420 ppm, 25°C, 50% RH; SEN55 PM1/2.5/4/10 = 5/7/9/10 µg/m³, 25°C, 50% RH, VOC index 100, NOx index 1. No ambient measurements are inferred. Identity responses use synthetic zero serials; fan diagnostics, analog warm-up and exact response latency are outside the model.

The SCD40 datasheet gives a maximum STOP duration of 500 ms. A negative probe that used exactly that maximum rejected this factory build's next START 85,318 cycles (0.355492 ms) early: START cycle 486,886,902 versus completion 486,972,220. The guest then remained in its data-ready loop. The digital endpoint now uses an explicitly chosen 450 ms completion point within that bound; it is not a hardware-calibrated timing claim, nor a change to guest code or the emulator clock. Other short command-response delays are abstracted; firmware requiring analog timing needs additional work.

RTC8563 currently NACKs. Consequently the official page's `2000-00-00` / `00:00` and battery `0.0V` are retained as unsupported-device output. Buzzer, external Grove behavior, Wi-Fi/cloud access, supply latch and sleep are not accepted. The virtual profile is USB powered; the visible power button is not mapped to an invented GPIO action.

## Geometry and controls

The original manufacturer STL, coordinates and triangle stream remain unchanged. Six assets rebuild byte-for-byte from the committed plan. The 56×72 mm body matches its nominal width/height; selected CAD depth is 26.5 mm against the original page's 24.1 mm, an unresolved 2.4 mm discrepancy retained from the prior review. Existing front/back silhouette scores (0.8933 / 0.9126) remain below 0.95, not promoted to a 1:1 pass. Printed Stamp-S3 and rear labels remain incomplete; `UNKNOWN` is an estimated internal PCB envelope.

Two invisible front hit planes follow the upper/lower integral flexures. The official rear pin illustration identifies A/G0 above B/G8. Positions are CAD/photo estimates, ±0.6 mm, and use existing green hover behavior. They do not add new plastic or deform the manufacturer's flexible tabs. A/B also map to keyboard A/B. This e-paper has no touch sensor; screen clicks do not inject touch.


## Acceptance and reproduction

The official v0.2.0 remote image boots through the actual ESP32-S3 ROM, not an application screenshot or source-built replacement. Native and production WASM display the sensor dashboard. B opens Settings, B again opens AP setup, and A returns from Settings. AP graphics/QR generation do not establish host Wi-Fi access. The unchanged image, ROM and production WASM hashes are recorded in [official-wasm.json](evidence/airquality/official-wasm.json); actual guest pixels are [official-samples.png](evidence/airquality/official-samples.png).

In a headless workbench at 1440×1000, mouse positions (640,350) and (640,322) hit the lower B and upper A planes with green button hover. A ~0.35 guest-second down/up interval is used, followed by two guest seconds for display completion. A mouse B→A sequence followed by keyboard B produces six recording events with the correct semantic IDs; replay reaches the identical Settings framebuffer. Playback is stats-clock scheduled and not bit/cycle exact. The receipt preserves lateness, including its ~0.1244s final accumulated difference. Both main and mini use the same hash `c64bd09a0ec2e34723ace59182661e8b6bbca04ed62ffa05b1df248d39f34e4d` for Settings.

The factory boot writes an NVS page. Recording/replay tests explicitly reset only their isolated browser's device-storage scope before each fresh start; they do not bypass the platform's pristine-storage guard or automatically clear real user data. Before loading, select the fixed official catalog source; call `loadFirmware()`, then in a dedicated empty test context call `recordingStart()` and use bounded guest-time waits. Stop recording, retain its bundle, and replay in another pristine test context (or explicitly reset that isolated test scope). See [browser-recording.json](evidence/airquality/browser-recording.json) and [mini.json](evidence/airquality/mini.json). Settings/AP raw screenshots and full UART are omitted because they contain a device identifier; hashes and input receipts remain.

```sh
tools/wasm-build.sh
node tools/probe-airquality-official.mjs <fixed-official-https-url> <sha256>
# Native: obtain the same remote image as an ignored local test input.
target/release/esp32sim --board airquality --flash-image <fixed-official-image> \
  --boot rom --rom web/wasm/fw/esp32s3_rev0_rom.elf --flash-mb 8 --psram-mb 0 \
  --max-seconds 20 --tft-png <output.png> --no-dump
```

[Checks](evidence/airquality/checks.json) include latest stable Rust 1.99.1, both warning-denying Clippy commands, 1,438 Rust tests in 132 groups, 11 WASM scenarios, 100 platform tests, schema, real recording, timing/BLE/worker checks, 111,126 differential WASM JIT cases and 24 GPIO waveform cases. No golden files changed. [CPU samples](evidence/airquality/cpu.json) compare five alternating 30-guest-second hello pairs per S3/C3/C6 against a fixed main84e67571 binary set. This is a cross-revision regression guard, not an optimization attribution; no S3 core change requires Pocket in this slice.
