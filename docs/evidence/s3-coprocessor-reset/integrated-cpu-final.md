# EX248 final Stamp integration CPU comparison

This is the final-source follow-up to the [initial integrated comparison](integrated-cpu.md),
not a new execution mechanism or experiment ID. The baseline remains
`49a277876d7f276d4b7d576e2bc167ffa9078482`; the candidate is
`aca1effdfc05e8f62c87ad23b97be46c09399a79`, whose Rust/Cargo source fingerprint
matches the integrated native sources since `f27e2340`. The initial results,
S3 confirmation, harness errors and uncertainty remain in their original receipts.
This comparison covers S3 hello, Pocket Tank, C3 hello and C6 hello. It does not
benchmark the new C5 backend.

## Frozen inputs and build alignment

The three baseline executables are the preserved read-only binaries from the
initial comparison, verified byte for byte against its recorded SHA-256 hashes.
The candidate executables were copied from the completed integration release
build and made read-only. All 20 ROM, firmware, model and manifest inputs match
the initial receipts and were copied to an independent read-only asset tree.
No executable or input is rebuilt or rewritten during timing.

All 526 candidate Rust/Cargo files matched their pinned Git blobs before freezing.
Their latest filesystem modification preceded all three candidate executable
modification times. Exact hashes, timestamps, source fingerprints and build
settings are retained in [the frozen-input receipt](integrated-cpu-final-inputs.json).
Both builds use stable Rust 1.99.1 (`58cf8f9c3`, LLVM 23.1.1), optimization 3,
fat LTO, one codegen unit and debug level 1. The build scopes differ explicitly:

```sh
# Historical baseline build, reused here without rebuilding:
CARGO_TARGET_DIR="$BASE_TARGET" cargo +stable build --release --locked \
  --manifest-path "$BASE_SOURCE/Cargo.toml" \
  --bin esp32sim --bin esp32sim-c3 --bin esp32sim-c6
# Final candidate build, already completed before freezing:
RUSTUP_TOOLCHAIN=stable CARGO_BUILD_JOBS=3 cargo build --release --workspace
```

No additional features or Rust flags were used. Repository, workspace-parent and
home Cargo configuration files were absent. The receipt distinguishes the exact
candidate workspace build command from the historical harness's build-command
template; the measured binary hashes identify the actual artifacts.

## Fixed-work method

The unchanged [harness](integrated-cpu.py) runs each workload for 30 guest seconds.
One warmup per arm precedes five measured pairs, alternating baseline/candidate
and candidate/baseline. Every sample is retained. User CPU is the child process's
`RUSAGE_CHILDREN.ru_utime` delta; wall and system times are recorded separately.
The child stdout bytes are hashed unmodified, then omitted. Equality includes
reported per-core counts, cycles, guest duration, exceptions, interrupts, console
length and console hash, and is also checked against the original receipts.

```sh
python3 docs/evidence/s3-coprocessor-reset/integrated-cpu.py \
  --base "$BASE_BINS" --candidate "$CANDIDATE_BINS" \
  --base-revision 49a277876d7f276d4b7d576e2bc167ffa9078482 \
  --candidate-revision aca1effdfc05e8f62c87ad23b97be46c09399a79 \
  --assets "$FROZEN_ASSETS" --repo "$CANDIDATE_SOURCE" \
  --pairs 5 --workloads s3-hello,pocket-tank --output integrated-cpu-final.json
# Repeat with --workloads c3-hello,c6-hello --output integrated-cpu-final-controls.json
```

Reported `insns` is the existing CLI work counter; on RV32 it includes WFI idle
slots. Matching that counter is an equal-work check, not a claim that every slot
retired an instruction. CPU measurements use host user time, not that count.
There is no C5 throughput inference from its nominal 7.2-billion-slot idle run.

## Results

All 48 runs completed with exit 0 and stopped at 30 guest seconds. Every
work/console signature equals the corresponding initial receipt, including the
warmups. All binary and input hashes remained unchanged after timing. These
measurements replace no historical samples.

