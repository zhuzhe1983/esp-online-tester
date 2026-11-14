# Upstream synchronization, 2026-11-14

Merge `29b646eb` integrates upstream `mikroverk/esp32sim` through `3bdb9582` into product-fork baseline `b1c7a001`. The ancestry difference contains 166 commits; some features already existed independently in this fork. Only merged upstream main is included, not its open pull requests.

## Features and conflict decisions

The update includes native ESP32-S31 Machine/CLI and Mosaico (display/touch, audio/microphone, Wi-Fi/WPA2, IMU and DMA/PPA), RV32 instruction extensions and decoded blocks, CYD and XIAO C6 support, Ulanzi page controls, multi-AP Wi-Fi/PCAP, NAT transport abstraction, BLE/WASM APIs, UART ROM download mode, and GPIO/sleep/reset/SPI correctness fixes. The RV32 optimization is a decoded-block interpreter, not a native machine-code JIT.

All 56 textual conflict files are listed in [conflicts.json](conflicts.json), with grouped decisions. The fork keeps its C5/C61 boards, product platform, SoftAP/relay/dynamic host mappings, streamed audio, SDMMC IRQ path, light sleep and existing BLE contracts. Both new and old regression cases remain enabled.

The material S31 difference is explicit: native Machine follows upstream's register-storage fallback and counted ignored/refused operations, while `esp32s31::bringup` retains strict faults and the original diagnostic fixtures. Unsupported register storage does not demonstrate an emulated peripheral function. See [the S31 boundary](../../../esp32s31/README.md). This synchronization alone does not enable S31 in the product platform or WASM.

The two inherited `hello-s3-reboot` golden changes are intentional EX259 CCOUNT reset corrections. No golden was regenerated locally. Historical source logs, golden CRLF bytes and vendored license text are retained unchanged, including their upstream whitespace.

## Validation

[checks.json](checks.json) records compiler/runtime versions, artifact hashes, checks and limitations. Rust stable 1.100.0: both Clippy commands with warnings denied, release workspace build, and 2,219 Rust tests across 175 groups pass (ignored tests included; 96 external-input tests filtered). All 412 platform tests and 15 production WASM scenarios pass. Native virtual-quantum checks, network helper/relay, timing/BLE ABI, worker and benchmark harness checks, retained x5 receipts, 111,126 WASM differential cases, 24 GPIO waveform cases and profiling-feature compilation pass.

Headless browser checks use a repository subpath. The real StopWatch worker boots, produces a 468×466 framebuffer and reports a fresh BUTTON_A after input. Real recording/replay covers 18 events and ends with the same framebuffer. The legacy XIAO page runs its firmware; CYD/Ulanzi options, Network panel and local dissector load. Packet capture is tested in Rust; no native packet source was connected to that UI check. The external relay rejects localhost through CORS, so Internet access is not accepted here. Task-owned browser/server resources were closed.

Initial compile failures from the RX metadata name collision, new UART fields and C5/C61 Wi-Fi option type were fixed, without weakening tests. Missing local xterm vendor assets were fetched with the pinned repository script before the final legacy-page check.

## CPU comparison

This is an integration regression check of existing EX239/EX259/EX265 mechanisms, not a new speed experiment. Baseline `b1c7a001` and candidate `29b646eb` use the same compiler and release binary settings. No task-owned build, emulator or browser runs concurrently with the measured subprocesses. The source comparison is combined and cannot attribute cost to any one upstream change.

The first 30-second S3 hello comparison intentionally fails exact-work checks: the run crosses a reset, and EX259 changes CCOUNT, early log timestamps and instruction counts. Its six samples are preserved in [cpu-reset-difference.json](cpu-reset-difference.json). A 10-second pre-reset window restores exact work/output; five alternating pairs are in [cpu-pre-reset.json](cpu-pre-reset.json). The C3/C6 hello and Pocket Tank runs use 30 guest seconds and three alternating pairs in [cpu.json](cpu.json). The harness now accepts `--seconds` explicitly; its 30-second default is unchanged.

```sh
cargo +stable build --release --locked --bin esp32sim --bin esp32sim-c3 --bin esp32sim-c6
python3 tools/compare-display-cpu.py --baseline "$BASE_BIN" --candidate "$CANDIDATE_BIN" --firmware "$FW_DIR" --roms "$ROM_DIR" --rounds 5 --seconds 10 --workload hello --output pre-reset.json
python3 tools/compare-display-cpu.py --baseline "$BASE_BIN" --candidate "$CANDIDATE_BIN" --firmware "$FW_DIR" --roms "$ROM_DIR" --rounds 3 --workload c3-hello --workload c6-hello --workload pocket-tank --output cpu.json
```

These are short shared-host samples, without a separate warmup. They are not evidence of hardware timing, zero overhead or an isolated speedup. Only hashes, counts and numeric samples are retained; raw serial, screenshots, local paths and browser state are excluded.

All 28 equal-window runs retain identical work and console hashes.

| Workload | Guest seconds | Baseline median user s | Candidate median user s | Change |
| --- | ---: | ---: | ---: | ---: |
| hello | 10 | 0.076536 | 0.079417 | +3.76% |
| c3-hello | 30 | 2.404621 | 2.457703 | +2.21% |
| c6-hello | 30 | 1.539236 | 1.533157 | -0.39% |
| pocket-tank | 30 | 39.657413 | 39.482699 | -0.44% |
