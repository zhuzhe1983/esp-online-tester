# Seeed integration checks — 2026-11-22

This is the historical first-stage integration snapshot for the original 13 Seeed devices. Its test counts, catalog totals and measured artifact hashes describe that checkpoint, not the later expanded tree. See the [48-model expanded report](../seeed-expanded-models/README.md) and the [final integration report](../seeed-final-integration-20261122/README.md) for subsequent model verification and integrated gates.

This directory records the final native workspace gate. Related, independently
bounded evidence is linked below. A successful test is not a declaration that
every peripheral or every factory workflow is emulated.

## Completed checks

| Check | Result | Evidence |
| --- | --- | --- |
| Workspace Clippy, all targets, warnings denied | Exit 0 after the integrated `SpiPins` test literal was corrected | `cargo clippy --workspace --all-targets -- -D warnings` |
| WASM Clippy with `jit-tests`, warnings denied | Exit 0 | `cargo clippy --release --target wasm32-unknown-unknown -p esp32sim-wasm --features jit-tests -- -D warnings` |
| Release workspace tests, including ignored hermetic tests | **2,274 passed, 0 failed, 0 ignored; 95 external-fixture tests filtered** | [Receipt and changed Rust source hashes](rust-tests.json); [complete sanitized log](full-rust-tests.log.gz) |
| Production WASM build and sections | Exit 0; 5,998,680 bytes, SHA-256 `9089bd6c056e2c3752b4f04db2b7053e640aa98039c3ddc455b6a5273621f03f` | [Node gate receipt](../seeed-node-gates/receipt.json) |
| 99 platform test files | Initial 430/431 pass; missing English labels corrected; the affected i18n file then passes 9/9 | [Full and targeted logs](../seeed-node-gates/receipt.json) |
| Eleven additional Node/worker/ABI gates | All pass: real recording, worker pacing/channel, touch, StopWatch UI/WASM/diagnostics, production sections, timing, BLE ABI/connection | [Node gate receipt](../seeed-node-gates/receipt.json) |
| Existing WASM firmware regression | 14/14 manifests plus JIT handoff pass; 65 input artifacts hashed | [Regression receipt](../seeed-wasm-regression-20261122/receipt.json) |
| Catalog schema and source references | 232 products, 10 accessories, 197 firmwares, 130 watchlist entries; 240 knowledge records; 13 Seeed entries and 117 original source bodies pass | `validate-platform-catalog.py`, `platform-knowledge-test.mjs`, `platform-seeed-catalog-test.mjs --source-dir` |
| Seeed model verification | All 13 models, 48 retained sources, generated artifact hashes and front-facing screen samples pass | [Final model checks](../seeed-models/checks.json) |
| Model loader after source-metadata minimization | Both targeted Node tests pass | `node --test tools/platform-model-test.mjs tools/platform-manufacturer-model-test.mjs` |
| Evidence privacy | Existing evidence gate passes; Seeed-specific files, gzip/ZIP members and image metadata separately reviewed | [Metadata privacy review](../seeed-models/metadata-privacy.json) |

The platform suite was not repeated in full after the isolated translation
correction; the successful 430 tests and the 9-test targeted rerun are retained
as separate facts. Source-metadata minimization changed no image pixels, CAD
or model geometry. The model verification was repeated after that change.

Both Clippy commands and the production build were observed to exit zero;
their original complete console transcripts were not archived. The release
test log is archived in full after the documented path/MAC redactions. Cargo
also emitted the existing warning that the C5 and S31 `bringup` examples share
output filenames. A passing gate is not a claim that Cargo printed no warnings.

## Actual device execution

- [Seven-device WASM checks](../../seeed-wasm-runtime.md): 7 ROM boots, 16 input
  actions and 24 captured frames. Successful inputs and unchanged frames remain
  separately identified.
- [Five reTerminal defaults](../../seeed-reterminal-runtime.md): five additional
  WASM ROM boots and guest TF mounts; three distinct native ADC=0 display
  fixtures. The fixtures do not replace the default-run failures.
- [D1001 diagnosis](../../seeed-d1001-blockers.md): the unchanged factory image
  is blocked in the P4 bootloader ADC entropy initialization. DSI/DMA/I2C work
  remains; this device is not declared runnable.

All third-party firmware remains a remote catalog source. No new third-party
firmware BIN was added to Git; the 13 new `geometry.bin` files are model meshes.

## Publication and provenance

The target remote `main` was re-read and remained at
`573f5a6c1bcc6dbf0388383d178a3935ec8ddcce`. The connected GitHub integration's
write attempt was rejected with HTTP 403, `Resource not accessible by
integration`; command-line Git had no usable HTTPS credentials. No remote
branch update or Pages deployment was completed by this integration.

Browser acceptance is also pending. Local Chromium could not create its
required socket, and the cloud browser could not access the workspace loopback
server. Software mesh renders and Node execution are not reported as browser
DOM/WebGL/CORS verification.

The reviewable publication tree is consolidated before publishing so superseded
unredacted source-metadata blobs are not included in its new Git ancestry.
Checkpoint commit IDs in measurement receipts describe **unpublished local
integration history**, not public release tags. Original measured artifact
hashes remain unchanged; the published files and their retained source hashes
are the basis for reproduction. Manufacturer and upstream source commit links
retain their original meanings.
