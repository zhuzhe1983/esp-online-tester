# PaperS3 EDCBook: indexed vector load and display follow-up

The unchanged EDCBook 3.1.2 merged image stops on the previous PaperS3 runtime at
PC `0x420a0da0`, word `0xe0723d4e`. Espressif's Xtensa objdump decodes that word as
`ee.ldxq.32 q2, q0, a4, 0, 6`. The image's SHA-256 is
`06eb3b595fc47bc5d35f760a16cdc5795ea8c0fad6295bcee8db5268cf906986`
(16,777,216 bytes). The caller supplied the local image; no firmware bytes are
patched, committed or republished. Its public upstream reference is the
[fixed M5Burner version](https://burner.m5stack.com/api/v1/firmwares/1897200000000000015/versions/2113660506860756993/download).

## Instruction correction

Source revision `43792396` implements the table-path instruction. The generator
previously gave its independent destination `sel4` and source `sel8` operands the
same role. Separate roles preserve both selectors; the generated STXQ entry also
retains both selectors but STXQ execution remains unimplemented.

The [ESP32-S3 TRM](https://documentation.espressif.com/esp32-s3_technical_reference_manual_en.pdf)
v1.8, section 1.8.37, printed page 113 describes an unsigned 16-bit source lane,
scaled by four and added to the address register. The effective address wraps at
32 bits and its low two bits are cleared. One 32-bit load replaces only the
selected destination lane; the address register and other lanes are retained.
Read-before-write also covers aliased source/destination Q registers. The TRM's
lane-three operation line says `[63:47]`, inconsistent with its eight 16-bit
segments; using `[63:48]` is an explicit inference from that layout, with a
directed adjacent-bit test. No physical-silicon validation is claimed.
The official site's indexed section text was checked; the full PDF was not
downloaded and no PDF file hash is asserted. Two Espressif objdump releases,
esp-15.2.0_20251204 and esp-14.2.0_20260121 (binutils 2.43.1), independently
decoded the reported word with the same operands.

The actual indexed address can be 262,140 bytes beyond the base, outside the
existing conservative PIE address-register margin. Virtual-quantum deferral now
checks this instruction's aligned effective address before memory access. It
does no new work when deferral is disarmed. A failed load leaves the old Q value
available for retry. The existing WASM fallback executes the operation; no new
JIT emitter is added.

This extends correctness coverage of EX103/EX042 and the EX133 deferral contract.
It is not a new operand-packing, scheduling or throughput experiment. Dependency
masks read both Qs and the retained destination Qu and write Qu. EX080/EX146 did
not measure LDXQ load latency, so this change does not assign it an invented
loaded-Q delay or claim cycle accuracy.

## Reproduction and negative results

The previous clean production WASM (`8d8ccce696666eeb7a355cdbe15fd398eaec9ddb9ae9ade85544d91639b4fb20`)
stopped with `EMULATOR_STOPPED`, code 2, at 0.08 reported guest seconds and
29,134,366 instructions. It produced no frame. The native executable reproduced
the same PC and word; CLI exit status alone is insufficient because this stop
was still reported with process exit zero.

An initial instruction-only production WASM
(`84d666dab12285112d93f20570f5739b962b192f70c48de273a05f702466936b`)
reached 30 guest seconds and 1,558,435,845 instructions, with one UART0 ROM boot
and no emulator error. Capture still rejected with `NO_FRAME`: the RGB LCD output
used by EPDiy had not reached the PaperS3 panel model. Thus fixing the opcode
alone did not establish product compatibility. Product-level display and input
results belong in [the PaperS3 EDCBook report](../../platform/papers3-edcbook.md).

Use the caller's original file and an isolated test output directory:

```sh
node tools/probe-papers3-edcbook.mjs \
  --url 'https://burner.m5stack.com/api/v1/firmwares/1897200000000000015/versions/2113660506860756993/download' \
  --sha256 06eb3b595fc47bc5d35f760a16cdc5795ea8c0fad6295bcee8db5268cf906986 \
  --file <original-image.bin> --output <output-directory>
```

The probe uses the production page runtime and worker in Node; a real Chromium
page check is reported separately. A virtual 64 MiB FAT32 card is attached by the
runtime. The native opcode-only reproduction did not attach a card, so its card
mount errors are not evidence of a TF emulation failure. Combined USB/UART
console text duplicates some boot messages; ROM boot counts use UART0 alone.

## Core validation at 43792396

- Five directed LDXQ tests include 8,192 register/selector/alignment combinations,
  unsigned high indices, source/destination aliasing, PC advance, 32-bit wrapping,
  fault preservation/retry and CP3-disabled behavior. An additional unit test
  checks far-index and wraparound deferral without accessing memory.
- Both required Clippy commands pass with warnings denied.
- CI-style release workspace tests: 1,963 passed across 172 result groups,
  zero failed or ignored (`--include-ignored --skip external_`).
- WASM differential checks: 111,126 cases across 78 groups; all 24 GPIO waveform
  cases passed. These broad regressions supplement the actual EDCBook run; they
  are not independent exhaustive LDXQ differential coverage.
- An earlier workspace attempt failed ten P4 co-simulation tests because the
  new checkout lacked required public firmware fixtures. Fetching and verifying
  the pinned fixtures resolved that setup failure; no tests were skipped.

Commands use stable Rust 1.99.1:

```sh
cargo +stable clippy --workspace --all-targets -- -D warnings
cargo +stable clippy --release --target wasm32-unknown-unknown -p esp32sim-wasm --features jit-tests -- -D warnings
ESP32SIM_ROM_DIR=<rom-directory> cargo +stable test --release --workspace -- --include-ignored --skip external_
tools/wasm-jit-test.sh
```

Full private console captures and the caller's local filename are excluded from
this receipt; only reproducible counters, hashes and outcomes are retained.
Source firmware and output PNGs remain in caller-owned ignored storage.

## Integrated product checks

The final runtime source is `cad3ddd2`, including the RGB adapter (`e358441d`),
stable-Clippy cleanup (`d328c087`) and the nominal battery divider input. Production
WASM is 5,582,097 bytes, SHA-256
`f0f3925e54876db21a77a1a62b1fa73c8ac29621bc81cace460a43670020b8fe`.
Both Clippy commands pass on this source; all 1,969 CI-style Rust tests pass across
172 groups. The 387 platform tests pass, public catalog schemas validate, and
goldens are unchanged. The 111,126 WASM differential and 24 GPIO waveform cases
were rerun after RGB integration at `d328c087`; the subsequent battery change
only installs a default source during board attachment and has directed SAR,
reset and retained-wake tests plus the final production-browser checks.

[Chromium 157 results](browser-final.json) cover the original image's homepage,
built-in guide, next/previous page and nonempty firmware-drawn battery icon.
All frames are 540×960, with no emulator error or repeated ROM boot. The returned
page still differs at 388/518,400 glyph pixels (x35..507/y92..328); this remains a
negative result for exact partial-refresh consistency. No 16-gray optical claim.
[Official v0.5 regression](factory-battery-regression.json) independently passes
its i80 screen, touch toggles, IMU input, Wi-Fi scan and actual TF test-file
write/reset/reload checks on the final WASM. Those TF/network results do not
transfer to EDCBook.

Two reload observations are retained. With no modeled battery source, the guest
showed a low-battery shutdown screen ([original browser receipt](browser-before-battery.json)).
The nominal 4.0 V battery now supplies 2.0 V at physical G3 through the existing
SAR conversion/calibration path; explicit low voltage or raw-zero host input is
not overwritten. There is no discharge or charging model.

After preserving 16 changed Flash pages, an unattended reload still shows the
firmware's shutdown cover. A controlled repeat with the same saved data, holding
the startup logo at (270,250) for about two guest seconds starting near 2.5 seconds,
enters the homepage and then the reader. This verifies the required startup input
without erasing data or patching firmware. The [read-only NVS receipt](../../platform/evidence/papers3-edcbook/reload-nvs-20261112.json)
contains no `boot_guard` or `skip_welcome` key; `first_boot=0`, `boot_retry=1` and
`shutdown=3` are present. Their exact branch/enum interpretation is not established,
so this is not attributed to a user-enabled Boot Guard setting. Queued touches
after the shutdown cover are explicitly not counted as successful reader input.

## Native CPU comparison

Three alternating pairs per workload, each 30 guest seconds, compare retained
`2891d72c` executables with final runtime `cad3ddd2`. There is no Rust/Cargo diff
between the baseline build and integration base `13fa568e`. All instruction,
cycle, exception, interrupt and stdout hashes match across every arm. Task
browsers, workers and compilation were stopped before timing; other system
activity was not controlled.

| Workload | Baseline median CPU s | Final median CPU s | Change |
| --- | ---: | ---: | ---: |
| hello | 0.217142 | 0.223036 | +2.714% |
| c3-hello | 2.553327 | 2.532751 | -0.806% |
| c6-hello | 1.548485 | 1.571527 | +1.488% |
| pocket-tank | 39.754989 | 40.014031 | +0.652% |

Nonzero host-time changes are not isolated to a mechanism, and no speedup or
zero-overhead claim is made. [Final samples and hashes](cpu-final.json) retain
all 24 executions. [The earlier candidate](cpu-before-battery.json) is preserved;
the repeat is justified by the additional board-default voltage code, not by
selecting a better timing result.

```sh
python3 tools/compare-display-cpu.py --baseline <baseline-binaries> \
  --candidate <candidate-binaries> --firmware <firmware-directory> \
  --roms <rom-directory> --rounds 3 --output <receipt.json>
```
