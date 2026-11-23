# Publication WASM regression — original twelve Seeed devices

This is a fresh execution of the twelve-device plan using production WASM SHA-256
a3f0c90c47f9403225084c7138a3ec6c6b7ab5d2d9a0d1e6d3db8938a7379a75 (6,056,179 bytes). The actual platform loader, worker and WASM ran under Node. No firmware bytes were patched, no application-direct shortcut or stubs were used, and every run started with isolated storage.

See [receipt.json](receipt.json) for all 12 attempts, 16 input actions, 29 capture attempts, exact source/initial-Flash hashes, guest sampling times, instruction counts and per-frame comparisons. [source-checks.json](source-checks.json) records frozen executable/worker/probe hashes and the before/after source checks. The comparison baseline is the [earlier 1002b80d checkpoint](../seeed-final-wasm-20261122/README.md), retained unchanged.

## Results

12 harness processes exited zero; 16 input actions; 29 capture attempts; 26 exact RGBA matches; three preserved NO_FRAME results. Four devices pass the listed display/input subset, three show UI with incomplete workflow, and five provide no usable default UI in the specified window.

All 26 captured RGBA files match their recorded hashes and the earlier checkpoint. The three NO_FRAME results also match. This includes intentional backlight-off test images as well as the unusable white C3 and Sticky startup frames; matching pixels does not make those negative cases pass.

| Product | Input actions | Capture attempts | Observed category |
| --- | ---: | ---: | --- |
| seeed-xiao-ips-096 | 3 | 4 | display-and-listed-input-subset |
| seeed-xiao-ips-114 | 4 | 5 | display-and-listed-input-subset |
| seeed-xiao-ips-147 | 4 | 6 | display-and-listed-input-subset |
| seeed-sensecap-indicator | 1 | 2 | display-and-listed-input-subset |
| seeed-sensecap-watcher | 3 | 4 | visible-ui-incomplete-workflow |
| seeed-trmnl-75-diy | 1 | 2 | visible-ui-incomplete-workflow |
| seeed-xiao-epaper-75 | 0 | 1 | no-usable-default-ui-in-window |
| seeed-reterminal-e1001 | 0 | 1 | no-usable-default-ui-in-window |
| seeed-reterminal-e1002 | 0 | 1 | no-usable-default-ui-in-window |
| seeed-reterminal-e1003 | 0 | 1 | visible-ui-incomplete-workflow |
| seeed-reterminal-e1004 | 0 | 1 | no-usable-default-ui-in-window |
| seeed-reterminal-sticky | 0 | 1 | no-usable-default-ui-in-window |

The three IPS devices reproduce the listed display/backlight/button/touch behavior. Indicator reproduces the WiFi-list-to-password-keyboard touch step. Watcher shows its welcome page, but the three tested touch/encoder/knob events leave it unchanged. TRMNL shows provisioning, but the tested D4 press leaves it unchanged. E1003 reproduces the original low-battery page; normal UI, PMIC and touch remain unverified. E1001/E1002/E1004 publish no frame in these default-startup windows. C3 remains white after an RTC watchdog reset; Sticky remains white with storage/font-loading failures. These exact negative observations are retained in the receipt.

## Reproduction and privacy

Use the linked historical run plan and its unchanged input fixtures, the tracked tools/probe-seeed-runtime.mjs, official artifact hashes in each run, a caller-supplied local URL map and ESP32SIM_ROM_DIR. Each run records a command with directory placeholders. The harness was serial and checked WASM, worker and probe before and after every attempt. The additional 120 runtime/catalog files and all 11 private runner/plan/map/input files matched before/after; they were not individually frozen per instruction or at every run boundary.

No new PNG was created. The receipt links only the five previously reviewed PNGs after checking their compressed hashes and decoded RGBA against the fresh captures. The TRMNL device SSID/QR image, raw serial, network state, storage keys and absolute paths remain private. Public SHA-256 values identify the originals without publishing their contents.

This is not a browser, hardware/RF, complete workflow or host-performance acceptance result. It does not rerun the EE combinations or new SenseCAP commercial variants. See the [final integration record](../seeed-final-integration-20261122/README.md) for their separate evidence.
