# Testing plan for esp32sim

## Goals

The test suite should establish that:

- the Xtensa LX7 decoder and interpreter implement instruction semantics correctly;
- exceptions, register windows, interrupts and dual-core scheduling transition between
  architectural states correctly;
- the ESP32-S3 memory map and modeled peripherals behave consistently at their register
  interfaces and through complete data paths;
- real ESP-IDF firmware continues to boot and exercise the modeled hardware end to end;
- failures are reproducible without requiring a particular developer machine or physical
  board; and
- results that require external toolchains, third-party suites or physical silicon are
  clearly separated from the hermetic default suite.

The default `cargo test --workspace` must run useful tests with no environment variables,
downloaded firmware, ESP-IDF installation or hardware. A required test must never report
success after silently skipping its work.

## Status (2026-10-29)

Phase 1 is done (no silent skips; CI runs everything on every push, with the ROM ELFs
fetched). Phase 2 has its first semantic suites (`xtensa-lx7/tests/semantics.rs`,
`riscv-rv32/tests/semantics.rs`, assembled with the Espressif toolchains and pasted as bytes; the
oracle test covers step vs blocks vs JIT). Phase 4 has the register-level device tests
(`esp-periph/tests/devices.rs`) and the machine tests (`esp32s3/tests/machine.rs`). Phase 5's
goldens are in `tests/golden/` (all five chips; with reboot on the S3, C3, C6 and P4). Phase 1
item 5 (parser robustness) found and fixed a panic in the BMP loader.
Open: Phase 3 (a checked-in binary suite), Phase 6 (conformance firmware), the external sets,
and the silicon differential as a scheduled job. `tests/README.md` is the map.

## Test layers

| Layer | Purpose | Normal schedule |
| --- | --- | --- |
| Rust unit tests | Individual CPU, bus and peripheral state transitions | Every change |
| CPU binary microtests | Real assembled Xtensa instruction sequences | Every change |
| SoC integration binaries | Interrupts, timers, DMA, flash and dual core | Every change |
| ESP-IDF firmware images | ROM, bootloader, FreeRTOS and driver integration | CI/nightly |
| External test corpora | Broader compiler and emulator compatibility | Nightly |
| Real-silicon differential tests | Authoritative comparison with ESP32-S3 hardware | Scheduled/manual |

## Phase 1: reliable baseline

1. Replace the silent skip in `xtensa-lx7/tests/objdump_diff.rs` with one of two explicit
   modes:
   - a hermetic fixture used by the ordinary test; or
   - an `#[ignore]` external test that fails with a clear setup error when explicitly run
     without `XTENSA_DIS_FILES`.
2. Add CI commands for:
   - `cargo test --workspace`;
   - `cargo fmt --all -- --check`; and
   - `cargo clippy --workspace --all-targets`.
3. Pin the Rust release and the Espressif toolchain/ESP-IDF release used to generate test
   artifacts.
4. Print meaningful case counts for generated and external suites. Zero collected cases is
   an error.
5. Add parser tests and fuzz targets for ELF, ESP application image, PPM and action-script
   inputs. Arbitrary input should return an error rather than panic.

Strict `-D warnings` can be introduced after the existing warnings and style debt are
resolved; it need not block the first testing changes.

## Phase 2: host-driven CPU semantic tests

Add a shared harness around `xtensa_lx7::step` and a small in-memory `Bus`. A table-driven
case should be able to specify:

- initial PC and instruction bytes;
- address registers, PS and special registers;
- initial scratch memory;
- maximum steps;
- expected PC, registers and memory writes; and
- expected completion, exception, interrupt or unimplemented trap.

Suggested layout:

```text
xtensa-lx7/tests/
  support/mod.rs
  decode_cases.rs
  alu_cases.rs
  branch_loop_cases.rs
  memory_cases.rs
  windows_cases.rs
  exceptions_cases.rs
  interrupts_cases.rs
  floating_point_cases.rs
  mac16_cases.rs
  pie_cases.rs
```

