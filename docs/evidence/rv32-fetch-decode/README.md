# EX249: C6 instruction fetch and RV32 decode inline into the step

Base: main `ddf753a5`. Toolchain 1.99.0, release profile unchanged. ARM64 macOS host.

The branch was rebased onto `66eb9704` with code range-diff `=`.

## What

The C3/C6/P4 core is a plain interpreter. On the C6, every executed instruction called `SocBus::fetch` (out of line, four bounds-checked byte loads merged into the result) and then `decode32` or `decode_compressed` (out of line, the `Insn` returned through the stack). On a 5 s `sample` of c6-energy-scan on base these three were the top-of-stack for 48.9% of main-thread samples (`decode_compressed` 21.8%, `fetch` 15.7%, `decode32` 10.9%; local profile, not committed).

Two changes, no new state:

1. C6 `Bus::fetch` is `#[inline(always)]`, and a window that lies entirely inside its buffer is read as one 4-byte slice. A window cut short by the end of SRAM, ROM, LP SRAM or the flash image takes the old byte loop, so the bytes past the end still read as zero. A window crossing a mapped flash page within the image reads the following physical bytes through the same resolution as before; MMU resolution is unchanged.
2. `decode32` and `decode_compressed` are `#[inline(always)]` into `decode_for`. This is the shared RV32 crate, so the C3 and P4 steps get the inlined decoders too (their fetch is unchanged).

The decoders, their results and the order of every bus access are unchanged.

## Exact work

`exact.py <out> 10 <base-probe> <candidate-probe>` with the local post-run retired-count report (the edit applied by `probe.py` is not committed), 10 guest seconds, ROM boot. Identical: retired instructions, scheduler work, interrupt totals and per-line sources, stderr (wall time removed) and console SHA-256 ([exact-work.json](exact-work.json)).

| Fixture | Retired | Interrupts | Console SHA-256 |
| --- | ---: | ---: | --- |
| c6-hello | 3,657,500 | 1,010 | d5d1f41f… |
| c6-contiki | 61,053,934 | 12,994 | 101f1060… |
| c6-energy-scan | 895,091,212 | 28,722 | 720c0e65… |
| public display starter | 373,050,392 | 10,346 | 1e6395a8… |
| c3-hello (`esp32sim-c3`) | – | 1,009 | 6dd87f0c… |
| p4-hello (`esp32sim-p4`) | – | 1,011 | db6c8bcf… |

C3 and P4 rows compare the unprobed release binaries (scheduler work, interrupts, stderr, console).

## Hot-path disassembly

`codegen.py <base-bins> <candidate-bins> codegen.json` compares main `2e36224d` with the rebased change over four release binaries (esp32sim, -c3, -c6, -p4), 79 selected symbols each, normalized for addresses, data page offsets and import stubs ([codegen.json](codegen.json)). 73/79 are identical in every binary. The six differences are the intended ones: the three RV32 `step_outcome` instantiations grow (C6 915 → 1854, C3 915 → 1731, P4 946 → 1771 instructions) and `decode32`, `decode_compressed` and the C6 `fetch` no longer exist as separate functions. Classic and S3 run loops, `step_blocks`, every bus tick, deadline and device function are identical after normalization. `python3 test_codegen.py` checks zero page offsets.

## Mutations

`mutate.py <scratch-worktree> <out>`: 6/6 killed by a failing test, none by compile errors ([mutations.json](mutations.json)). Fast window byte order, shifted window, last byte dropped, window clamped to the buffer, short window read as a whole one, and the zero fill of the tail all fail `bus::fetch_tests`. The inline attributes have no functional rule to mutate; the disassembly above records them.

## Daytime smoke (not a CPU table)

`smoke.py smoke.json 10 <main> <candidate>`: user CPU from `wait4`, 10 guest seconds, 1 warmup pair + 3 alternating pairs, 1-minute load 4.2–5.1 from unrelated work ([smoke.json](smoke.json)). A main-vs-main control ran right after it, load 2.8–4.1 ([smoke-aa.json](smoke-aa.json)).

| Fixture | Main median s | Candidate median s | Change | Candidate slower | A/A change |
| --- | ---: | ---: | ---: | ---: | ---: |
| c6-hello | 0.571 | 0.578 | +1.3% | 2/3 | −0.9% |
| c6-contiki | 1.492 | 1.476 | −1.1% | 1/3 | +0.5% |
| c6-energy-scan | 10.546 | 9.498 | −9.9% | 0/3 | +1.9% |
| public display starter | 3.657 | 3.271 | −10.6% | 0/3 | +0.4% |

The gain is on fixtures that execute instructions; the idle-heavy ones spend their time in idle rounds and stay within the A/A spread.

## Variants measured and not adopted

