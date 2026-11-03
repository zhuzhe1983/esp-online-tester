StampFly browser runtime and output-panel checks passed in headless Chrome 156. The JSON receipt separates factory firmware execution from the internal source-built PWM fixture, and separates tested model metadata from the later UNKNOWN annotation repair.

The original factory image `0138cc333d05790f8804fabae223f4c7eb40d162ad1d23d8c8daead6412d4359` and v1.1 factory image `d903acc1815cb8b08224aa4e9a581bb23b69b606813f9edbf4af5002efda113b` were downloaded directly from their catalog URLs through the ordinary workbench trial-confirmation flow. Both printed sensor initialization, ESP-NOW readiness and `Enjoy Flight!` on USB. A physical keyboard A press and release produced a fresh `RTC_SW_CPU_RST`, followed by a second successful initialization. This was a guest software restart, not the host Reset command. Unpaired factory motor duties remained zero. The old v1.1 image left its G38-gated Stamp LED dark while the two carrier outputs remained active.

The separate `tools/fixtures/stampfly-firmware` ESP-IDF build was selected as four browser Files: `flasher_args.json`, `bootloader/bootloader.bin`, `partition_table/partition-table.bin`, and `stampfly_fixture.bin`. No binary was committed or made available through the public HTTP server. In both final product models, a pointer press on the G0 module cap reached guest GPIO and produced the next fixture output step: duties approximately 0.25, 0.375, 0.5 and 0.625, with the observed carrier frequency 150093.808630394 Hz. The panel displayed the same normalized duties and three RGB values as `observe()`.

For each model, two screenshots separated by at least 0.25 guest seconds changed with active PWM. After waiting for the actual paused state, two screenshots 450 host milliseconds apart remained geometrically unchanged and the guest cycle count stayed fixed. Original paused PNGs were identical. The v1.1 raster differed at one pixel by one blue-channel level, within the retained two-pixel/one-level raster tolerance; this is not treated as rotor movement. Host Reset emitted a clearing observation, restored four zero-duty/zero-frequency outputs, and cleared the RGB values. The panel remained within the 390-pixel workbench viewport.

The original pinned share URL restored an unloaded embed preview. Origin-bound parent RPC performed hello, load, start, serial wait, observation, pause and framebuffer capture; the factory booted and capture correctly returned `NO_FRAME`. A v1.1 pinned share restored the separate product identity. Its source-built fixture was passed as structured-cloned Files over the same bridge: G38 low hid only Stamp RGB, raising G38 without a new RMT frame kept it dark, and a fresh frame exposed the red value. Parent GPIO input changed actual PWM, and explicit BMI270 host orientation was read by the guest as 4096, 0, 0 at the configured 8 g range. The final embed panel matched actual output in a 390-pixel iframe. Switching both workbench and embed to DinMeter hid the output panel and its mount.

Reproduction uses the ordinary local server and the Playwright wrapper:

```sh
python3 - <<'PY'
from http.server import ThreadingHTTPServer, SimpleHTTPRequestHandler
ThreadingHTTPServer.request_queue_size = 128
ThreadingHTTPServer(('127.0.0.1', 8788), SimpleHTTPRequestHandler).serve_forever()
PY
~/.codex/skills/playwright/scripts/playwright_cli.sh -s stampfly-browser-check open http://127.0.0.1:8788/web/platform/
```

Use the supplied source-built fixture and the product's current catalog-pinned factory source. Preserve distinct factory and fixture receipts. For button assertions, register a fresh serial wait before pressing and release the input in a `finally` block. Await lifecycle state acknowledgments after UI clicks, wait for PWM/RGB publication independently of serial output, and recompute pointer coordinates after scrolling. Apply a duty tolerance of 1/65535 because the board observation uses the routed output representation. Select the module cap only after the real hover classifier reports `button`. Use actual `#scene-mount` screenshots, not a synthetic rotor harness. The JSON records viewport-relative pointer coordinates, fresh input receipts and measured output values.

The initial local Python preview server reset several concurrent JavaScript module connections. Reloading and increasing `ThreadingHTTPServer.request_queue_size` to 128 resolved the serving condition; no application source changed for it. Test-owned browser resources were closed and verified absent. The local preview server remained available for integration checks.

This receipt does not establish physical-hardware compatibility, ESP-NOW pairing, flight dynamics, thrust, calibrated RPM, calibrated carrier LED positions, or operation of the secondary carrier button surface. The annotation-only amendment at the end of the JSON identifies the final UNKNOWN marker metadata and font changes; its visibility is accepted by a separate model review, not inferred from these runtime tests.

The final annotation review in [annotation-review.json](annotation-review.json) verifies both models through the public viewer with a flipped orientation and maximum wheel zoom. The original label sits inside its actual 4×3 mm source face beside the installed battery. Its previous tail and carrier-plane positions were occluded. The generic UNKNOWN font increased from 68 to 100 pixels within the same 512×128 canvas; retained runtime screenshots predate this text-only renderer amendment. The independent current screenshots and exact renderer/model hashes cover that amendment.
