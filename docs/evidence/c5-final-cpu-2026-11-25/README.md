# Native non-C5 regression controls

All 24 runs have identical stop fields, per-core instruction counts, guest duration,
cycles, exceptions, interrupts and console stdout SHA-256 within each workload.
Median user CPU increased for S3 hello (+2.03%) and Pocket Tank (+3.66%); those
observations are retained. Every workload has overlapping ranges and only one
of three pairs with a slower candidate. These are short controls on a shared host. They do not establish an isolated
speedup, zero overhead or exclusive-host behavior.

Measured baseline: `930731a2fb52cfecb9eb43fd4c2651c7c74e1e70`.
Measured candidate: `c8232bfcc3464bd62bdafbc93c18d096c169d6f5`.
The public production-source correspondence is recorded in the
[final source identity](../c5-final-integration-2026-11-25/source-identity.json).
A later native-example-only change was not timed; it does not replace the
measured source identity. Source/tree/native-manifest identities, all ten frozen
CLI hashes and all input hashes are in [result.json](result.json).

| Workload | Guest s | Baseline median user s | Candidate median user s | Change | Baseline range s | Candidate range s | Candidate slower |
| --- | ---: | ---: | ---: | ---: | --- | --- | ---: |
| hello | 3 | 0.045771 | 0.046698 | +2.03% | 0.045277–0.061714 | 0.045272–0.059217 | 1/3 |
| c3-hello | 3 | 0.274555 | 0.271985 | -0.94% | 0.274236–0.289326 | 0.270950–0.305176 | 1/3 |
| c6-hello | 3 | 0.177577 | 0.176956 | -0.35% | 0.173940–0.194845 | 0.175632–0.180485 | 1/3 |
| pocket-tank | 8 | 7.810150 | 8.095616 | +3.66% | 7.272432–8.803564 | 7.268268–8.402413 | 1/3 |

The default release profile is opt-level 3, debug 1, fat LTO and one codegen unit,
with rustc 1.100.0 targeting aarch64-apple-darwin. Each workload has three pairs in
AB, BA, AB order. Every sample, including the first, remains in the medians.
The declared windows are three guest seconds for each hello and eight for Pocket;
this is not the longer 30-second example from CONTRIBUTING. No guest clock,
scheduling quantum or experimental execution option was changed.

The shared-host window paused known compilation/test/VM work through coordination;
other host activity was not inventoried or excluded. All per-arm anonymous 1/5/15
minute load averages are retained. No CPU affinity, priority or frequency control
is claimed. The launch-to-completion interval was 54.424 seconds within a 150-second
guard. CPU was explicitly returned before report processing; no retiming followed.

## Public-harness reproduction

The existing `tools/compare-display-cpu.py` has SHA-256
`b6e0034224e2b5f376213cc0ad1a03a946694304b9772e672003643fbf4a5a39`.
Build both source revisions with matching stable compiler/default release flags
and supply the frozen binary and firmware/ROM directories:

```sh
python3 tools/compare-display-cpu.py \
  --baseline "$BASE_BIN_DIR" --candidate "$CAND_BIN_DIR" \
  --firmware "$FW_DIR" --roms "$ROM_DIR" \
  --rounds 3 --seconds 3 --workload hello --workload c3-hello \
  --workload c6-hello --output "$OUT/hello-3s.json"
python3 tools/compare-display-cpu.py \
  --baseline "$BASE_BIN_DIR" --candidate "$CAND_BIN_DIR" \
  --firmware "$FW_DIR" --roms "$ROM_DIR" \
  --rounds 3 --seconds 8 --workload pocket-tank \
  --output "$OUT/pocket-8s.json"
```

Input identities are pinned in the result; ROMs and the Pocket model must be
prepared through the repository's pinned fetch instructions. The public harness
reproduces workload arguments and strict work/console checks. This run's private
thin capture wrapper added raw retention, order and anonymous load outside the
original CPU/wall measurement interval. Only its hash, byte size and caller-
parameter command are provided in [curation.json](curation.json); its source and
local paths are not included. Raw UART/stdout/stderr streams remain outside Git,
with all 48 stream byte counts/hashes retained. No PNG was generated for timing.

## Original counter discrepancy and curation

The original combined summary inherited `guestRunsStarted=0` from its planning
object and did not update that field. Separately, it contains 24 actual executed
arms; the two phase receipts contain 24 matching samples and 48 raw streams verify
by hash. This receipt explicitly reports `actualRuns=24` and preserves the stale
original field as a known metadata defect. Neither the original summary nor the
runner or samples was overwritten, and no run was repeated to repair metadata.

[result.json](result.json) retains all 24 original numeric samples, actual order,
anonymous loads and strict equality fields. [curation.json](curation.json) records
original artifact hashes, omitted private fields and the effect of curation.
Private paths, process/session identifiers, raw console content and unrelated
inventories are omitted; no numeric sample was changed or discarded.