Primitive instruction tests should be checked by host-side Rust expressions or explicit
expected values. They should not depend on guest comparison or branch instructions that may
contain correlated emulator bugs.

Important input classes include:

- `0`, `1`, `-1`, `i32::MIN`, `i32::MAX` and `u32::MAX`;
- shift widths around `0`, `7`, `8`, `15`, `16` and `31`;
- aligned, misaligned, mapped and unmapped addresses;
- NaN, infinity, signed zero and subnormal floating-point values;
- window-base wraparound and window underflow/overflow;
- interrupt priority boundaries; and
- loop counts `0`, `1` and wrapping values.

Maintain an instruction coverage inventory. Every supported base `Op` must have a semantic
test, not only a decode test. Every PIE table entry must have decode coverage, and every PIE
operation used by ESP-DL/ESP-DSP must have semantic coverage.

## Phase 3: checked-in Xtensa binary suite

Small freestanding binaries should be first-class test fixtures. Check in both source and
assembled bytes so the ordinary suite does not need an Xtensa toolchain:

```text
tests/cpu-binaries/
  README.md
  toolchain.lock
  regenerate.sh
  cases/
    alu.S
    shifts.S
    loops.S
    windows.S
    exceptions.S
    interrupts.S
    fpu.S
    mac16.S
    pie.S
  bin/
    alu.bin
    shifts.bin
    ...
  expected/
    alu.txt
    ...
  SHA256SUMS
```

`regenerate.sh` should use a pinned `xtensa-esp-elf` assembler/linker and fail if regenerated
artifacts differ. `toolchain.lock` should record tool names, versions, flags, load addresses
and linker script. Each binary must have license/provenance information.

### Test ABI

Use a deliberately small emulator-test ABI:

- a fixed SRAM load address and stack address;
- a fixed result mailbox in scratch RAM;
- a mailbox header containing magic, ABI version, test ID and status;
- optional actual/expected diagnostic words;
- `simcall` to signal completion; and
- an instruction/cycle limit to detect hangs.

The emulator already turns `simcall` into a stop reason. The host harness should also fail on
an unexpected exception, unimplemented instruction, timeout or write outside the allowed
scratch region.

Use two kinds of binary test:

1. **Host-inspected microtests:** execute one instruction or a very short sequence and have
   Rust inspect architectural state. These provide the strongest isolation.
2. **Guest self-tests:** longer assembly or C programs write pass/fail and diagnostics to the
   mailbox. These are useful for interactions and compiler-generated code.

Prefer several focused binaries over one monolithic image so failures remain easy to locate.

## Phase 4: SoC and peripheral tests

### Register-level tests

For every modeled peripheral, test:

- reset values;
- read/write masks and reserved bits;
- write-one-to-clear/write-one-to-set behavior;
- raw, enable, status and clear interrupt relationships;
- timing one cycle before, at and after a deadline; and
- which state is cleared or retained across each reset type.

Initial targets are systimer, timer groups, interrupt matrix, GPIO, UART, USB Serial/JTAG,
RTC watchdog, SPI flash, cache MMU, GDMA, I2S, RMT, I2C, SHA and LCD_CAM.

### Guest-driven data paths

Add small binaries that configure devices as real firmware does and assert observable results:

- a timer interrupt increments a mailbox counter;
- UART and USB emit known framed data;
- SPI returns the expected JEDEC ID and flash bytes;
- cache MMU mappings span pages and invalid mappings fault;
- GDMA copies patterned buffers and raises DONE/EOF interrupts;
- I2S emits deterministic PCM samples;
- RMT emits a known WS2812 frame;
- I2C reads the modeled board-device identities;
- camera DMA receives a small deterministic frame;
- watchdog reset produces the expected subsequent reset cause; and
- core 0 releases core 1 and both cores update independent mailboxes.

Run these tests against `Machine` directly rather than spawning the CLI. Direct library access
is faster and exposes memory, CPU state and device events without parsing human-readable logs.

## Phase 5: board-model golden tests

Golden assertions should use semantic state or stable hashes:

- replay GPIO edges into the ST7735 model and compare GRAM hash, visible-frame hash and
  selected pixels;
