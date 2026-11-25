# C5 production WASM and real browser checkpoint

The new default-release WASM executes the unchanged original LCD2.73 factory
through ROM, bootloader, the full PSRAM test, app_main and SHTC3 ID0807. Both
the production worker and actual headless Chrome stop at the same unimplemented
private MAC word, with the original UI still absent. The [receipt](runtime-checks.json)
keeps these partial results separate from successful ABI/protocol checks.

The production artifact is8,846,328 bytes, SHA-256
`85c451d202bbad9df5ee438d95cc87107960e40bf58a6d53c0277065b0fdfd32`.
It uses `tools/wasm-build.sh` with one build job and default release settings,
without the focused native LTO override. Twenty Node checks in seven files
pass with no skips, including real compiled C5 cold-load/corruption tests,
ROM/SRAM lifecycle protocols, exact diagnostics parsing and platform checks.
The cold-load ABI case alone executes no firmware; the separate worker does.

The real public-runtime worker loads only the pinned ROM and merged factory,
uses interpreter ROM startup and has no stubs. Its result is partial, exit3,
`acceptancePassed:false`, with `GUEST_BLOCKED` at408142c6. The exported64MiB
card after stop is byte-identical to the input:0 changed bytes/sectors. This
proves preservation through this failed boot, not guest mounting, a completed
SD transfer or durable persistence. The Node backend is memory-only.

In actual Chrome the exact product is configured through its documented API,
then the human local-file picker uploads the unchanged factory. The UI
recognizes a complete merged image and the Load and run button starts the real
worker. Buffered guest output contains the PSRAM pass, Calling app_main,
Sensor ID0807 and the MAC stop PC. There is no C5_SRAM_REQUIRED error. The
observed error is EMULATOR_STOPPED/code2; no successful original UI is claimed.
Flash storage is IndexedDB, pristine with0 changed pages. The default card is
64MiB FAT32, but guest SD command count remains0. The ready3D model is only
presentation, and last-published stats are not live final CPU counters.

No JavaScript console errors or warnings were observed. The private screenshot
and raw observation are hash-pinned without copying them into Git. The runtime
was explicitly disposed; the task-owned browser was closed and its absence
verified; the task server exited and its port had no listener. Other browsers
were not closed.

The worker captured tracked HEADfa520550 while the compiled source composition
was still uncommitted. That composition was subsequently committed95da422c;
the six exact source hashes and WASM hash identify it. The receipt does not
mislabel this as a clean fa520550 build. beb97707 changes old evidence metadata
only. The default-release full workspace run passed 3,459 tests, with no failures
or ignored tests and 144 filtered tests, using CI's include-ignored/skip-external
command. All 14 production WASM demos and the JIT handoff check also passed.
The full-run wall time includes 747.404 seconds of coordinated compiler pauses
and is not a performance measurement. One build job was used throughout; a
planned parallelism change was not performed because the run had already ended.
No golden outputs were regenerated. CPU controls, latest-main integration and
original LCD/buttons/TF acceptance remain separate gates; the recorded result
remains partial regardless of regression success.

Reproduce with the three pinned inputs from integration-checks.json, a new
ignored output directory and a caller-selected runtime checkout:

```sh
CARGO_BUILD_JOBS=1 sh tools/wasm-build.sh
node tools/c5-lcd273-acceptance.mjs --run \
  --runtime-root "$RUNTIME_CHECKOUT" --wasm "$WASM" \
  --wasm-sha256 85c451d202bbad9df5ee438d95cc87107960e40bf58a6d53c0277065b0fdfd32 \
  --rom "$ROM" --factory "$FACTORY" --sd "$SD" --out "$NEW_OUTPUT" \
  --wall-timeout-ms 120000 --save-uart --save-sd
```

Build with CARGO_TARGET_DIR and CARGO_PROFILE_RELEASE_LTO unset, as the build
script copies from the checkout target. The current original MAC stop is an
expected partial result, not a passing product test. For UI reproduction,
serve the checkout with `tools/serve-platform.py`, use an isolated headless
browser and the exact product/local-file flow above, then dispose/close it.
No input clearing, firmware edits, forced ACK/ready, PC overrides, driver hooks
or host-generated frame substitutes are used.
