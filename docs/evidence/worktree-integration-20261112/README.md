# Pending-branch integration validation

The candidate combines the retained upstream synchronization branch and the K147-CAM sensor-configuration subset with product-platform main. Exact revisions and check results are in [checks.json](checks.json). K147-CAM remains unavailable in the public runtime catalog; camera capture and audio are not implemented by this merge.

The fork keeps its existing BLE resolving-list configuration validation and does not advertise LL Privacy. The upstream BLE receipt is retained as history with that difference explicitly labeled. C61 Wi-Fi setup was updated for the upstream host-forwarding argument. Both existing board registrations and cold board hooks are preserved.

The S31 branch at `e20ddee749c5b0dbda3036d060d38a0d85dbabe6` is deferred: its Machine/DeviceSet migration replaces the existing Bringup bus, diagnostic interfaces, timer behavior and FPU policy. It needs a separate migration preserving the existing fixture contracts. Neither its execution changes nor product-enablement claims are included here.

## Validation

Rust stable 1.99.1: both warning-denying Clippy commands, release workspace build, 1,945 Rust tests in 171 groups (ignored tests included, external inputs skipped), 377 platform tests, 13 production WASM scenarios, real recording/replay with equal final framebuffer, 111,126 WASM differential cases and 24 GPIO waveform cases pass. Native virtual-quantum, network helper/relay, retained x5 receipts, timing/BLE ABI, worker/benchmark checks and profiling-feature compilation also pass. Two optional network-relay cases remain skipped by that suite.

A headless browser boots the internal StopWatch diagnostic through the production worker, captures a nonempty 468×466 framebuffer and verifies a fresh BUTTON_A response. Its browser and local server were closed. The public relay session endpoint rejects the local origin through CORS, so no Internet acceptance is claimed. This is integration regression coverage, not a fresh firmware acceptance run for every catalog product.

Initial native/platform attempts lacked the C61/S3 ROM in the selected checkout; after provisioning the pinned assets, the suites above pass. No tests were weakened and no golden files were regenerated. The inherited classic hello golden changes only the reported silicon revision from v2.0 to v3.0, consistent with the upstream ECO3 ROM correction.

## CPU comparison

This is a combined integration guard for existing mechanisms EX249 (upstream RV32 fetch/decode), EX254/EX255/EX257 (S3 service/idle paths), EX261 (native divide) and EX262 (idle C6 radio), not a new optimization experiment. [cpu.json](cpu.json) retains all alternating samples, binary/input hashes and exact-work comparisons. The baseline is main `4932dcd1`; the candidate source is `3a0a9a00`. Both use the same Rust stable compiler and release build command for the three native binaries.

```sh
cargo +stable build --release --locked --bin esp32sim --bin esp32sim-c3 --bin esp32sim-c6
python3 tools/compare-display-cpu.py --baseline "$BASE_BIN" --candidate "$CANDIDATE_BIN" --firmware "$FW_DIR" --roms "$ROM_DIR" --rounds 5 --output "$RESULT"
```

The harness uses 30 guest seconds and five alternating pairs for each hello workload and Pocket Tank. Short CPU samples, no separate warmup, shared-host load and a small task-helper compilation during the first Pocket sample limit attribution; the measurements do not establish hardware timing, zero overhead or a statistically isolated speedup. No other emulator or browser owned by this task ran during the comparison.

Only hashes, aggregate results and reproducible commands are committed. Raw UART, local paths, browser storage and screenshots remain outside Git. The retained binary and input hashes identify the exact measured artifacts.

All 40 measured runs preserve exact guest work and UART hashes.

| Workload | Baseline median user seconds | Candidate median user seconds | Change |
| --- | ---: | ---: | ---: |
| hello | 0.255235 | 0.227302 | -10.944% |
| c3-hello | 2.535599 | 2.529035 | -0.259% |
| c6-hello | 1.654740 | 1.556872 | -5.914% |
| pocket-tank | 39.891621 | 39.630693 | -0.654% |
