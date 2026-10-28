#!/usr/bin/env bash
set -euo pipefail
export ESP32SIM_ROM_DIR="$PWD/web/wasm/fw"
run_check() { local name=$1; shift; "$@" > "output/current-ci-$name.log" 2>&1; printf '%s passed\n' "$name"; }
run_check stable rustup update stable
run_check build cargo build --release --workspace
run_check clippy-native cargo clippy --workspace --all-targets -- -D warnings
run_check clippy-wasm cargo clippy --release --target wasm32-unknown-unknown -p esp32sim-wasm --features jit-tests -- -D warnings
run_check rust cargo test --release --workspace -- --include-ignored --skip external_
run_check wasm-build tools/wasm-build.sh
run_check wasm-sections node tools/check-wasm-sections.mjs web/wasm/esp32sim.wasm
run_check wasm node tools/wasm-test.mjs hello c3-hello c6-hello p4-hello c6-energy-scan c6-contiki c6-contiki-net c6-rpl-net panel
run_check receipt-test node --test docs/evidence/perf-x5-2026-09-22/verify-results.test.mjs docs/evidence/perf-x5-2026-09-22/m1-before-after/verify.test.mjs docs/evidence/perf-x5-2026-09-22/historical-before-after/verify.test.mjs
run_check receipt-verify node docs/evidence/perf-x5-2026-09-22/verify-results.mjs
run_check receipt-m1 node docs/evidence/perf-x5-2026-09-22/m1-before-after/verify.mjs
run_check receipt-historical node docs/evidence/perf-x5-2026-09-22/historical-before-after/verify.mjs
run_check vq env ESP32SIM_VQ_NATIVE=1 cargo test --release -p esp32s3 --test machine --test virtual_stops
run_check wasm-tdisplay node tools/wasm-test.mjs tdisplay-prices
run_check platform-all node --test tools/platform-*-test.mjs
run_check recording node tools/platform-recording-test.mjs --real
run_check stopwatch node tools/wasm-test.mjs demo-stopwatch
run_check timing-api node wasm/tests/timing-api.mjs web/wasm/esp32sim.wasm
run_check ble-api node wasm/tests/ble-api.mjs web/wasm/esp32sim.wasm web/wasm/fw/public "$ESP32SIM_ROM_DIR/esp32c3_rev3_rom.elf"
run_check ble-connection node wasm/tests/ble-connection.mjs web/wasm/esp32sim.wasm web/wasm/fw/public "$ESP32SIM_ROM_DIR/esp32c3_rev3_rom.elf"
run_check worker-pacing node tools/worker-pacing-test.mjs
run_check worker-channel node tools/worker-channel-test.mjs
run_check worker-touch node --test tools/touch-input-test.mjs tools/stopwatch-ui-test.mjs
run_check browser-contract node --test tools/wasm-build-policy.test.mjs tools/browser-benchmark/*.test.mjs
run_check browser-python python3 -m unittest discover -s tools/browser-benchmark -p 'test_*.py'
run_check jit tools/wasm-jit-test.sh
run_check profiling bash -c '. ./tools/wasm-rustflags.sh; cargo build --release --target wasm32-unknown-unknown -p esp32sim-wasm --features jit-profile,cpu-profile'
run_check arm cargo test --release -p xtensa-lx7
