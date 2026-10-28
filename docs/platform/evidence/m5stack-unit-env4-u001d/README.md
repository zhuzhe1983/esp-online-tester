# U001-D ENV-IV evidence

This is the SHT40-AD1B + BMP280 ENV-IV, not ENV-III or ENV Pro. See the [runtime scope](../../unit-env4-runtime.md). All source URLs, actual fetched UTC seconds, final URLs, MIME types, sizes and SHA-256 are retained in `source-audit.json` and `runtime/sources.json`.

## Reproduce

1. Build the source-only `tools/fixtures/unit-env4-firmware` with ESP-IDF5.5.1. Its independent BMP280 compensation header is unchanged from M5Unit-ENV `28c521a137cb6696f4860cdce93ca7df9ce9a6af`; MIT license retained. Binaries remain in ignored output.
2. `tools/wasm-build.sh`; provision pinned ROMs as documented. Run `node tools/probe-unit-env4.mjs --build output/unit-env4-build`. `runtime/fixture.json` contains actual page-worker/WASM hashes, input hashes and11 checks; expected values are independently computed by guest code.
3. `node --test tools/platform-*-test.mjs`; `python tools/validate-platform-catalog.py`; both Clippy commands and full Rust checks use stable1.99.1, including ignored tests but excluding external-input tests.
4. `python tools/rebuild-unit-library.py m5stack-unit-env4-u001d --scratch output/env4-rebuild`. Every generated asset including model metadata is byte-identical (`runtime/model-rebuild.json`). No private source directory or network is needed.
5. Serve `web/`. Open `platform/review-lane-01.html?id=m5stack-unit-env4-u001d`. Capture the actual renderer at800×800 for front/back/connector with the committed camera views, and its `laneReview.mask()` black pass. Run `python tools/verify-unit-env4-model.py --renders output/playwright`; expected names are `env4-{view}.png` and `env4-{view}-mask.png`.

## Meaning of the checks

The raw manufacturer CAD is explicitly **ENV-III**. Native triangles are retained; only duplicate print components are omitted and XY is centered. ENV-IV's independent drawing confirms24×32×8mm,5mm mounting holes,16mm spacing and12mm hole-to-bottom distance. Front outer silhouette IoU0.9766 passes0.95. Back and connector comparisons use an approximate orthographic pose against perspective photos; their metrics are retained but **not quantitative acceptance**. The ENV-IV label texture uses the same back photo, so that region is not independent evidence. No complete internal1:1 or manufacturing-fit claim.

The PCB is visibly marked UNKNOWN and simplified. Original/gold/translucent-purple material checks preserve white paper and red socket. `runtime/plug.png` shows the actual shared plug nose inside the hollow socket; hidden depth is approximate. The back label fixes per-pin mirror order; the5.2mm drawing mark describes the casing seam, not pin-row height. Pin-row z≈2.85mm is an image estimate. Rear screw and actual PCB layout remain absent.

`runtime/browser.json` retains observed UART0 values from human numeric controls and an independently restored mini scene. Shared URLs carry configuration; local firmware bytes were supplied separately in memory. No firmware was published, flashed or sent to an external service. Each task browser was closed. Captured screenshots contain only public catalog/fixture data; no private desktop or process inventory is retained.

## Retained negative results

- The initial source fixture used one10ms FreeRTOS tick as a sensor wait. Depending on tick phase it was shorter than the8.2ms SHT40 conversion and correctly NACKed. Two ticks fix the test; emulator timing was not relaxed.
- Immediate post-reconnect reads also correctly NACKed during1ms startup. The harness waits3ms of guest time before a normal sample, while still explicitly checking early-read rejection.
- ESP-IDF uses negative `ESP_FAIL`; the initial probe regex wrongly required nonnegative errors. The expected NACK was observed, and the parser was corrected.
- The old StopWatch unit test assumed the device list contained only four internal chips. It now separately checks those unchanged unpinned endpoints and all four optional, pinned, initially silent ENV endpoints. This is an intentional device-list extension, not a golden change.
- One initial full-suite run lacked C6-hosted inputs in the new worktree. The existing fetch script verified the copied pinned inputs before rerunning. No missing-input test was hidden.

Numeric/source details for the first three are in `runtime/initial-results.json`. Build logs remain local; public receipts contain normalized commands, outcomes and hashes rather than personal paths. Native CPU measurements are in `runtime/cpu.json`; previous U027 C6 and U003 Pocket costs remain in their original EX201/EX219 evidence.

Five measured alternating CPU pairs after an excluded warmup pair use retained Rust-identical `53400cbf` binaries for base `be5291d7`. S3/C3 hello medians are unchanged; C6 rises1.67→1.72s (+2.994%,4/5 slower), Pocket41.45→41.51s (+0.145%,2/5). Instructions and stdout hashes match for every pair. C6 attribution remains unresolved; no no-regression claim and no unchanged-candidate repeats. Optional ENV state is lazily allocated in StopWatch’s existing panel; no generic Machine layout field or unconditional additional tick traversal was added. Board method/vtable and compiled layout still change, so source-level coldness alone does not prove zero CPU cost.
