# Cardputer-Adv kernel validation and native investigation, 2026-11-25

Baseline: `461695ca`. Native executable mapping fix: `51bc71e2`; follow-up test isolation `e1bb3fb1` and codec/DMA `d28e343a` are listed in `verification.json`. These are source fixes in ESP Online Tester; the CardputerZero App and physical device were not changed or accessed.

## Behavior

Cardputer-Adv now uses the shared ES8311 control state, physical GPIO/IO_MUX gates, steady-state ADC/DAC gain, mute/reset/power controls and the existing S3 RX/TX GDMA engines. RX tests assert actual guest-memory bytes; TX tests assert DMA-produced PCM. S3 `bytes_in` counts payload writes, including a prefix before a later DMA fault, excluding descriptor writeback. Silence is still a real zero-valued transfer. The generic GDMA bool APIs remain available, with a counted variant for S3 I2S.

The native AArch64 CodeCache owns its `mmap` and now releases it with `munmap` on destruction. Reset still reuses its mapping. A child-process test checks 64 write/reset/move/drop cycles against actual OS VM state: `mach_vm_region` on macOS and `mincore` on Linux. An initial all-tests-in-parallel run exposed a test race: another core test can reuse the address between unmap and inspection. The test now executes alone in a child process; the initial failure is retained in the verification summary. No production code timing changes were made to solve the test race.

BoardModel adds two cold PCM hooks at the end of the vtable; existing slots stay in place. Only nonempty active audio reaches them. Default boards preserve PCM. Upstream PR #382 was inspected before introducing these hooks; its extra display/slot interfaces do not duplicate them.

## Native performance investigation

Prior art was checked before instrumentation: EX002 native JIT, EX161 coverage census (including the preserved sanitized `d54411ed` WASM source), EX159/EX164 call/return constraints and EX261 native helper counts. This retry changes the workload and backend census: Cardputer-Adv Wordle, native AArch64 helper executions. It is not a new call/return optimization or another timing model.

`native-census.patch` is diagnostic instrumentation only, applied to a separate source snapshot. It counts actual `h_exec` operations and prints totals at cache destruction. Production binaries contain none of these counters. Apply the zero-context artifact to `d28e343a` with `git apply --unidiff-zero native-census.patch`; its exact instrumented source hash and the original equivalent patch hash are retained in the census receipt. Its runtime is not used as a performance result. The patch preserves existing execution and delegates every operation to the same helper body.

At 480,000,000 guest cycles (2 s), the actual Wordle image retired 8,184,729 / 424,129,942 instructions. It made 149,977,645 native interpreter-helper calls: RETW.N 48,257,659; ENTRY 48,198,448; CALL8 47,807,701; CALLX8 316,892. Those window operations account for 96.4086% of helper calls, or 33.4458% of all retired guest instructions. There were 12,965 compiled blocks, 11,464 KiB of generated code and zero block-cache flushes. See `wordle-census.json`. The real captured framebuffer was inspected and shows the Wordle grid and prompt. Console and frame hashes are compared with the production binary and interpreter by `native_probe.py`.

A compiled-block instruction count includes its interpreter helpers: JIT enabled/active is consistent with this result. The two platforms' App diagnostics already show matching work and active JIT. The census, production baseline/candidate and `--no-jit` runs also match instruction counts, console SHA-256 and actual framebuffer PNG SHA-256. This census identifies a substantial native call/return helper count to investigate; a host census cannot establish what dominates on a low-end ARM core, attribute the cross-machine slowdown, or certify a speedup there. Existing call/return correctness constraints, window exceptions, budget cuts and timer delivery must be preserved before adopting code generation changes.

The user-supplied CM0/M4 timing and low-memory observations are context, not measurements by this task. No clock, governor, firmware-output or guest timing changes are adopted. The mapping release fixes lifetime leakage across destroyed machines; it is not a claimed steady-state throughput optimization. The 256 MiB code-cache size is virtual address reservation; only written pages become resident, so it must not be reported as 256 MiB resident RAM per core.

## Reproduction

`native_probe.py` accepts caller-supplied local paths, starts independent CLI processes, preserves the native quantum of 64 and compares actual instruction counts, console SHA-256 and framebuffer PNG SHA-256. It reports per-child user/system/wall time and peak RSS from `wait4`; it never connects to a device. Pair 0 is a warmup, then pair order alternates. `--oracle` separately executes `--no-jit` and requires the same work/output; its timing is not presented as a speedup candidate.

