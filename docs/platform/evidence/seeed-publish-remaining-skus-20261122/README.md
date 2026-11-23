# W1-A and D1S: independent publication-WASM observations

These two commercial products were each configured and ROM-booted independently using production WASM `a3f0c90c47f9403225084c7138a3ec6c6b7ab5d2d9a0d1e6d3db8938a7379a75` (6,056,179 bytes). Both probe processes exited zero, without a timeout, application-direct boot, guest stubs or modified firmware bytes. [receipts.json](receipts.json) preserves the distinct product/default-firmware IDs, original artifact hashes, per-run observations and source checks.

## Product identities and shared sources

- **SenseCAP Indicator D1S — SKU 114993069.** The independent catalog identity is `seeed-sensecap-indicator-d1s`, with default firmware `seeed-sensecap-indicator-d1s-factory`. Its actual source is the original official v1.0.0 merged BIN at offset `0x0`, SHA-256 `749c9b77222405e2b2ffc1a08f3d51a0752e762e10b6bba6c426f5440d638cf2`; it does not have a separate manifest file. The official [family native-firmware guide](https://wiki.seeedstudio.com/SenseCAP_Indicator_Native_Firmware/) describes the common UI. This run uses the D1S product ID and its configured GX board independently.
- **SenseCAP Watcher W1-A — SKU 113991315.** Its independent ID is `seeed-sensecap-watcher-w1-a`, with default firmware `seeed-sensecap-watcher-w1-a-factory`. It selects the unchanged repository [family recovery manifest](../../../../web/platform/firmware-plans/seeed-sensecap-watcher-factory.json), whose existing name remains “SenseCAP Watcher W1-B AI Vision factory firmware.” That label was not renamed into a new image. The six exact official 1.1.7 source files, flash addresses and hashes are retained in the receipt. The [official W1-A/W1-B selection table](https://www.seeed.cc/product/sensecap-watcher-the-physical-ai-agent-for-smarter-spaces) documents their common main hardware and dimensions with different enclosure materials; its fetched source identity is recorded. The new execution, not a common shell or previous W1-B result, provides the W1-A observation.

## Actual observations

| Product | Final guest time | Input actions | Capture attempts | Result |
| --- | ---: | ---: | ---: | --- |
| D1S | 9.08 s | 1 | 2 | Wi-Fi list at 8.00 s; touch `(200,136)` opens the password keyboard and changes 205,305 pixels. |
| W1-A | 11.66 s | 3 | 4 | Welcome frame at 8.00 s; touch, encoder and knob deliveries are accepted, but all three later captures have zero changed pixels. |

All six captured RGBA files have correct dimensions and hashes, and all six are byte-identical to the corresponding separately executed D1/W1-B family-reference captures from the [same production WASM](../seeed-publish-wasm-20261122/README.md). The receipt retains separate sampling times, instruction counts, storage product identities and raw receipt hashes. There were **zero NO_FRAME results** in this two-product batch.

D1S verifies only the observed display/touch navigation step. The synthetic access-point list does not establish network association or cloud behavior, and the RP2040/air-quality sensors remain unverified. This D1S configuration has no LoRa extension. W1-A verifies a visible factory welcome page; queued input acceptance is not a confirmed application callback or successful navigation. Its factory identity and SPIFFS errors remain explicit. Neither product is upgraded from partial support or described as hardware-validated.

## Input, artifact and privacy checks

The probe uses the unchanged existing input fixtures: D1S holds the touch for 0.2 guest seconds and requests 0.8 seconds to settle; W1-A applies the welcome touch, one encoder-step request and a 0.2-second knob press. Both start with an 8-second observation request. Every returned guest time is retained instead of pretending asynchronous polling hits the deadline exactly.

The harness checked the expected WASM, worker `f2903f82105003582d2ccde163c3dbfbc1c61d980c542fd9c0a5a69803af960d`, and probe `8cf822899f24ebff4a6f5295dee5f2c66f19fd32b67f05e6acedfa8518b34429` before and after each run. All **127** source/catalog/manifest/input file hashes also match before and after each run. Curation rechecked six current RGBA files, six reference RGBA files, six per-capture serial files, two final serial files and all eight distinct loaded firmware/ROM source files. Changed-pixel counts were recomputed without rotation, resampling or tolerance.

Each receipt contains a reproduction command using the tracked `tools/probe-seeed-runtime.mjs`, the existing committed input/network fixtures, caller-supplied `ESP32SIM_ROM_DIR` and local source-map/output placeholders. The selected official hashes remain enforced. No source, catalog, manifest, image bytes or compatibility flag was changed.

Raw serial, network/router state, storage keys, absolute local paths and framebuffer bodies remain private. Only content hashes, requested safe input payloads, observed numeric values and selected nonsensitive guest markers are published. No PNG was added. These are actual Node worker/WASM observations, not a browser, physical-device, RF or complete application-workflow acceptance result.
