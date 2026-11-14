# Published Mosaico check, 2026-11-14

Test URL: <https://zhuzhe1983.github.io/esp-online-tester/platform/>. A fresh headless Chromium session selected **厂家 → Espressif** through the visible picker. No scene was injected and no HTTP response was mocked. The repository revision inspected alongside this check was `65fa567109b5c2358152a745e33804778f15d7da`.

The workbench screenshot (`workbench.png` in the [screenshot manifest](screenshots.json)) shows the source-derived white-shell 3D model and the selected official Hello 0.1.3 + Vibe Mode 0.3.0 recovery entry. The picker also offered Xiaozhi v2.6.0. `#scene-mount.dataset.modelStatus` was `ready`. Requests for `platform/assets/espressif-esp-mosaico/model.json`, its hash-qualified `geometry.bin`, `rear-brand.svg` and `rear-pad-labels.svg` all returned HTTP 200 under the repository subpath. No model-loading error appeared. The empty dark display before starting is the idle framebuffer, not a missing model.

**下载并尝试运行 → 继续运行** encountered the publisher's missing `Access-Control-Allow-Origin` header. The recovery dialog (`download-recovery.png` in the [screenshot manifest](screenshots.json)) exposed the original URL, BIN picker and pinned hash. Importing the original 16,777,216-byte BIN through that dialog passed SHA-256 verification and resumed the same selected source:

- Image: `https://mosaico.ai/api/v1/firmware/4/download`, SHA-256 `7562ff67cad8280fe8fdd06f3be7d6c91f44e5a8cc63fe596721f48da428af9b`.
- ROM: `https://zhuzhe1983.github.io/esp-online-tester/wasm/fw/esp32s31_rev0_rom.elf`, 513,396 bytes, SHA-256 `a52ad7513deb656a910a5740125f1cce2c7941f11ce57213b7b43aea93d5ab87`.
- Runtime reported `wasm-interpreter`, `appDirect:false`, no stubs, `status:running`, `lastError:null`, and a 480×480 framebuffer.
- Serial reported `UI live: 1 scene(s), 480x480`; the 3D boot frame (`boot-3d.png` in the [screenshot manifest](screenshots.json)) displayed “Choose Your Language”.
- Switching to 2D and physically clicking its Chinese-language region near guest `(350,300)` produced the Chinese homepage. Switching back to 3D displayed that real guest output (`chinese-3d.png` in the [screenshot manifest](screenshots.json)). The observation reached frame 75; this is not a performance measurement.

This uses explicitly imported browser cache bytes. The retained `source:remote` receipt preserves the original URL/hash and **does not mean the publisher download succeeded**. A new recipient can still need the BIN. No firmware bytes are committed or uploaded.

The separate older `https://zhuzhe1983.github.io/esp-online-tester/run.html?wasm` page showed its upstream demo selector and local firmware configuration, with no Mosaico board option, product firmware library or 3D product view. This is a verified entry-point difference, not proof that a reported missing model came from that URL. An old cache/tab was not reproduced or diagnosed by this clean-session check.

Only the two expected publisher CORS console errors appeared on the product page. Browser screenshots contain the public UI; local source paths, profiles, device identifiers and raw console exports are omitted. The task-owned browser session was closed and its exit verified. The workbench reproduction above requires no local server.

## Legacy navigation change

The old page now has an explicit **Product workbench** header link, plus **Product workbench · 3D models and firmware library** in the configuration dialog that opens on first visit. Both use `./platform/`; they do not redirect the legacy page or transfer an old demo selection. The legacy page has English text and no locale module, so the new labels follow its existing language and styles.

A separate headless browser served the changed tree locally under `/esp-online-tester/`. Clicking the dialog link, the normal header link, and the `?wasm&plain` header link each reached `/esp-online-tester/platform/`. Selecting Espressif there gave `modelStatus:ready`, `espressif-esp-mosaico-factory`, and the Mosaico V1.2 title. These are local checks of the changed navigation, distinct from the published-site check above. Dialog (`legacy-dialog.png` in the [screenshot manifest](screenshots.json)) and header (`legacy-header.png` in the [screenshot manifest](screenshots.json)) screenshots show the links. Root-hosted and GitHub Pages URL resolution was also checked using the actual anchor href. No firmware was loaded during this navigation check.

Validation: `node --test tools/stopwatch-ui-test.mjs tools/camera-ui-test.mjs tools/platform-analytics-test.mjs` passed all 8 tests; `node --check web/workbench.js` and `git diff --check` passed. The initial local checkout lacked ignored xterm assets; after supplying the existing local vendor files, the navigation checks had no page errors. Its browser session was closed and absent from the session list; the preview server exited normally. This navigation change addresses a discoverability gap without attributing an unconfirmed report to the old URL.

## Screenshot retention

Full browser screenshots are retained unchanged under ignored `output/evidence/mosaico-live-20261114/`, following [the evidence policy](../../../evidence/README.md). [screenshots.json](screenshots.json) records each original name, byte size, SHA-256, source revision and local path, plus the time its local copy was verified. The same manifest accompanies the local originals. No screenshot was re-encoded or cropped during this cleanup, and no capture was uploaded to a release or external evidence repository. This committed report preserves the observations and reproduction steps; viewing the original screenshots requires the retained local output directory.
