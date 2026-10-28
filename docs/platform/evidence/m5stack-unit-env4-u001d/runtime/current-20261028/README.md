# ENV-IV current runtime verification

Base: `5a9b68e0cd5aee1c6c51ca17c9fd2250a3c23fa7`. Implementation: `95fc5da2aeaaeec3b5d0ba84b4a6627a2b957f67` and `da61bbc1cd7fd76db67efae5d4e066cbae145515`. Earlier `d3c7820f` evidence remains in the parent directory with its original dates. No firmware image is added to the website, catalog or Git.

The migration keeps StopWatch's current shared internal G47/G48 I²C devices and RX8130, and appends external G10/G11 hardware-controller endpoints for ENV-IV 0x44/0x76. External software I²C is still absent. The cold Board input is appended without moving existing hot vtable slots. SHT40 now uses the shared Sht4x with an explicit D1 v2 profile; the default profile used by Air Quality retains its v7.3 timing and busy semantics. A dedicated test checks both timing boundaries and reset behavior. No S3 core, bus or generic tick traversal is changed.

`sources.json` contains fresh GET receipts for the product page, both SHT document versions, BMP280 and host/unit schematics. Four fixed PDFs retain the previous hashes. `open-pr-audit.json` identifies 17 open upstream PRs with no overlap in the shared paths checked before implementation. Manufacturer CAD/photograph sources remain fixed by the original model plan and source audit.

## Actual execution and checks

`fixture.json` identifies the production WASM, ROM, unchanged source-only IDF 5.5.1 fixture and all eleven checks. Inputs travel through the page worker, GPIO matrix, real hardware I²C controller, sensor commands and guest compensation. Wrong pins/addresses NACK; SHT conversion busy/CRC/read-once, M5PM1 power, reset, unplug/replug, both kinds of restart, one-level U006 and recording are exercised. The unchanged M5Unit-ENV compensation header independently computes BMP280 results. This is protocol fixture validation, not a claim that arbitrary official firmware or every sensor command works.

`browser.json` records actual human-control samples in Studio (18.500°C, 72.000%RH, 1004.999hPa) and restored mini configuration plus agent input (13.000°C, 41.000%RH, 851.000hPa). Both are observed guest UART values with successful CRC. The fixture intentionally produces no framebuffer. A full-page Studio screenshot has sticky-header scroll stitching, so it is not used to claim layout fidelity. Captures remain ignored locally; their hashes and byte sizes identify them. Owned headless browser and server were closed; unrelated sessions were preserved.

`checks.json` records all 28 CI groups and separate catalog/fixture checks: 1,568 Rust tests including ignored/non-external, both Clippy commands with warnings denied, 104 platform tests, 11 standard WASM scenarios, recording and transport checks, 111,126 differential cases, 24 GPIO waveform cases, profiling and ARM checks. Goldens are unchanged. Local raw logs are not published; hashes retain their identity without personal paths. `initial-results.json` preserves harness failures and corrections; none was fixed by weakening emulator behavior.

## Model boundaries

Public reconstruction is byte-identical (`model-rebuild.json`). Actual 800×800 fixed-canvas front/back/connector views were recaptured, and `model-verification.json` reports ten dimension/source checks plus front silhouette IoU 0.976600 (minimum 0.95). Back 0.900470 and connector 0.661108 are qualitative-only because photo camera intrinsics are unknown. These are not complete 1:1 passes.

Original, gold and translucent-purple views keep paper and red Grove material independent. UNKNOWN PCB remains visible and explicitly estimated. The real shared plug enters the hollow socket; the stop and wire exit remain outside. Pin order, mirrored back-label interpretation and anchor matrix are retained in `browser.json`. Insertion depth 3mm and pin-row height 2.85mm remain photo estimates. Related ENV-III shell reuse, missing rear screw, internal layout and connector detail limitations are unchanged; no geometry was guessed to inflate acceptance.

## CPU guard

`baseline.json` fixes an independent `git archive 5a9b68e0` release build with Rust 1.99.1; it is not an unverified pre-existing target directory. `cpu.json` contains one excluded warmup then five alternating measured pairs per hello workload, identical input hashes, instruction counts and stdout hashes. S3 median is 0.26→0.26s; C3 2.49→2.59s (+4.016%, 5/5 pairs slower); C6 1.70→1.65s (−2.941%). C3 attribution is unresolved. Other tasks paused guest/build/browser work for this run; unrelated host load and 0.01s timer resolution still limit inference. No speed or no-regression claim, no unchanged-candidate repeat. Historical Pocket measurements remain separate; this migration changes only optional board state and cold input, so it does not repeat S3-core workloads.

## Reproduction

Run from repository root with documented Rust, ROM and model Python dependencies installed. Build the fixture using `tools/fixtures/unit-env4-firmware/README.md`; its binaries remain in ignored `output/unit-env4-build`.

```sh
bash docs/platform/evidence/m5stack-unit-env4-u001d/runtime/current-20261028/checks.sh
node tools/probe-unit-env4.mjs --build output/unit-env4-build
python tools/validate-platform-catalog.py
python tools/rebuild-unit-library.py m5stack-unit-env4-u001d --scratch output/env4-rebuild
python tools/verify-unit-env4-model.py --renders output/playwright --evidence-dir output/env4-verification
python docs/platform/evidence/m5stack-unit-env4-u001d/runtime/current-20261028/bench.py --base-bins <independently-built-baseline>/target/release --candidate-bins target/release --rom-dir web/wasm/fw --receipt output/env4-cpu.json
```

For model captures, serve `web/` at a caller-selected local port, open `platform/review-lane-01.html?id=m5stack-unit-env4-u001d`, select the model's committed review views and mask pass, and place the actual canvas at an integer origin with 800×800 CSS dimensions. Use `env4-{front,back,connector}.png` and `env4-{front,back,connector}-mask.png`. Do not rescale screenshots to satisfy validation. Configure the accessory through the documented API, upload the local fixture bundle through Studio, and repeat the recorded ambient inputs in Studio and an independently restored mini URL. Close owned browser sessions when done. No physical hardware test was performed.
