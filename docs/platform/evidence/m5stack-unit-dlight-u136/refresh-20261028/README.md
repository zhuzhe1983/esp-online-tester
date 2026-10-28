# U136 current-main verification

This review migrates `4670491d` onto main `5a9b68e0` with ENV-IV `95fc5da2` and the shared SHT4x refinement `da61bbc1`. Internal StopWatch SDA47/SCL48 remains on its shared hardware/GPIO-I²C path; external BH1750 uses only the routed hardware-I²C G10/G11 endpoint. No unrelated runtime board is enabled.

- `sources.json`: fifteen primary-source re-fetches, original and final URL, timestamp, MIME, bytes and SHA-256. All match the historical source hashes; original collection timestamps remain in the catalog/audit.
- `probe.json`: newly built ESP-IDF5.5.1 fixture on the actual page worker/WASM. Fifteen groups assert raw sensor bytes and UART results, including continuous H1/H2/L, one-shot H1/H2/L, gain/timing, saturation, power, wrong routing/address, disconnect, guest/platform reset, recording and ENV coexistence.
- `browser.json`: actual Studio input, shared mini restoration, 2D/3D and missing-CAD checks. Top-level visual checks retain the pre-SHT-refinement artifact hash; `finalRuntimeRecheck` repeats Studio/mini reads with the final shared SHT4x implementation and final WASM. A model HTTP404 removes the accessory geometry but does not prevent physical I²C execution. It is not a replacement 3D model or a claim of visible fallback geometry.
- `rebuild.json`: eleven byte-identical assets rebuilt from committed CAD, photographs and model plan. The parent verification file records final-render hashes and front/back external contour IoU; oblique projection, hidden connector depth and UNKNOWN PCB remain estimates.
- `checks.json`: full CI commands, results, local log hashes and final source/WASM hashes. Full logs remain ignored because they contain local build paths. No golden output changed. No hardware flashing or physical calibration was performed.
- `negative-results.json`: retained environment, metadata and browser-harness failures plus their resolution; these are not hidden by the final passing results.

## Reproduction

Build the firmware in an ESP-IDF5.5.1 Python environment. The current local Python3.14 environment had an excluded esp-coredump version; the installed Python3.11 environment built successfully without global dependency changes.

```sh
idf.py -C tools/fixtures/unit-dlight-firmware -B "$PWD/output/unit-dlight-current-build" -D SDKCONFIG="$PWD/output/unit-dlight-current-build/sdkconfig" set-target esp32s3 build
tools/wasm-build.sh
node tools/probe-unit-dlight.mjs --build output/unit-dlight-current-build
node --test tools/platform-*-test.mjs
python tools/validate-platform-catalog.py
python tools/rebuild-unit-library.py m5stack-unit-dlight-u136 --scratch output/dlight-rebuild-current
node tools/check-dlight-connector-compat.mjs
```

For Studio: choose StopWatch, physically attach U136 to Port A, load the fixture's `flasher_args.json` and all three segments, wait for `DLIGHT_READY`, set 4321.5 lx, wait at least 0.4 guest seconds and send UART0 `q`. Expected raw 5186. For mini: share the same scene with initial 120 lx; local firmware bytes are not shared, so supply the same build files; after setting 777 lx expect raw 932. The fixture does not draw a screen, so a dark display is expected. UI controls, agent and recordings use the same runtime input.

CPU reproduction uses the fixed-main independent native binaries recorded in `baseline.json`, with final candidate binaries from `cargo build --release --workspace`:

```sh
python docs/platform/evidence/m5stack-unit-dlight-u136/runtime/bench.py --base-ref 5a9b68e0 --base-bins /path/to/main-bins --candidate-bins target/release --rom-dir web/wasm/fw --workload hello --workload c3-hello --workload c6-hello --receipt output/dlight-current-cpu.json
```

This is an idle production regression guard for the complete ENV-IV+DLight stack against current main, not an isolated cost attribution to DLight. Three hello workloads are measured because this migration changes optional board behavior and cold input parsing, without changing the S3 CPU, generic bus or tick traversal. Historical Pocket Tank results remain in the earlier receipt. No speed or zero-overhead claim follows from overlapping noisy samples.

Final five-pair medians: S3 0.00%, C3 +0.397% (four of five pairs slower), C6 −1.212% (none slower). Every measured pair has identical guest instructions and stdout. C3 and timing quantization remain in the receipt; no samples were discarded or repeated for favorable results. These whole-stack values do not replace the separate ENV-only result or earlier cost observations.