- feed RMT symbols and compare LED RGB values;
- run a short speaker scenario and compare sample count, format and PCM hash;
- feed a fixed camera image and compare YUYV hash and selected pixels; and
- replay the Atech action script and compare the ordered board events.

On failure, write optional PNG, WAV and event-log diagnostics into the test output directory.
Do not use PNG or WAV files as the only assertion when the underlying pixel/sample values can
be compared directly.

## Phase 6: ESP-IDF conformance firmware

Create a purpose-built `tests/firmware/emu-conformance` ESP-IDF project. It should report
machine-readable records over UART or USB, for example:

```text
ESP32SIM-TEST {"name":"boot","status":"pass"}
ESP32SIM-TEST {"name":"core1","status":"pass"}
ESP32SIM-SUITE {"passed":18,"failed":0}
```

Cover at least:

1. direct application boot;
2. ROM to second-stage bootloader to application;
3. FreeRTOS scheduling on both cores;
4. timers and interrupt priorities;
5. software and watchdog resets;
6. flash mapping and PSRAM;
7. UART and USB consoles;
8. I2C device interaction;
9. GDMA, I2S and RMT;
10. the camera path;
11. the Atech board scenario; and
12. the Waveshare detection scenario.

Pin one baseline ESP-IDF release for required CI. Test additional supported releases only in
the compatibility/nightly job. Small license-clean firmware artifacts may be checked in with
their source. Images containing redistributability-sensitive ROMs or binary blobs should be
built or downloaded in the external job, cached, and verified by hash.

ESP-IDF's own target-test convention uses component `test_apps`, Unity and pytest-embedded.
The conformance application should follow those conventions where practical so it can run on
both esp32sim and a real board.

References:

- <https://github.com/espressif/esp-idf/blob/master/docs/en/api-guides/unit-tests.rst>
- <https://docs.espressif.com/projects/esp-idf/en/latest/esp32s3/contribute/esp-idf-tests-with-pytest.html>
- <https://github.com/espressif/pytest-embedded>

## External test sets

External suites expand coverage but must not be required by the hermetic default tests.

### Flexe

The MIT-licensed Flexe ESP32/LX6 emulator reports hundreds of tests for instructions,
register windows, exceptions, interrupts, peripherals and firmware. Audit and port compatible
semantic cases. LX6 and LX7 configuration differences must be recorded explicitly, and S3 PIE
tests remain local to this project.

Reference: <https://github.com/levkropp/flexe>

### QEMU Xtensa TCG tests

QEMU maintains target programs under `tests/tcg/xtensa`. Adapt compatible cases to the test
mailbox/`simcall` ABI or run them through an external adapter. Filter cases by the ESP32-S3
core configuration, and audit each imported file's license before copying it into this MIT
repository.

References:

- <https://gitlab.com/qemu-project/qemu/-/tree/master/tests/tcg/xtensa>
- <https://www.qemu.org/docs/master/devel/testing/main.html>

QEMU is useful as an independent implementation, but real ESP32-S3 results take precedence
when its configurable Xtensa core differs from the LX7 configuration.

### ESP-IDF test applications

Select ESP-IDF component test applications that exercise already-modeled hardware. Initial
candidates include FreeRTOS, timers, SPI flash, SHA, UART, GPIO, I2C, GDMA, I2S and RMT. Avoid
tests that require RF, analog behavior, unmodeled peripherals or precise cache timing.

### Compiler execution suites

Compile portable C programs to ESP32-S3 freestanding binaries and run them through the guest
test ABI. Candidates include GCC `gcc.c-torture/execute`, the corresponding LLVM test-suite
corpus and selected compiler-rt integer/floating-point tests. Exercise multiple optimization
levels (`-O0`, `-O1`, `-O2`, `-Os`, `-O3` and, separately, LTO).

Fetch and build these suites in nightly CI until their per-file licensing and redistribution
requirements have been reviewed. Do not copy an upstream corpus or derived binaries into the
repository without that review.

