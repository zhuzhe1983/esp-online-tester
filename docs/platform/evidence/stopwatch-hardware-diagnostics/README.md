# StopWatch hardware initialization checks — 2026-11-02

Implementation through `9bc583e2`; baseline `18b8eadd`. The new correctness
contract extends [EX216](../../../experiments.md#ex216) with cold observations
of actual address-phase SCL settings, and [EX092](../../../experiments.md#ex092)
with optional CO5300 command-state gating. It does not change I²C ACK policy,
calibrate transfer timing or implement analog PM1 power/wake behavior.

## Firmware execution

[Actual Worker/WASM receipt](actual-worker.json) records input/module/ROM hashes,
executed instructions, guest time, diagnostic states and framebuffer hashes.
All four images boot from ROM, run at least eight guest seconds, produce nonblack
visible frames and have no observed panic. This is a startup sample, not full
application or hardware qualification.

| Input | Warnings | Information |
| --- | --- | --- |
| Original private application | PM1 clock 400 kHz versus 100 kHz configuration; download lock; missing explicit CO5300 brightness-control initialization | Power LED disabled |
| Repaired private application | None in covered checks | None |
| Internal fixture 1.0.1 | None in covered checks | None |
| M5Stack official User Demo V0.5 | None in this startup window | None |

Private binary/source paths, server URLs, artwork and raw serial data are not
published. Hashes identify those inputs but cannot make private firmware publicly
reproducible. The original bad register sequence has independent portable Rust
regressions. The corrected MIT fixture and official source are reproducible.
The original worker receipt omitted the official source digest; the curated
receipt adds the catalog digest that the loader enforced and retains the raw
receipt hash. Measured times, counters and framebuffer hashes are unchanged.

[Browser results](browser.json) use the same actual Worker/WASM in headless
Chrome: original → repaired on the workbench, then original in the mini player.
Three warnings are visible, stale findings clear on firmware replacement, the
390 px card fits, and no page errors occur. Screenshots were visually inspected
and retained privately; private artwork is not committed. UI transport doubles
separately cover malformed inputs, unknown checks and retired worker messages.

## Checks and commands

- Rust 1.99.1 stable: native all-target Clippy and WASM JIT Clippy with `-D warnings` pass.
- Full release workspace tests with `--include-ignored --skip external_`: 1,763 passed; existing goldens unchanged.
- All platform Node tests: 255 passed; final diagnostic boundary tests also cover an invalid 40 MHz guest divider without dropping the warning snapshot.
- Worker pacing/channel, real recording/replay, production WASM sections and the [public CI probe](public-fixture.json) passed.
- Production WASM: hello, C3/C6/P4 hello, P4 hosted and synthetic LCD, C6 energy/Contiki/network/RPL, panel, both T-Display fixtures and StopWatch passed.
- Catalog schemas and unique IDs passed (213 products, 10 accessories, 166 firmwares, 112 watchlist entries).
- ESP-IDF 5.5.1 rebuilt internal StopWatch fixture; source provenance is in `examples/platform-stopwatch/manufacturer-sources.json`.

```sh
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy --release --target wasm32-unknown-unknown -p esp32sim-wasm --features jit-tests -- -D warnings
ESP32SIM_ROM_DIR="$ROM_DIR" cargo test --release --workspace -- --include-ignored --skip external_
tools/wasm-build.sh
node --test tools/platform-*-test.mjs
ESP32SIM_ROM_DIR="$ROM_DIR" node tools/probe-hardware-diagnostics.mjs tools/fixtures/stopwatch-hardware-checks.json output/stopwatch-hardware-checks.json
node tools/probe-hardware-diagnostics.mjs "$LOCAL_CASES_JSON" output/stopwatch-local-checks.json
```

`LOCAL_CASES_JSON` explicitly supplies private input paths; no USB calls occur.
CI runs the public fixture check against the production WASM. The probe verifies
visible pixels and snapshot isolation, not just a ready message or 3D preview.

Retained setup failures: the first full Rust run lacked the locally fetched P4
inputs in the isolated checkout; restoring the pinned fixtures allowed the full
unchanged-code run to pass. The default Python lacked `jsonschema`; catalog
validation passed with `uv run --with jsonschema python tools/validate-platform-catalog.py`.
An initial browser wait did not await the asynchronous observation predicate and
paused during boot; explicit guest-time polling fixed the harness, with no
emulator change to hide the failed observation.

## Limits

No physical device was written during this verification. PM1 wake/retry timing,
analog power and a physical download-lock clearing operation remain unverified.
Missing 0x53 warns because manufacturer reset defaults conflict; it does not
force an invented black screen. Explicit sleep/display-off and enabled zero
brightness do hide output while raw GRAM remains available for debugging.
LED-off is informational because it can be intentional. See the
[model contract and primary sources](../../hardware-diagnostics.md).

## Native CPU guard

[All samples and binary/input hashes](cpu.json): seven alternating AB/BA pairs
per workload, 30 guest seconds each, measured by subprocess user CPU usage.
macOS 27.0.1 arm64, Rust 1.99.1 release. Instructions, cycles, exception/IRQ
counts and stdout hashes are identical across every sample for each workload.
Task builds and browser/Worker probes were idle; the host was not isolated and
other host activity was not quantified. No repeat was selected for favorable
numbers.

| Workload | Baseline median user s | Candidate median user s | Change |
| --- | ---: | ---: | ---: |
| hello | 0.236824 | 0.238836 | +0.850% |
| c3-hello | 3.255038 | 3.332046 | +2.366% |
| c6-hello | 1.647790 | 1.617781 | -1.821% |
| pocket-tank | 38.475708 | 38.420915 | -0.142% |

The C3 increase remains unattributed; these samples are a regression observation,
not a speed or zero-cost claim. The feature adds no per-tick polling, but shared
trait/layout/code changes can still affect unrelated workloads. Existing output
goldens were not regenerated.

```sh
cargo build --release --bins
python3 tools/compare-display-cpu.py --baseline "$BASELINE_BINS" --candidate "$CANDIDATE_BINS" --firmware "$FIRMWARE_DIR" --roms "$ROM_DIR" --output output/stopwatch-cpu.json --rounds 7
```
