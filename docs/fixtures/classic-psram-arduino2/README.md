# Stock Arduino-ESP32 2.0.17 classic PSRAM fixture

Public MIT sketch, real stock dual-core `esp32:esp32:esp32cam` startup, no SDK
patches or explicit sketch initialization. The recipe installs Arduino-ESP32
2.0.17 in an output-local configuration and compiles with four jobs/nice 10.
It does not change globally installed cores. Supply an absolute output path.

    bash docs/fixtures/classic-psram-arduino2/recipe.sh OUTPUT
    python3 docs/fixtures/classic-psram-arduino2/provenance.py OUTPUT provenance.json
    ARDUINO2_FIRMWARE_DIR=OUTPUT/fw-A2 ESP32SIM_ROM_DIR=ROMS \
      CARGO_TARGET_DIR=TARGET CARGO_BUILD_JOBS=4 \
      nice -n 10 cargo +1.99.0 test --release -p esp32sim \
      --test classic_psram_arduino2 -- --ignored --nocapture --test-threads=1

The external tests boot present twice and absent once. They require matching
ELF symbols plus `.bin`, `.bootloader.bin` and `.partitions.bin` artifacts.
Only symbols/read-only breakpoints and reads observe real init/test/heap-add;
there are no guest stubs, cache/MMU preparation, memory writes or reset retries.
The same executable must handle absent-device failure without RAM fallback.

The present sketch allocates, volatile-writes/reads and frees 1 MiB twice with
different patterns, then checks full free-heap recovery. It does not exercise
the separate `realloc` API. Arduino's getter reports 4192123 payload bytes for
this 4 MiB fixture, not the physical size; the exact SDK metadata/poisoning
accounting is explained in `docs/evidence/classic-psram-arduino2/README.md`.

No firmware binary is redistributed or made a CI golden by this fixture.
Exact build artifacts and linked inputs are pinned by `provenance.py`.
Different absolute build paths affect ELF debug information and its embedded
application digest; full cross-directory ELF/image hash identity is not claimed.
All application bytes except that digest and dependent image checksums match
in the retained two-build qualification.

Mutation qualification requires a separate clean disposable worktree:

    ARDUINO2_FIRMWARE_DIR=OUTPUT/fw-A2 ESP32SIM_ROM_DIR=ROMS \
      CARGO_TARGET_DIR=MUTANT_TARGET CARGO_BUILD_JOBS=4 \
      python3 docs/fixtures/classic-psram-arduino2/mutate.py MUTANT_TREE RESULTS

Compilation failures are not kills. A new target (`MUTANT_TARGET-restored`) is
used for the restored control. Remove both owned targets after collecting logs.
No timing, hardware, camera, PSRAM-on WASM or production qualification is implied.
