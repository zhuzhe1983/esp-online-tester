# Native window helpers, 2026-11-25

`bae7f607dfbd03a262a37ca2251832006e8bcea5` replaces the native AArch64 JIT's general interpreter dispatch for CALL8, CALLX8, ENTRY and RETW/RETW.N with four specialized helper entry points. Against `603bd8e842f3cdd69363a895f5cceef128cc54fe`, Wordle's median user CPU time falls **10.76%** in the coordinated host comparison below. This is an Apple M5 Max result; low-end ARM/CM0 performance remains unverified by this task.

## Mechanism and contract

This extends EX002's native investigation and its retained Wordle census. Unlike EX159/EX164's WASM region-call work, the native block boundaries, helper calling convention, overflow checks, budget decrement, post-helper reloads, traps and exits stay in place. Only the helper table slot selected for these operations changes. The four monomorphized functions implement the interpreter's window operations directly, including CALLX8 a8 aliasing, illegal and double exceptions, return underflow, window wrap and ENTRY's fall-through loop tail. The independent `exec_insn` oracle is unchanged. Direct AArch64 lowering is not adopted in this change.

The old helper slots retain offsets 0–72; four slots are appended at 80–104, growing the private native `Helpers` table from 80 to 112 bytes. CPU state, Board/I2C traits and the App's C ABI are unchanged. Source consumers must rebuild the whole native library; independently built old code/table pairs must not be mixed. App-local CodeCache Drop and I2C transport hooks can be retained; this commit does not edit them.

Real native Wordle runs retain 240 MHz, quantum 64, VQ 1, round batching 1, ROM boot, original 8 MiB merged Flash, no PSRAM and no input events. They retire **8,184,729 / 424,129,942** per-core instructions at 480,000,000 cycles, with 78,530 exceptions, 4,451 interrupts, 12,965 compiled blocks, 11,464 KiB of generated code and zero cache flushes. No clock, firmware, output, timing model or scheduling change is used to obtain the result.

## Correctness and code generation

- `xtensa-lx7/tests/jit_windows.rs`: 122,880 independent-interpreter comparisons over the 16 window bases, call increments, return sizes, live-window masks, WOE/EXCM/UM, direct/aliased indirect calls, illegal ENTRY operands, both RETW encodings, stack wrap and loop tails. Separate cases cover mid-block rotation, budget cuts, bus exits, overflow and CCOMPARE delivery. Actual compiled retirement is asserted.
- Full release workspace tests with ignored tests included and `external_` skipped passed. There are 2,907 main test passes plus one isolated cache child-process pass (2,908 reported passes across 240 result summaries). Both required Clippy commands pass with warnings denied. Native opt-in VQ checks pass 44 tests.
- Wordle baseline, candidate and `--no-jit` oracle have identical work, console and actual framebuffer PNG hashes. The quiet pairs include the same checks. Pocket Tank's additional 8 guest-second baseline/candidate PNG, console and work check also matches. See `correctness-wordle.json` and `correctness-pocket.json`.
- `codegen-helpers.json` and `window-helpers.dis` show four small S3 helpers with no BL/BLR calls, including no call to `exec_insn`. These are static disassembly sizes, not dynamic host instruction counts. 26 pre-existing Xtensa/S3 hot functions and 139 RV32/C3 hot functions are identical after the repository's address/padding normalization. Normalized identity does not establish identical cache placement or runtime cost.
- The final production WASM is byte-identical to the prior **local** production artifact: 8,514,890 bytes, SHA-256 `bf34206f6006e6adc4698ab832f18eb7e2ec1c1e1f450e998bd3664fbeb9224b`. This differs from the published CI artifact hash; they are not conflated. The shared-memory handoff, 13 real-firmware manifests and the four required platform test files pass. The first platform run correctly failed 404 because the ignored built WASM was absent; after installing that same verified artifact, the checks pass.

## Coordinated host CPU comparison

Apple M5 Max, macOS 27.2.1, rustc 1.100.0. Both native trees use `cargo build --release --bins`, default opt3/fat-LTO/codegen-units1/debug1. Binaries are frozen before measurement; hashes and inputs are in `verification.json`. Each run starts a new process and cold guest/JIT; warmup pairs warm host file caches, not the guest JIT across processes.

C5 owned compilation and guest/browser/worker activity was paused; the App supplied a fresh confirmation of no host build, VM, OCR or compression until explicit CPU return. Light edits and device I/O were allowed. This coordination does not imply an exclusive desktop. Anonymous loads and every sample remain in the JSON receipts. No device was accessed by this task.

