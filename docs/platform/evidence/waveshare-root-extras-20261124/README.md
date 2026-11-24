# Classic display, thermal module and controller-containing Basic Kits

Three storefront families contain seven sale SKUs and eight explicitly named
runtime profiles. They are selected into normal five-family delivery increments;
this shared evidence directory is not itself a three-family release.

## ESP32-Touch-LCD-3.5

The classic ESP32-D0WDR2-V3 board has 16 MiB Flash and 2 MiB PSRAM. SKU 32870
and 32871 distinguish battery inclusion. ST7796 SPI, FT6336 touch, TCA9554,
RTC, codec, PMU, BOOT and SD routes retain the original board implementation.
Three native tests execute actual guest instructions and check pixels, reset,
touch, BOOT, I2C routing and SD command/MISO gating, including negative routes.

The unchanged official 20261021 FactoryOnly image was validated and executed
through the public runtime and production Worker for 5.000 guest seconds.
It passes the Flash/PSRAM startup and enters app_main, then reports
ESP_ERR_INVALID_STATE in PMU configuration. The last 320 x 480 frame is black,
with zero frame updates. The shared AXP2101 implementation deliberately does
not acknowledge unsupported DC5/watchdog/button-cell charging operations;
the first failing transaction was not traced, so no exact register cause is
claimed. The prepared 64 MiB FAT32 host card stays pristine: this run does not
establish guest filesystem writes or completion of the manufacturer app.

Published outer dimensions and active area drive a photo-based model. The
case is hollow, the screen is an unobscured live plane, the side button is
separate and an opposite-side ray is blocked by the case. The unavailable
interior face remains UNKNOWN and is exposed only by inspection. PCB outline,
thickness, internal placement and case wall details are estimates, not CAD
or fit measurements. The battery and no-battery sale options do not establish
the precise concealed battery geometry.

## Thermal Camera ESP32 Module

SKU 30136 and 30139 are the 45-degree and 90-degree optical options. Both use
ESP32-S3-WROOM-1, 16 MiB Flash and 8 MiB PSRAM. They have neither an onboard
display nor a TF slot: the 80 x 62 sensor is not presented as a screen.

The new board implements BOOT GPIO0 and three discrete active-low RGB channels
on GPIO1/38/2. SPI2 routes on CLK6/MOSI15/MISO7, sensor CS16, calibration CS5,
SYSRST17 and PD3 are observed. Unsupported sensor/calibration transfers return
0xff; no MI0802 identity, temperature, image, optical calibration or DATA_AV
behavior is fabricated. Three native checks include actual LX7 GPIO execution
and negative route/chip-select tests.

Both unchanged original factory ZIP members were decoded, checked and run
through the public runtime/Worker for at least 5.000 guest seconds. Both enter
app_main and initialize GPIO, RGB and SPI, then report SenXor initialization
failure. Neither has a display frame. The desktop image declares 2 MiB Flash
although the physical module has 16 MiB. The web image's bootloader and app
declare no appended SHA digest; XOR validation passes, and the original ROM
SHA warning is preserved. A missing optional digest is not a successful SHA
comparison and the binary was not rewritten to hide the warning.

The 29 x 29 mm carrier is L-shaped, with three observed holes and a separate
module antenna overhang. The sensor daughter board is distinct. An open barrel
and square lens base surround a recessed optical plane; the 45/90 aperture
sizes, lens heights, connector details and unseen faces remain estimates or
UNKNOWN. Inspection moves the carrier away from the stationary underside
label and restores it exactly. Two ZIP inventories contain no verified CAD.
The publicly linked sensor PDF is marked confidential/NDA and is referenced,
not redistributed. Original photographs remain unmodified.

## ESP32-XX Basic Kit

SKU 33356 contains S3 Zero, 33357 contains C6 Zero, and 33358 contains C3 Zero.
SKU 33355 has accessories only and is excluded from the controller count.
The C6 kit has separate 8 MiB and source-documented 4 MiB profiles because
current linked documentation and older board material differ; its sale SKU
is counted only once and the shipping revision is not assumed.

Each kit reuses its exact included controller's board/model. The loose OLED,
PIR, buzzer, RGB module and breadboard parts are neither onboard devices nor
an automatically wired circuit. There is no invented kit PCB or full-kit 3D
layout. Pinned complete S3/C6 tutorial trees contain no matching complete
factory BIN; C3 candidates audited earlier identify as S3 and are rejected for
C3. Tutorial watch entries are manual references with no automatic targets.

## Verification and limits

- Six native board tests passed (three classic, three thermal).
- Release WASM build and the three original-image bounded attempts passed as
  executions, with the application stop points recorded above.
- Catalogue validation passed. Knowledge/manual-watchlist/registry checks
  passed all seven Node test entries over 582 knowledge records. The additional
  H2 public Worker and registry regression run passed three entries.
- Three new models and three reused controller models passed the actual
  production Three.js loader, source hashes, dimensions and applicable CPU
  ray/inspection checks. Four-view and source-overlay review receipts retain
  exact model/image hashes. The renderer's screen swatch is only a geometry
  aid; it is not a firmware screenshot.
- Host all-target Clippy and release WASM jit-tests Clippy passed with warnings
  denied. Earlier failures and final logs are retained. The fixes replace two
  needless lazy C5 defaults and add the omitted H2 reset match arm to an
  existing cfg(test) storage test; they do not add H2 storage/Flash execution.

`checks.json`, `visual-review.json`, per-image result/console records, model
checks and source manifests are the evidence. These are not real-browser
DOM/WebGL, complete factory app, loose-kit tutorial, RF, physical fit or real
hardware acceptance. Final all-workspace tests and CPU comparisons are
performed against the completed combined change, separately from this receipt.