```sh
python3 docs/evidence/cardputer-adv-kernel-20261125/native_probe.py \
  --base "$BASE_BIN" --new "$NEW_BIN" --rom "$ROM" --flash "$WORDLE_MERGED_BIN" \
  --seconds 2 --pairs 3 --oracle --out output/wordle-pairs.json
```

Build baseline and candidate using `cargo build --release --bins` with the same stable toolchain and default release profile. Run the existing `docs/evidence/p4-cpu/cpu_pairs.py` with identical demo inputs for S3/C3/C6 hello and Pocket Tank. Timing starts only after task-owned builds/tests and coordinated C5 CPU work have stopped. This coordination cannot exclude other desktop work; anonymous load and run ranges remain part of the receipt. Aggregate deltas are not isolated codec or Drop costs. The first screen is retained as `*-contended.json`: C5 CPU work was paused, but the separate App build may have overlapped; it is not presented as an exclusive-host result.

## Coordinated CPU comparison

Apple M5 Max, macOS 27.2.1, rustc 1.100.0, default fat-LTO release builds, baseline `461695ca` versus `e1bb3fb1`. Three alternating pairs plus one warmup pair for each workload. Task-owned builds/tests stopped; C5 and App builds, host VMs and compression were explicitly paused. Light edits and device SSH/I/O were allowed; other desktop load was not excluded. Anonymous load and every sample are retained. These short samples are a regression screen, not a formal equivalence test or a low-end ARM speed measurement.

| Workload | Base median user s | New median user s | Change | New slower pairs |
| --- | ---: | ---: | ---: | ---: |
| S3 hello, 30 guest s | 0.230 | 0.230 | 0.00% | 0/3 |
| C3 hello, 30 guest s | 2.550 | 2.440 | -4.31% | 0/3 |
| C6 hello, 30 guest s | 1.550 | 1.560 | +0.65% | 2/3 |
| Pocket Tank, 8 guest s | 7.110 | 7.080 | -0.42% | 0/3 |
| Adv Wordle, 2 guest s | 2.313021 | 2.285299 | -1.20% | 1/3 |

All baseline/candidate instruction counts and console hashes match. Wordle framebuffer PNG hashes also match `--no-jit` and the instrumented census. The Wordle interpreter oracle used 3.102066 user s; it is an output check, not a proposed timing mode. New Wordle peak RSS median was 50,216,960 bytes; this is the CLI process on this host, not the App or a low-end device. Wordle's measured ranges overlap. Small changes in unrelated C3/C6 workloads are not attributed to codec or mapping release. No throughput optimization is adopted.

The earlier contended screen is retained in full: S3 0.00%, C3 -0.81%, C6 -0.65%, Pocket +0.42%; Wordle -1.5963% user median. Its external-build overlap motivated the coordinated repeat. Do not pool the two conditions or discard slower arms. `cpu-pairs.json` and `wordle-pairs.json` are the coordinated run; `*-contended.json` are the initial screen.

## Sources and support limits

The codec bit layout and steady-state dB values come from [Everest ES8311 Rev10.0, January 2021](https://files.waveshare.com/wiki/common/ES8311.DS.pdf), pp14,17–21,25,28, cross-checked against the retained Rev6.0 control subset. Physical pins follow [M5Stack Cardputer-Adv](https://docs.m5stack.com/en/core/Cardputer-Adv); S3 matrix indices follow [ESP-IDF v5.4.2 gpio_sig_map.h](https://github.com/espressif/esp-idf/blob/v5.4.2/components/soc/esp32s3/include/soc/gpio_sig_map.h), consistent with the model's v5.5.5 fields. Standard-register packing uses the existing S3 I2S model.

This is a digital subset: 16-bit effective PCM, left TX slot, no serial-edge/MCLK lock or power-settling simulation, calibrated analog gain/noise, EQ/HPF/ramp dynamics, ALC/DRC, full stereo/loopback, real microphone/speaker test or voice/cloud application acceptance. Ordinary Cardputer/other ES8311 boards do not inherit the Adv PCM wiring. See [runtime scope](../../platform/cardputer-adv-runtime.md).

Curated receipts retain hashes, work/output checks, run order, timing samples and aggregate environment details. Raw logs, native source snapshots, temporary binaries and profiles remain under ignored `output/cardputer-adv-upstream/`; machine names, login paths, unrelated process lists and session identifiers are excluded from committed evidence. No personal audio was collected. Verification results and any remaining limitations are listed in `verification.json`.
