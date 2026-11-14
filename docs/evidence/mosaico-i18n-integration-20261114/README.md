# Mosaico and i18n integration, 2026-11-14

The upstream synchronization at `9f1ff87b` is followed by two independent changes: ESP-Mosaico V1.2 product/browser support and completion of the existing Chinese/English UI. Runtime integration is `f9c77c2b`; model/source integration and the official recovery metadata fix finish at `b94bae97`. Later commits add browser evidence, normalize preview origins and update current documentation without changing Rust or runtime execution.

## Behavior and conflict decisions

- Mosaico uses the production S31 machine, exact 16 MiB Flash/16 MiB PSRAM profile, correct S31 chip ID and bootloader offset. The unmodified official Hello 0.1.3 image boots and responds to touch, AI standby and wake. The [integrated worker receipt](worker-receipt.json) binds the image, ROM and WASM hashes to actual guest output. [Browser evidence](../../platform/evidence/mosaico-runtime-20261114/README.md) covers workbench, mini and configuration sharing.
- The official endpoint blocks direct browser CORS. Users can download the original BIN and select it in the existing SHA-256 recovery dialog. Sharing carries configuration and source identity, not locally selected bytes. Cross-runtime language retention, full cloud/voice/modules and physical timing are not accepted. The [photo-derived model](../../platform/mosaico-model.md) retains UNKNOWN internals and explicitly lacks manufacturer-CAD/whole-device dimensional acceptance.
- i18n fills device tools, storage, audio, configuration, errors, accessibility labels and control/interface names, including Mosaico. User names/SSID/firmware output remain opaque. Mobile language, TF format and shell choices no longer clip. [Screenshots and fixtures](../../platform/evidence/i18n-20261114/README.md) include iframe and language round trips.
- Feature branches merge without textual conflicts. Restoring pre-existing uncommitted NFC work in the primary working copy produces one shared capability-line conflict: retain both NFC and S31 serial-port fields. Every original tracked addition/deletion is unchanged apart from the shared S31 context; all nine untracked file hashes match. That unrelated work is not committed here. The earlier 56 upstream conflicts remain documented in the [upstream receipt](../upstream-sync-20261114/README.md).

The first full platform sweep caught fractional UTC timestamps and invalid watchlist paths/statuses in the new product data. Existing validation remains strict; the declarative schema registers the named Mosaico provider. A fixed-endpoint `espressif-mosaico` adapter reads actual recovery metadata without GitHub credentials, firmware payloads, publisher notes or download counters. It rejects wrong targets/artifact identities and retains previous metadata on failure. Automatic firmware replacement remains disabled; the pinned SHA stays unchanged. The official endpoint was also read successfully, with no BIN request. These fixes and the completed sweep are recorded in [checks.json](checks.json).

## Validation

Stable Rust 1.100.0: both Clippy variants with warnings denied, release workspace build, and all 2,221 Rust tests pass (175 groups, ignored tests included, 96 external-input tests filtered). All 419 platform tests, 15 production WASM scenarios, real 18-event recording/replay, timing/BLE ABI, 111,126 WASM differential cases, 24 GPIO waveform cases, production section inspection and profiling-feature compilation pass. Native/AArch64 coverage runs on the same arm64 host. No golden is regenerated in this follow-up.

```sh
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy --release --target wasm32-unknown-unknown -p esp32sim-wasm --features jit-tests -- -D warnings
cargo build --release --workspace
ESP32SIM_ROM_DIR="$ROM_DIR" cargo test --release --workspace -- --include-ignored --skip external_
tools/wasm-build.sh
ESP32SIM_ROM_DIR="$ROM_DIR" node --test tools/platform-*-test.mjs
node tools/probe-mosaico-factory.mjs "$OFFICIAL_BIN" output/mosaico-integration
ESP32SIM_ROM_DIR="$ROM_DIR" node tools/wasm-test.mjs hello c3-hello c6-hello c6-xiao p4-hello p4-c6-hosted p4-lcd35-synth c6-energy-scan c6-contiki c6-contiki-net c6-rpl-net panel tdisplay-prices tdisplay-schematik demo-stopwatch
ESP32SIM_ROM_DIR="$ROM_DIR" node tools/platform-recording-test.mjs --real
node tools/check-wasm-sections.mjs web/wasm/esp32sim.wasm
node wasm/tests/timing-api.mjs web/wasm/esp32sim.wasm
node wasm/tests/ble-api.mjs web/wasm/esp32sim.wasm web/wasm/fw/public "$ROM_DIR/esp32c3_rev3_rom.elf"
node wasm/tests/ble-connection.mjs web/wasm/esp32sim.wasm web/wasm/fw/public "$ROM_DIR/esp32c3_rev3_rom.elf"
tools/wasm-jit-test.sh
. ./tools/wasm-rustflags.sh
cargo build --release --target wasm32-unknown-unknown -p esp32sim-wasm --features jit-profile,cpu-profile
node tools/check-evidence-privacy.mjs
```

The primary working copy also builds its own production WASM and passes the official Mosaico probe with the pre-existing local edits retained. Its generated artifacts are not used for the clean-source receipts or CPU comparison. Browser sessions and preview servers are closed; completed worktrees are recoverably trashed after merge/patch-equivalence and open-handle checks, retaining branch refs and a private recovery record.

## CPU comparison

This follows the existing EX239/EX259/EX260/EX265 integration contracts, not a new optimization. Baseline is the completed upstream sync `9f1ff87b`; the candidate adds cold S31 host adapters and its WASM factory. The original upstream reset change exists on both sides, so all workloads use 30 guest seconds. Three alternating pairs per workload compare identical release settings and input hashes. No task-owned compiler, emulator or browser runs concurrently with the measurements. Shared-host short samples have no separate warmup and do not establish isolated speedups or hardware timing.

The initial invocation completed the three hello workloads, then stopped before Pocket Tank because the clean worktree lacked its model cache. Those completed samples remain in [cpu-hello.json](cpu-hello.json). The existing public model was checked against its pinned SHA-256 and only the missing workload was run again; see [cpu-pocket.json](cpu-pocket.json). No failed performance sample is discarded.

```sh
python3 tools/compare-display-cpu.py --baseline "$BASE_BIN" --candidate "$CANDIDATE_BIN" --firmware "$FW_DIR" --roms "$ROM_DIR" --rounds 3 --output cpu-hello.json
python3 tools/compare-display-cpu.py --baseline "$BASE_BIN" --candidate "$CANDIDATE_BIN" --firmware "$FW_DIR" --roms "$ROM_DIR" --rounds 3 --workload pocket-tank --output cpu-pocket.json
```

All 24 measured runs retain identical instruction counts, cycles, stop reasons and console hashes.

| Workload | Baseline median user s | Candidate median user s | Change |
| --- | ---: | ---: | ---: |
| hello | 0.229409 | 0.228828 | -0.25% |
| c3-hello | 2.437837 | 2.427983 | -0.40% |
| c6-hello | 1.521481 | 1.509609 | -0.78% |
| pocket-tank | 40.669617 | 40.583323 | -0.21% |