| Workload | Main median (s) | Final median (s) | Change | Main range (s) | Final range (s) | Final slower pairs |
| --- | ---: | ---: | ---: | --- | --- | ---: |
| S3 hello | 0.253883 | 0.257796 | +1.541% | 0.249546–0.258974 | 0.252057–0.263410 | 4/5 |
| Pocket Tank | 45.396449 | 45.446335 | +0.110% | 44.637512–45.749740 | 44.316916–45.503333 | 2/5 |
| C3 hello | 3.530116 | 2.790922 | -20.940% | 3.478897–3.655980 | 2.742147–3.001662 | 0/5 |
| C6 hello | 1.912960 | 1.916463 | +0.183% | 1.893977–1.944072 | 1.903854–1.991992 | 3/5 |

Each median uses five measured samples, excluding the two retained warmups per
workload. All individual samples, paired percentage changes, run order, user/
system/wall times and load samples are in the [S3/Pocket Tank receipt](integrated-cpu-final.json)
and [C3/C6 receipt](integrated-cpu-final-controls.json). The summed child wall time
was 606.583388 seconds. No extra confirmation batch or timing sample was added.

The S3 increase remains visible: four of five paired candidate runs are slower,
with a median increase of 0.003913 CPU seconds. Pocket Tank and C6 have small
positive median changes and mixed pair directions. Their overlapping ranges do
not establish performance equivalence, and the samples do not isolate a source
regression.

C3 is lower in all five measured pairs, with nonoverlapping measured ranges and
paired decreases of 14.362%–24.213%. This is an observation of this fixed batch,
not evidence that a particular backend addition makes C3 faster. No controlled
ablation, independent host replication, affinity, clock or thermal experiment
was performed. It would be incorrect to suppress this result or to present it
as an attributed optimization.

## Conditions, source audit and limits

The host remains Apple M5 Max, 18 logical CPUs, macOS 27.0.1. The measurement
window excluded concurrent project builds/tests, browser rendering and model
generation; only lightweight text/JSON curation continued. It was still a shared
host, not an isolated machine. One-minute load across all before/after samples
was **3.369–5.355**, compared with 8.81–38.75 in the initial comparison. Load
average is not CPU utilization. User CPU excludes descheduled time but still
depends on core placement, frequency, caches and compiler layout. Five pairs
remain a small sample, especially for the subsecond S3 workload.

The unchanged baseline itself has lower medians than the first batch: S3
−2.628%, Pocket Tank −30.169%, C3 −22.493% and C6 −13.016%. This confirms a
substantial difference between the two measurement periods. The first receipt's
S3 confirmation and its own baseline drift are retained; neither period is
selected to hide an increase or to claim a performance-equivalence pass.

Source review of `49a27787..aca1effd` finds no changes to the RV32/Xtensa execution
cores or the scheduling loop. The shared `Machine` changes are cold app-boot and
reset exception-budget updates, which default to no change for S3/C3/C6. Other
changes include board construction and board-specific devices, the S3 CPENABLE
reset correction, the independent C5 backend, CLI loading/registration, and the
shared GP-SPI software interrupt-set register correction. C3's bus and core are
unchanged; its setup now selects the same bare `NoBoard` and uses the C3 GP-SPI
constructor. No timing-model or instruction-count flag is enabled by these
commands. The equal-work/output checks establish that these workload results
were not obtained by omitting guest work.

The build audit found matching native CLI compiler/config/profile identifiers
and empty Rust flags in both retained Cargo fingerprints. C3, RV32 and `esp-soc`
have no enabled native release features. The S3 cache-inline feature requires
`wasm32` in the bus and applies to the Wasm JIT module. The differing build scopes
and larger link contents remain explicit; code-generation/layout effects are
not ruled out or measured separately. No concrete harness or build-boundary
failure was found to justify more timing runs.

The current main revision `c2c7ddd0` changes only unrelated catalog metadata from
the pinned baseline, and its Rust/Cargo fingerprint is identical. Post-measurement
integration revision `928cfa9c` also retains the frozen candidate fingerprint.
All 526 working-tree Rust/Cargo files matched their pinned blobs after timing.

## Receipt validation and privacy

Validation independently recomputed the medians, percentage changes and pair
order; checked all 48 signatures against the initial receipts; and rehashed all
six executables and 20 frozen inputs after timing. It also checked current-main
and final-candidate source equivalence. Each JSON file is below 50 KB.

Curated receipts contain only relevant revisions, hashes, timestamps, compiler/
profile data, numerical samples and normalized commands. Raw guest output is
hashed before omission; no private console, process inventory, host/user identity
or local absolute path is retained. The preserved historical receipts are
unchanged. `git diff --check` and the evidence privacy checker are run before
committing this follow-up.