Wordle: one warmup pair, then five alternating pairs. Median user CPU 2.236818→1.996077s (**−10.7627%**); candidate is faster in all five. Base range 2.205675–2.287064s, candidate 1.968093–2.019111s. A separate baseline-versus-the-same-baseline control, one warmup plus three pairs, gives 2.261535→2.250858s (**−0.4721%**). The control is small and order-sensitive; it is not a universal noise floor. Wordle user/system/wall/RSS are per-child `wait4` values.

The existing regression harness uses `os.times().children_user`, with 0.01s granularity on this host. One warmup pair and three alternating pairs per workload:

| Workload | Base median user s | New median user s | Change | New slower pairs |
| --- | ---: | ---: | ---: | ---: |
| S3 hello,30guest s |0.230|0.220|−4.35%|0/3|
| C3 hello,30guest s |2.410|2.460|**+2.07%**|3/3|
| C6 hello,30guest s |1.550|1.530|−1.29%|0/3|
| Pocket Tank,8guest s |7.070|6.950|−1.70%|0/3|

All work and console hashes match. C3's initial slowdown is retained as a negative result, despite unchanged normalized hot code. Small changes in other workloads do not establish isolated gains or zero cost. The C3 follow-up is recorded separately rather than pooled into this table. A fresh coordinated short window ran one warmup plus five C3 pairs: 2.450→2.430s (**−0.82%**), 1/5 slower, base range 2.430–2.470s and candidate 2.430–2.450s. Its baseline-versus-identical-baseline control, one warmup plus three pairs, is 2.460→2.440s (**−0.81%**). All work and console hashes match. The earlier slowdown is not reproduced and remains unattributed; these short, coarse-resolution samples do not prove zero cost. See `c3-followup.json` and `c3-aa.json`.

The first cohort is preserved as `*-contended.json`: the App subsequently reported a possible overlap with a version rebuild. Its Wordle screen was −11.7395%, A/A +0.0840%; it is not presented as quiet or pooled with the coordinated repeat. Original receipts are unchanged.

## TCA8418 diagnosis

The separate `97d00ce4882f66f05fc4c93326628b8280661965` changes only comments and tests in `esp-periph/src/i2c/tca8418.rs`, with no production behavior change. Six TCA tests and 319 peripheral tests pass, as do both Clippy commands. New cases preserve a ten-event press/release backlog at 62.5ms edge spacing, the live FIFO state after reading A and then B, and repeated A reads with auto-increment disabled.

[TI SCPS215G §8.6.2.4](https://www.ti.com/lit/ds/symlink/tca8418.pdf) describes draining events by repeated reads of KEY_EVENT_A with a FIFO shift after each read. [M5Stack's pinned factory driver](https://github.com/m5stack/M5Cardputer-UserDemo/blob/b549eac0a3c65bc108186c276b8fac0a214aaa4e/main/hal/utils/adafruit_tca8418/Adafruit_TCA8418.cpp) reads one byte at register0x04 in `getEvent`; its `enableInterrupts` does not enable auto-increment. The suspected B-register skip was not established as a model defect. The App owner subsequently resolved its edge-ordering issue; this source change does not claim to fix it or prove undocumented B–J burst behavior on hardware.

## Reproduce and limits

Build the baseline and helper commit with the same stable toolchain and default native release profile, copying `esp32sim`, `esp32sim-c3`, `esp32sim-c6` into separate caller-supplied directories. Use the exact ROM and merged Flash hashes in `verification.json`.

```sh
python3 docs/evidence/cardputer-adv-kernel-20261125/native_probe.py \
  --base "$BASE/esp32sim" --new "$NEW/esp32sim" --rom "$ROM" --flash "$FLASH" \
  --seconds 2 --pairs 5 --oracle --out output/wordle-pairs.json
ESP32SIM_ROM_DIR=roms python3 docs/evidence/p4-cpu/cpu_pairs.py \
  --base "$BASE" --new "$NEW" --pairs 3 --out output/cpu-pairs.json
```

Run the Wordle control with both binary paths naming the baseline. Preserve all samples; compare work and real output before interpreting time. CM0/A53 throughput, swap sensitivity and actual App frame pacing are still the App owner's acceptance work. Neither host improvement nor helper census percentages establish low-end ARM speedup or solve the reported 99.67s wall time.
