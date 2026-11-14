# Mosaico browser downloads and recovery

Checked 2026-11-14 UTC (2026-11-15 in Asia/Shanghai). This receipt covers the download/default-selection repair after `65fa5671`. It uses ordinary HTTPS and headless Chromium with browser security enabled. No firmware response was mocked, no local proxy supplied firmware, and no third-party BIN is committed or hosted by this site.

## Default: official Vibe recovery

[Espressif's recovery manifest](https://raw.githubusercontent.com/esp-mosaico/esp-mosaico-utils/c5688aec14fa943ecd3813338cf9b7030dc03734/firmware/recovery/manifest.json) identifies version `0.3.0`, profile `recovery`, product `esp-mosaico/v2`, and initial partition `vibe_mode`. The platform manifest preserves the publisher's four files and offsets; each remote URL is pinned to that repository commit and SHA-256. It adds the existing S31 mask ROM. This is Vibe-only recovery, **not** the complete Hello + Vibe image or the preinstalled Mosaico-Claw app. The complete Hello catalog entry keeps its factory role.

| Artifact | Flash offset | Bytes | SHA-256 |
| --- | --- | ---: | --- |
| bootloader.bin | 0x2000 | 23,456 | `02337c209b001e68552d7673b9bb6b2961d856dfad225d7a4ea53e395d22754b` |
| partition-table.bin | 0x8000 | 3,072 | `ff7ac35a4f2e51424150a6ba56172174299baa737230a09792dd46e8f5d0bb99` |
| ota_data_initial.bin | 0x9000 | 8,192 | `5a5d79a1c029da23a3b9cc03c111c5b72f9cbfa386877b0a5d20dc93ab42c7e1` |
| vibe-mode.bin | 0x20000 | 1,743,056 | `603f192496eb488c26a6140183cd208557e2fae6bdc152d74d11375e8d74c58a` |

The local manifest SHA-256 is `8333a72bd984224f704622f2604b4f00491afd37bc25a88215c5213af2cf8931`. The publisher manifest is 1,741 bytes, SHA-256 `7d75664979a2b9f5da859303a223bacfe3c3a737115f8952c55cad6601b50692`.

[pages-fetch.json](pages-fetch.json) records actual `fetch` from the deployed Pages origin with `credentials: omit`, `cache: no-store`: all five Raw resources returned readable HTTP 200 and matching hashes. [http-candidates.json](http-candidates.json) independently records `Access-Control-Allow-Origin: *`. Browser Resource Timing's zero transfer size is not treated as a cache hit or performance measurement.

On a fresh local preview with the same `/esp-online-tester/` subpath and final manifest, choosing Espressif/Mosaico selected Vibe automatically. Human trial confirmation downloaded the four remote segments and booted ROM → bootloader → Vibe through the production WASM worker, with no stubs or app-direct boot. [vibe-browser.json](vibe-browser.json) records the actual artifacts, serial markers and acknowledged input positions. Real 3D screen pointer input selected Chinese at `(348,308)`, then skipped Wi-Fi at `(422,49)`; the guest changed its framebuffer through language selection → Wi-Fi page → launcher.

Vibe language selection: `vibe-final-language.png` (local capture; see [screenshot manifest](screenshots.json)).
Chinese Wi-Fi page: `vibe-final-wifi.png` (local capture; see [screenshot manifest](screenshots.json)).
Vibe launcher after skip: `vibe-final-launcher.png` (local capture; see [screenshot manifest](screenshots.json)).

This establishes download, boot and those touch transitions. NAND LittleFS reported an uninitialized/corrupted directory pair and the guest reported an IDEA operation failure plus battery-read failure; those negative outputs are retained. Installing IDEA applications, cloud connectivity, complete audio and physical hardware operation are unverified. The screenshots retain the photo-derived model in use at capture time; they do not validate the later official-CAD geometry. No claim of portable recording replay or a fresh receiver's mini-player boot is made here.

## Sources that remain blocked

The full method/status/header/final-URL chains are in [http-candidates.json](http-candidates.json). Temporary object signatures are removed; URL hashes and expiry preserve redirect provenance. The signed object URL is never a permanent catalog source.

| Candidate | Observed result |
| --- | --- |
| Complete Hello `/api/v1/firmware/4/download` | HTTP 200, no ACAO; browser fetch rejected. The official firmware list and frontend use this same URL. |
| `?download=1`, `?raw=1`, `?redirect=true` | HTTP 200, no ACAO; no alternate public object URL exposed. |
| Old-domain API; OpenAPI/id endpoints | HTTP 404. |
| Official Hello IDEA `.irisfw` attachment | HTTP 200, no ACAO; a separate application package, not a complete recovery image. |
| Documented IDEA manifest query | HTTP 400 `INVALID_REQUEST`, no ACAO. No device registration or authentication workaround attempted. |
| Xiaozhi v2.6.0 GitHub Release ZIP | HTTP 302 → object HTTP 200, neither response grants browser CORS. |
| GitHub asset API, `Accept: application/octet-stream` | Earlier ordinary GET: API 302 with ACAO `*` → object 200 without ACAO; true browser rejected the final response. Later audit returned API 403 with ACAO `*`; both observations are retained. |
| Xiaozhi official download page | Redirects to its Feishu documentation; no usable public Mosaico object URL found there. |
| Existing configured `iot.mess.host` firmware relay | Actual Pages-origin read returned HTTP 403 `SOURCE_NOT_ALLOWED` for the ZIP. No service configuration or deployment was changed. |

The [GitHub asset API documentation](https://docs.github.com/en/rest/releases/assets) permits octet-stream asset requests but does not guarantee a CORS-readable final object. The public trees checked in `esp-mosaico-utils`, `esp-mosaico-vibe`, `esp-mosaico-bsp` and `esp-mosaico-claw` did not reveal a complete Hello mirror; the four recovery segments above are not a substitute for the missing Hello app. This is a bounded search, not a claim that no alternative source can exist.

## ZIP recovery regression

Xiaozhi remains a pinned trial option. Its Release ZIP is 3,131,826 bytes, SHA-256 `619e5a67f8c24bf934a8b62890dbd7eb054ebb1a9194855ae527af19dd343887`; its `merged-binary.bin` is 11,374,509 bytes, SHA-256 `440797fe7dbf952dc79126002e32e9a2d9ec4f2ad7752989ad74e7ed0dac9eb2`.

The recovery UI previously dropped the archive identity and cached an imported BIN under the raw ZIP URL. Runtime lookup uses `URL#zip=merged-binary.bin`, so retry downloaded the blocked ZIP again. Recovery now validates and retains the archive descriptor, writes the extracted BIN under that exact key, and explains how to extract the ZIP. Version, hash, chip, capacity and image-layout checks remain mandatory.

In the real browser, explicitly choosing Xiaozhi survived the new default logic. One Release fetch failed, the recovery dialog appeared, and selecting the publisher's extracted BIN then pressing **校验并继续** booted app `xiaozhi` version `2.6.0`, 480×480, through ROM/WASM. [xiaozhi-recovery.json](xiaozhi-recovery.json) records one failed ZIP resource, the populated archive-entry cache, frame 1224 and guest transition into `wifi_configuring`. It was paused for capture. The screen has clipped text and missing emoji; the guest disables camera after an expansion EEPROM failure. SoftAP/provisioning, network/cloud and voice are **not** accepted. This is a successful manual recovery, not direct download.

ZIP recovery instructions: `xiaozhi-zip-recovery.png` (local capture; see [screenshot manifest](screenshots.json)).
Xiaozhi after verified BIN recovery: `xiaozhi-recovered.png` (local capture; see [screenshot manifest](screenshots.json)).

The on-screen AP address is the guest's fixed virtual configuration address, not a reachable physical device. Screenshots contain no user credentials. Local preview origins in JSON are normalized to `https://platform.example`; no local paths or session identifiers are retained. [screenshots.json](screenshots.json) supplies final image hashes.

## Checks

```sh
node --test tools/platform-mosaico-layout-test.mjs tools/platform-catalog-selection-test.mjs tools/platform-firmware-recovery-test.mjs tools/platform-firmware-archive-test.mjs tools/platform-i18n-test.mjs tools/platform-config-test.mjs tools/platform-input-test.mjs tools/platform-view-test.mjs
node --test tools/platform-knowledge-test.mjs tools/platform-mosaico-model-test.mjs
python tools/validate-platform-catalog.py
node tools/check-evidence-privacy.mjs
git diff --check
```

The first command passed 35 tests; knowledge and existing Mosaico model tests passed. Schema validation passed 219 products, 10 accessories, 178 firmwares and 122 watchlist entries. The recovery regression covers both raw BIN and ZIP-entry retry and proves invalid images still fail before worker creation. Default-selection tests retain factory menu order/fallback, explicit defaults and unrelated products. Browser sessions were headless and task-owned; no physical hardware flashing occurred.

## Screenshot retention

Full browser screenshots are retained unchanged under ignored `output/evidence/mosaico-download-20261115/`, following [the evidence policy](../../../evidence/README.md). [screenshots.json](screenshots.json) records each original name, byte size, SHA-256, source revision and local path, plus the time its local copy was verified. The same manifest accompanies the local originals. No screenshot was re-encoded or cropped during this cleanup, and no capture was uploaded to a release or external evidence repository. This committed report preserves the observations and reproduction steps; viewing the original screenshots requires the retained local output directory.
