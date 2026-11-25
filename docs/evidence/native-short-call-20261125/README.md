# Guarded native short calls

The default-off `native-short-call` feature reduces host dispatches for one already-decoded
`CALL8 → ENTRY a1,32 → RETW.N` leaf. It executes all three instructions through the existing
window helpers. On the Apple M5 Max, five alternating pairs reduce median user CPU by 4.40%
for accurate Wordle and 5.08% for CPI16 Wordle with a complete CRANE submission. Low-end ARM
benefit is unverified. UserDemo has no eligible fusion and its three measured menu pairs are
0.45% slower; the same-binary A/A is 1.74% slower. Those samples remain part of the result.

This follows [EX164](../../experiments.md#ex164). Its material difference from the retained
WASM `db7a3e50` leaf implementation is a bounded native helper, retaining actual window ISA
execution, rather than static window renaming inside a WASM region. It also follows the
dedicated window helpers in [EX002](../../experiments.md#ex002). The later timer-limit
arithmetic proposal and native MMIO-deferral candidates are outside this comparison.

## Source and correctness contract

The production baseline is `388d3aeec761b3ae13beb49e7b9399ec9423133a`; its metadata child
`1f018abc331ba4e9bc25f09cfef0097484735c80` changes only the catalog snapshot. The implementation
is `2493d549e16c691970f26c5b5506931acbe9c0fb`, corrected by
`c662de94dc0cd1f5219ed60374e38e9f606e36cd`. The final checked tree is
`678a7ad56c5855cd08b7f1a1eb167d5ac4014cf9`; `d35e12484ec143ef134cec78495a264522b847ed`
adds the same raw-matrix probe to both engines. Both use normal ROM boot.

Replacement code is appended between native executions, before retaining an entry or arena
reference. The previous mapping stays live. A callee must have been decoded normally and have
exactly the supported two-instruction shape. Every use checks mapping identities, both page
versions, the remaining budget, observers, probe bloom, CPU timing debt, bus timing debt and
bus-stop state. ENTRY/RETW retain overflow/underflow, loop-end and exception handling. Extra
retirement is returned to the generated caller; a pre-instruction trap uses the existing
unretired-exit protocol. An unsuccessful guard keeps the original call boundary.

The first prototype failed a directed counterexample: a prefix `L32I` queues 97 cycles of data
debt at CPI16 with CCOMPARE64. It returned four retired instructions before the reference
returned two. `c662de94` adds a non-mutating `Bus::timing_penalty_pending() -> Option<bool>`
tail default. `None` declines continuation; S3 peeks at its existing pending cache debt. The
getter never drains or requeues that debt. The failing source, overlay hash, command and
observed/reference returns are retained in [the counterexample](pending-debt-counterexample.json).

The feature is forwarded by `esp32s3` and the CLI. The private native helper table grows from
112 to 120 bytes when enabled, retaining old slots. Rust consumers require a complete rebuild;
public App C ABI and WASM export names are unchanged. Board wrappers, I2C hooks, audio and
CodeCache Drop remain the published implementations. No clocks, scheduler caps, MMIO-deferral
gate or CPI values change within a pair. This remains opt-in.

## Measured result

Rust 1.100.0, native AArch64, Apple M5 Max. Quantum64/VQ1/BB1; no frontiers; probe run budget
30000; real raw 25 ms/two-scan matrix input. CPI1 uses ordinary timing, CPI16 explicitly uses
the existing rough timing model. Compare only within a row. No guest counters or sample
instrumentation were added to the timed binaries. Child user/system CPU, wall time, RSS and
faults come from `wait4`; warmup pairs are excluded but retained. Subsequent pairs alternate
B→N/N→B. Cooperating tasks paused heavy host work; the whole machine was not exclusive.

| Workload | Pairs | Base median user s | Candidate median user s | Change | Candidate slower |
| --- | ---: | ---: | ---: | ---: | ---: |
| Wordle accurate, 2 guest s | 5 | 2.039117 | 1.949380 | −4.4008% | 0/5 |
| Wordle CPI16, 10 guest s, CRANE | 5 | 0.971882 | 0.922520 | −5.0790% | 0/5 |
| UserDemo CPI16, 10 guest s, Space/menu | 3 | 1.161104 | 1.166272 | +0.4451% | 3/3 |
| Same-binary accurate Wordle A/A | 3 | 2.043458 | 2.079006 | +1.7396% | 3/3 |
| S3 hello, 30 guest s | 3 | 0.235763 | 0.228434 | −3.1086% | 1/3 |
| C3 hello, 30 guest s | 3 | 2.442199 | 2.439150 | −0.1248% | 1/3 |
| C6 hello, 30 guest s | 3 | 1.541008 | 1.537868 | −0.2038% | 1/3 |
| Pocket Tank, 8 guest s | 3 | 7.041189 | 7.053082 | +0.1689% | 2/3 |

All 72 arms, including warmups, finish naturally in 151.995994 s. The suite has a 210 s limit.
Each workload matches per-core work, stop/cycles, exception/interrupt counts, console and real
PNG where available. Probe pairs additionally match complete CPU architectural dumps, ADC,
PC, JIT retirement, key FIFO/ACK state and actual input-edge cycles. Host compilation counts
and code-cache bytes may change with fusion; raw bytes are retained separately from the
architectural equality check. UserDemo code bytes are unchanged in its paired probe arms.

The Wordle measured ranges do not overlap. The A/A direction and UserDemo negatives prevent
a zero-cost or universal speed claim. Short hello samples have overlapping ranges. An earlier
contended CLI preflight is retained separately, including S3 +12.42%, C6 +1.79% and Pocket
+0.50%. Its initial parser failure did not produce an accepted performance result. Samples are
not filtered, corrected by A/A, or pooled with other implementations. These runs include boot;
they do not isolate a warm UI phase or measure host-to-device input latency.

The separate 14-arm functional campaign also checks accurate CRANE at five guest seconds,
FAST CRANE and the real UserDemo Scan/Record/Chat menu. Both CRANE PNGs were viewed for
letters and colors. Different CPI models produce different game state and are not compared
with each other. [All functional hashes and facts](functional.json).

## Checks and reproduction

[The exact local checks](local-checks.json) contain 30 passing steps: default and feature-on
native Clippy, WASM Clippy, 3526 reported Rust passes across 249 summaries, release workspace
build, actual native VQ checks, production WASM 14 manifests, all platform/worker/ABI checks,
WASM differential and profile builds. Five directed native tests include 25600 state/budget
cases, cuts1/2/3/63/64/65/30000, timers/wrap, window collisions, two-page changes/remaps,
probes, bus stops, pending debt, a suspended ordinary entry, clone/flush and cache lifetime.
The real S3 cache getter is checked before and after settlement without consuming it.
The rebuilt production WASM is 8911855 bytes, SHA-256
`e019f308adc0b8eca252165db3ad09438d149bf119cdb5477b35cff55c378cd9`.
Remote CI and ARM acceptance are recorded separately when available.

Build the baseline at388 with the probe source copied unchanged fromd35. Build the candidate
with `cargo build --release --bins --features native-short-call` and
`cargo build --release -p esp32s3 --example native_short_call_probe --features native-short-call`.
Keep the baseline CLIs under their ordinary names and its example as `probe-control`; keep
candidate CLIs as `esp32sim-short-call`, `esp32sim-c3-short-call`, `esp32sim-c6-short-call` and
the example as `probe-short-call`. The files' measured hashes, input hashes and original
runner hash are in [inputs and binaries](inputs-and-binaries.json). Fetch the repository ROMs,
demo assets and pinned Pocket model first. Caller assets contain the pinned normal-ROM ELF,
`wordle-merged.bin` and `userdemo.bin` with the recorded hashes.

```sh
python3 docs/evidence/native-short-call-20261125/cpu_pairs.py \
  --repo /path/to/esp32sim --assets /path/to/caller-assets \
  --base-bins /path/to/frozen-base --new-bins /path/to/frozen-candidate \
  --mode formal --out /path/to/new-results --limit-seconds 210
```

Each workload has a compact JSON retaining every numeric sample, order and correctness hash.
[Curation provenance](curation.json) identifies the original/sanitized hashes. Paths and PIDs
are removed or parameterized; edge arrays become hashes. Complete raw output, actual PNGs,
console and architecture dumps remain in ignored `output/native-short-call-revision2-20261125/`
and are preserved before worktree cleanup. No device or private ARM capture was accessed.