Reference: <https://github.com/gcc-mirror/gcc/blob/master/gcc/testsuite/README.gcc>

There is no known public Xtensa architectural compliance suite comparable to the RISC-V
architecture tests. Cadence provides commercial Xtensa simulators, so hardware differential
testing remains important.

## Differential testing against real silicon

Extend `hw/difftest*.sh` from fixed firmware traces to generated programs:

1. Generate a constrained instruction sequence from a recorded seed.
2. Initialize registers and a dedicated scratch-memory region.
3. Run the same binary on esp32sim and a physical ESP32-S3 through JTAG.
4. Compare PC, PS, address/special registers and scratch memory at defined checkpoints.
5. Reduce a failing sequence where practical.
6. Commit every failure as a permanent hermetic regression fixture.

Begin with deterministic user-mode arithmetic, shifts, branches, loops, loads/stores, FPU and
PIE. Add windows, exceptions and interrupts after the generator can avoid undefined states.
Mask only values proven to be nondeterministic, such as time-dependent registers. Every mask
must be documented.

Use this oracle order:

1. a mathematical host oracle for simple operations;
2. QEMU or another independent emulator for broader sequences; and
3. physical ESP32-S3 silicon as the authority for configuration-specific behavior.

## CI tiers

### Required per-change suite

- formatting and ordinary lint checks;
- all Rust unit and integration tests;
- all checked-in CPU binaries;
- register-level peripheral tests;
- short SoC data-path binaries; and
- a small direct-boot firmware smoke test.

Target: deterministic and reasonably fast on Linux and macOS.

### Nightly suite

- regenerate binary fixtures with the pinned toolchain and compare hashes;
- build and run the ESP-IDF conformance firmware;
- ROM/bootloader boot scenarios where legally distributable inputs are available;
- selected ESP-IDF component applications;
- QEMU/Flexe-derived compatibility cases;
- compiler execution suites;
- fuzzing for a fixed time budget; and
- longer Atech and Waveshare scenarios.

### Hardware suite

- current fixed ROM/bootloader differential traces;
- generated CPU differential programs;
- conformance firmware on a real ESP32-S3; and
- emulator-versus-hardware event/transcript comparison.

Hardware absence should be reported as `not run`, never folded into the ordinary passing test
count.

## Milestones

### Milestone 1: trustworthy default tests

- No silently skipped required tests.
- Shared CPU case harness exists.
- At least 50 high-value instruction and exception cases.
- ELF/image parsers do not panic on tested malformed inputs.
- CI runs test, format and lint jobs.

### Milestone 2: CPU regression suite

- Checked-in, reproducible assembly fixtures exist.
- At least 100 semantic CPU cases run by default.
- Every supported instruction family is represented.
- Windows, exceptions, interrupts, FPU, MAC16 and PIE have direct tests.
- Instruction coverage inventory fails when a supported operation is untested.

### Milestone 3: SoC confidence

- Every modeled peripheral has reset/register/interrupt tests.
- Timer, MMU, SPI flash, GDMA, I2S, RMT, I2C, watchdog and dual-core data paths have guest
  integration tests.
- Board display, LED, audio and camera models have deterministic golden tests.

### Milestone 4: firmware and external coverage

- Three or more firmware images boot and report structured pass/fail results.
- Direct boot and ROM/bootloader boot are both covered.
- A selected external Xtensa corpus runs nightly.
- Compiler-generated execution tests run at several optimization levels.
- Hardware differential failures automatically become permanent regression fixtures.

## Recommended implementation order

1. Fix the decoder-test skip and create the shared CPU harness.
2. Add focused host-driven CPU tests.
3. Establish the binary fixture format and regeneration script.
4. Cover windows, exceptions, interrupts and PIE.
5. Add timer, interrupt-matrix, MMU and DMA register tests.
6. Build the ESP-IDF conformance application.
7. Turn `hello_world`, Atech and Waveshare runs into deterministic regressions.
8. Port compatible Flexe and QEMU cases.
9. Add randomized hardware differential testing.
10. Add compiler execution suites to nightly CI.

