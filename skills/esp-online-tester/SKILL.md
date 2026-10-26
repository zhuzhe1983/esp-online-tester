---
name: esp-online-tester
description: Configure, operate and test ESP32 product scenes in ESP Online Tester through its browser automation API or embedded iframe bridge. Use for device/firmware/accessory selection, reproducible input checks, framebuffer capture and share links; also for declarative catalog contributions and private DIY accessories.
---

# ESP Online Tester

Use the running platform at `web/platform/index.html` (workbench) or `web/platform/embed.html` (embedded view). The host serves static assets; firmware executes in the browser's WASM worker. Do not start a separate remote emulator or assume that a 3D preview proves firmware execution.

Read [the API reference](references/api.md) for commands and [configuration and catalogs](references/configuration.md) when constructing scenes or adding products/accessories/firmware.

## Establish the actual device state

Use the page's `window.espTester`, wait until it exists, then read `version`, `capabilities()` and `observe()`. Version `1.0.0` is implemented. Methods are asynchronous except `subscribe`; failures reject with `error.code`, `error.message` and sometimes `error.details`.

Preserve the user's selected model, accessory revision and firmware. StopWatch has a partial executable board model. PaperMono currently has metadata and a visual preview but no executable board model. The generic ESP32-S3 Hello fixture has serial output and no display. `configure()` success establishes configuration validity; it does not establish firmware compatibility.

For a user-provided firmware, use `{manifestUrl, sha256?}` or `{url, sha256?}`. A raw BIN must be a merged flash image starting at address 0, with a bootloader and the partition table at `0x8000`; app-only binaries need a manifest with their other segments. Cross-origin downloads require CORS. A GitHub repository page is not a binary URL. Do not silently substitute a demo when the requested firmware fails.

## Configure, operate, verify

1. Obtain the catalog via `import('./catalog.js')` relative to the platform page. Use `loadCatalog()` and `createDefaultScene(catalog, productId)`; validate with `validateScene(scene,catalog)` before `espTester.configure(scene)`. Inspect warnings, not just `valid`.
2. `await espTester.loadFirmware()` fetches the scene's source; `await espTester.start()` boots it. Check firmware-specific serial output, a real frame, or another observable condition with a bounded wait. A queued input acknowledgment is not a test pass.
3. Before an action, register a fresh observation: `const result = api.waitFor({serialIncludes:'BUTTON_A',fromNow:true,timeoutMs:15000}); await api.press('button-a'); await result;`. Otherwise old buffered output could make a new action appear successful.
4. Use `capture({format:'png'})` for actual device pixels. `NO_FRAME` means there is no firmware framebuffer; do not substitute the 3D case or a mock screenshot as device output.
5. Record the scene, `(await api.observe()).firmware` artifact receipt, assertions, errors and support limitations. `await api.share({mode:'embed'})` produces a portable scene URL. After local file loading, this API rejects sharing until the firmware is published at a browser-readable URL. The UI can export or share a configuration without local firmware bytes and labels that omission. Pause or dispose task-owned sessions when finished; close task-owned browser resources according to the repository's lifecycle instructions.

Use explicit timestamps and values for reproducible GPS tests. Choose `m5stack-gps-u032` (AT6558, default 9600) or `m5stack-gps-u032-v11` (AT6668, default 115200) from the actual hardware revision. The implemented GPS path requires StopWatch and `{mode:'logical',port:'uart1'}`. It injects checksummed NMEA into the emulator's UART1 input and bypasses physical pin routing, baud timing and external power. It does not certify wiring or physical GPS behavior.

The bundled `platform-stopwatch-diagnostic` is our public diagnostic firmware, not M5Stack's factory firmware. It reports `PLATFORM_READY stopwatch`, `BUTTON_A`, `BUTTON_B`, `TOUCH down` and `GPS_RX`. The metadata distinguishes simulated tests from hardware validation.

## Verification commands

In a local checkout, run `node tools/platform-config-test.mjs` after changing scene, catalog or private accessory handling. Runtime and browser tests are listed in the repository's platform documentation. Follow `AGENTS.md` before a pull request; do not infer that this config check replaces Rust or browser checks.

For third-party embedding, use the origin/session-bound `postMessage` protocol in [the API reference](references/api.md#embedded-parent-control). A cross-origin parent cannot read `iframe.contentWindow.espTester` directly.
