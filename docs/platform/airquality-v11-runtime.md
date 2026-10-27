# Air Quality v1.1 K131-V11 bring-up

This separate hardware identity depends on the original K131 digital-model implementation at 006573f0. Sources are fixed in [sources.json](evidence/airquality-v11/sources.json); manufacturer CAD and photo receipts remain under the exact v1.1 knowledge/model entries.

The current official v1.1 page links the same AirQ v1.0 carrier schematic, GDEY0154D67 datasheet and AirQUserDemo factory source as the original SKU. Explicit display routes are BUSY1/RST2/DC3/CS4/SCK5/MOSI6; buttons are A0/B8, sensors SDA11/SCL12, supply G10 and power HOLD46/WAKE42. These identical documented carrier connections permit reuse of the digital SSD1681/SCD40/SEN55 implementation. They do not establish equal whole-device behavior.

The master module changes to Stamp-S3A. Its RGB supply requires GPIO38 HIGH, while the old Stamp-S3 does not have this gate. RGB/RMT and its supply behavior are not implemented in this acceptance slice; they must not inherit an always-powered old-module LED. RF/antenna improvements also do not imply network simulation. RTC, battery, audio, external Grove, sleep and supply latch retain the original explicitly unsupported scope.

The official factory catalog points at the shared v0.2.0 complete ROM image, SHA-256 `ac47e80ba0b0e6c304e90ba10aa978f367d4369a2e449c1f4a58813a671b3216`. The source-only checkpoint b1d2550d remains in history. The implementation now selects a separate `airquality-v11` / `m5stack-k131-v11` board name; existing K131 identity remains unchanged.

The v1.1 official product page explicitly links the same AirQ UIFlow2 tutorial. The official Burner project 1897200000000002050 identifies AIR_QUALITY and fixed version v2.5.3; its existing catalog/watchlist target now includes v1.1 without changing factory-first selection. UIFlow2 remains untested trial. The mapping is documented source reuse, not an execution or whole-revision compatibility claim. No exact AirQ Xiaozhi image was established in this review.


## Actual factory acceptance

The unchanged official v0.2.0 image boots through the native S3 ROM and production WASM with 8 MiB Flash and no PSRAM. It reaches the actual 200×200 sensor dashboard; B opens Settings and then AP setup. These are actual guest pixels, not product-photo content. See [native](evidence/airquality-v11/native.json), [WASM](evidence/airquality-v11/official-wasm.json) and [sample page](evidence/airquality-v11/official-samples.png). The base digital-sensor limits, including fixed virtual samples and abstracted completion timings, remain in [original K131 scope](airquality-runtime.md).

Headless workbench testing uses the catalog's first/default factory choice, then its real trial confirmation. At 1440×1000, model coordinates (640,350) and (640,322) hit lower B and upper A; green hover, mouse B→A and keyboard B produce six correctly named recording events. Replay reaches the identical Settings framebuffer `c64bd09a0ec2e34723ace59182661e8b6bbca04ed62ffa05b1df248d39f34e4d`. Scheduling is not cycle exact: final accumulated lateness was 0.1734 guest seconds. The first frame20 capture was transitional; after A returns, the completed dashboard matches the native/worker route. [Recording receipt](evidence/airquality-v11/browser-recording.json).

A shared mini scene passed its own human trial confirmation, 2D/3D switching and B→Settings, with the same framebuffer hash. Factory boot writes NVS; only the task's isolated test storage was explicitly cleared before recording/replay/mini checks, never user storage or a bypassed pristine guard. [Mini receipt](evidence/airquality-v11/mini.json) and [actual mini frame](evidence/airquality-v11/mini-samples.png). Both task-owned browser and local test server were closed. Raw UART and Settings/AP screenshots are omitted because they include a device identifier; hashes/timings are retained and local test origin is normalized.

## Geometry review

The v1.1 model rebuilds nine existing assets and model JSON exactly. Manufacturer triangles and printed-paper crops remain unchanged. The prior screen center `[8.5,15,13.3]` mm was about 3 mm too high, leaving a lower window gap with actual firmware; the exact v1.1 front photo and native CAD window support `[9,12,13.3]` mm (±0.6 mm placement estimate). The corrected guest display fills the window. Added invisible upper/lower hit planes sit over the existing plastic flexures, without adding or recoloring button geometry.

Original color, opaque gold and transparent purple were checked front/back, plus a side view; printed labels remain separate from case material. The 56×72×26.5 mm body matches documented v1.1 dimensions. Historical silhouette front 0.9444 still fails the 0.95 threshold; back 0.9899 passes. The UNKNOWN placeholder label is partially hidden by the opaque screen in the assembled transparent front view; it does not establish populated PCB accuracy. Four clip placements include photograph-derived assembly estimates, and some connectors/fasteners/etching remain incomplete. See [review](evidence/airquality-v11/model-review.json), [views](evidence/airquality-v11/model-review.webp), and [rebuild](evidence/airquality-v11/model-rebuild.json); no whole-product 1:1 claim.

## Reproduction and regression

```sh
tools/wasm-build.sh
node tools/probe-airquality-official.mjs <fixed-official-https-url> <sha256> m5stack-k131-v11
target/release/esp32sim --board airquality-v11 --flash-image <official-image> \
  --boot rom --rom web/wasm/fw/esp32s3_rev0_rom.elf --flash-mb 8 --psram-mb 0 \
  --max-seconds 20 --tft-png <output.png> --no-dump
python tools/rebuild-product-model.py build m5stack-k131-v11 --scratch <scratch-directory>
```

[Checks](evidence/airquality-v11/checks.json): current stable Rust 1.99.1, both warning-denying Clippy commands, 1,439 Rust tests/132 groups, 11 production WASM scenarios, 100 platform tests, schema, real recording, timing/BLE/worker checks, 111,126 WASM JIT cases and 24 GPIO waveform cases passed. Initial missing ignored ROM and missing mapping-amendment evidence were corrected, then the affected full suites rerun; no golden regeneration or test weakening. No CPU/bus/S3-core hot path changed in this variant slice.