Rotating daytime comparisons (1 warmup + 3 rounds, loads 2–63, so only large differences mean anything; [explore.json](explore.json)). Names: `f1` one-slice fetch window only; `f2` plus `#[inline]` (fetch stayed out of line); `f3` plus `#[inline(always)]`; `d0` inlined decoders only; `f1d`, `f3d` combinations (`f3d` is this change, also `lever4`); `cand`/`m1024`/`m16384` a direct-mapped decode memo keyed by PC, fetched word and `misa.F` (4096/1024/16384 entries) instead of inlining; `f2m`, `f3m` fetch variants plus that memo.

The decode memo is not adopted. A local hit counter showed 90% hits on c6-energy-scan (76.6M misses by the 671M-th hit) and 99.5% on the starter, yet a hit cost nearly what decoding did: −6% on the starter, −3% to +3.5% on c6-energy-scan across runs, and nothing on top of `f3`. It also adds 112 KiB per core. Neither part alone matched `f3d` on c6-energy-scan in the same rounds.

## Checks

- `cargo +1.99.0 clippy --workspace --all-targets -- -D warnings`: pass.
- `cargo +1.99.0 clippy --release --target wasm32-unknown-unknown -p esp32sim-wasm --features jit-tests -- -D warnings`: pass.
- `ESP32SIM_ROM_DIR=<fw> cargo +1.99.0 test --release --workspace --no-fail-fast -- --include-ignored --skip external_` (empty HOME): 1323 passed, 0 failed.
- `cargo +1.99.0 test --release --workspace --no-fail-fast` (empty HOME, no ESP32SIM variables): 1270 passed, 0 failed, 76 ignored.
- `RUSTUP_TOOLCHAIN=1.99.0 tools/wasm-build.sh && node tools/wasm-test.mjs hello c3-hello c6-hello c6-energy-scan c6-contiki c6-contiki-net c6-rpl-net panel`: pass.
- `node tools/check-evidence-privacy.mjs`: pass.

Goldens unchanged.

## Limits

C3 is timed in the maintainer run below; P4 is checked for exact work and disassembly, not timed. The display starter is a public C6 LCD starter image (hashes in [inputs.json](inputs.json)), not committed. Idle rounds, deadlines and the machine loop are untouched; the step itself (`step_outcome`, [EX168](../../experiments.md#ex168)'s step facts) is unchanged in source and only grows by the inlined fetch and decoders.

## CPU comparison

Maintainer's run ([PR #310 comment](https://github.com/mikroverk/esp32sim/pull/310#issuecomment-6540590356)), October 31, 2026: `cpu_pairs.py` from `docs/evidence/p4-cpu` on base `66eb9704`, new `f91b3908`. Trees extracted with `git archive` into sibling paths of equal length and built with `cargo build --release --bins`; rustc 1.99.1 (58cf8f9c3 2026-10-13), aarch64-apple-darwin, Apple M5 Max (18 cores), macOS 27.0.1. ROMs: esp-rom-elfs 20260528. One warmup pair and seven alternating pairs per workload; each started at 1-minute load below 5 with no build running. `c6-wifi-station` is the script's own workload. User CPU:

| workload | base median (range) | new median (range) | change | new slower | instructions, console | load before → after |
| --- | --- | --- | --- | --- | --- | --- |
| s3-hello (30 s) | 0.260 s (0.260–0.260) | 0.260 s (0.250–0.260) | +0.0% | 0/7 | identical in all 16 runs | 4.40 → 4.21 |
| c3-hello (30 s) | 2.530 s (2.460–2.530) | 2.510 s (2.460–2.570) | -0.8% | 2/7 | identical in all 16 runs | 4.21 → 4.27 |
| c6-hello (30 s) | 1.590 s (1.580–1.630) | 1.600 s (1.590–1.650) | +0.6% | 5/7 | identical in all 16 runs | 4.27 → 4.70 |
| pocket-tank (8 s) | 7.200 s (7.170–7.240) | 7.180 s (7.130–7.240) | -0.3% | 2/7 | identical in all 16 runs | 4.70 → 3.38 |
| c6-wifi-station (14 s) | 1.070 s (1.050–1.080) | 1.030 s (1.010–1.050) | -3.7% | 0/7 | identical in all 16 runs | 3.38 → 3.62 |

The maintainer reports nothing flagged (new slower ≥6/7 is the flag). C6 Wi-Fi station was 3.7% faster, 0/7 slower; C6 hello was +0.6%, 5/7 slower with overlapping ranges, noise at this resolution. Energy-scan and the display starter are absent from this run; their busy-fixture gains above are our separate daytime measurements.

[Posted JSON](maintainer-cpu-f91b3908.json): the maintainer replaced only the ROM directory with `$ESP32SIM_ROM_DIR`; he reported raw SHA-256 `b076fc07…`, sanitized `2d015ea1…`. The raw file was not provided; this redaction removes the local ROM path without changing measured values. The JSON extracted from the posted code block has SHA-256 `04b22c25bb539ddbd10cd88082a86fef21f9b8c9f87d28a559739810f21ad8c0`, not the reported sanitized prefix; its serialization may differ.
