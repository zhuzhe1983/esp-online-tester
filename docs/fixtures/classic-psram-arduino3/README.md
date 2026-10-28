# Stock Arduino 3.0.7 PSRAM qualification

This public MIT sketch uses the unchanged packaged dual-core SDK, not a unicore
replacement. Build from the repository root:

    bash docs/fixtures/classic-psram-arduino3/recipe.sh ABS_OUTPUT
    ARDUINO3_FIRMWARE_DIR=ABS_OUTPUT/fw-p6 ESP32SIM_ROM_DIR=ROMS \
      cargo +1.99.0 test --release -j4 -p esp32sim \
      --test classic_psram_arduino3 -- --ignored --nocapture --test-threads=1

The recipe uses an isolated Arduino installation. Platform 3.0.7, FQBN
esp32:esp32:esp32cam, bundled idf-release_v5.1-632e0c2a, GCC 12.2.0.
No core, SDK, clock or startup patches. The stock profile is explicitly
CONFIG_SPIRAM_BOOT_INIT=n, 80 MHz, dual core, light heap poisoning; the complete
sdkconfig hash is pinned by provenance.py. This is not a BOOT_INIT=y receipt.
The sketch reports the compiled profile; both success and absence tests require n.

The observer stops at the real memory test, Arduino psramAddToHeap and SDK heap
registration. It reads, but never modifies, SRAM/cache state. Guest allocations
prove completion of heap insertion independently of the detected flag.
Successful repeated Arduino init returns true; repeated SDK init returns
ESP_ERR_INVALID_STATE. Failed Arduino init remains failed on retry, rejects
heap-add and cannot allocate 1 MiB. A successful init still needs actual heap
registration; psramFound alone is not accepted.

The firmware is developer input, not a committed binary or CI golden. Both tests
are external_ and ignored with the required inputs named. Existing P5 inputs and
all classic/S3 goldens remain unchanged. See the evidence receipt for exact
accounting, provenance, commands and mutation results. This does not qualify
other SDK configurations, BOOT_INIT=y, WASM, camera or physical hardware.

Semantic probes:

    ARDUINO3_FIRMWARE_DIR=ABS_OUTPUT/fw-p6 ESP32SIM_ROM_DIR=ROMS \
      CARGO_TARGET_DIR=OWNED_TARGET python3 docs/fixtures/classic-psram-arduino3/mutate.py \
      CLEAN_DISPOSABLE_TREE EVIDENCE
    ESP32SIM_ROM_DIR=ROMS CARGO_TARGET_DIR=OWNED_TARGET \
      python3 docs/fixtures/classic-psram-arduino3/state_mutations.py \
      TREE ABS_OUTPUT EVIDENCE

The first script mutates emulator semantics only in a clean disposable worktree.
The second deliberately mutates the Arduino retry/error-state rules in the
caller-owned installation, builds separate negative firmware, then restores the
original source bytes in a finally block. Never use a shared installation.
The negative firmware is not an accepted control or a production fix. Compilation
errors do not count as kills. Recheck provenance and rebuild stock controls after
running these probes.

Licenses: the new sketch/harness are MIT; Arduino core and bundled libraries keep
their own upstream licenses. No linked firmware is distributed by this change.
The provenance archive list is input identification, not a blanket binary NOTICE.
