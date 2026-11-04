# EX250 disabled-flight native CPU comparison

This receipt tests the native runtime with flight disabled, comparing main
`8b20e74a45a1095a3c3bb3b61100493766a1969f` with integrated flight revision
`160c7a6d815a1c76f28cc01e83a2eec27096fd02`. Its Rust/Cargo fingerprint is unchanged
from `14d96b3a`. The baseline fingerprint is the previous final Stamp integration
fingerprint, not the older `49a27787` main used by EX248.

This is the disabled control for EX250, not a scheduling optimization. The
protocol follows [EX248's preserved final comparison](../s3-coprocessor-reset/integrated-cpu-final.md),
including every warmup and alternating pair. [EX117](../../experiments.md#ex117) concerns host publication and
worker cadence; these native measurements do not measure those browser paths.
[EX249](../../experiments.md#ex249) distinguishes RV32 idle slots from retired instructions; the same counter
qualification applies here. All earlier negative results, harness errors and
noise observations remain unchanged in their original receipts.

## Source and build boundary

The native source difference is confined to two appended cold `BoardModel`
methods and StampFly implementations/tests. The third changed Rust file exposes
WASM flight sensor/motor APIs. There is no native bus, device tick, scheduler or
execution-core change. The worker's default-disabled branch assigns
`runCycles = wasm.esp32sim_run`; only an explicitly enabled flight scene replaces
that reference with the flight runner. This source audit does not by itself
prove zero overhead or measure JavaScript startup/dispatch cost.

Both native arms are fresh builds from separate read-only Git archives. Each
archive contains Cargo.toml, Cargo.lock and every workspace member; all 555
archived files were verified against the pinned Git blobs. Outputs use private,
separate target directories. Both builds use stable Rust 1.99.1 (`58cf8f9c3`,
LLVM 23.1.1), optimization 3, fat LTO, one codegen unit and debug level 1:

```sh
# For each pinned revision, choose an empty, independent source/target directory.
mkdir "$ARM_SOURCE"
git -C "$REPO" archive "$REV" Cargo.toml Cargo.lock \
  emu-core esp-periph esp-soc xtensa-lx7 esp32s3 cli wasm wasm-jit riscv-rv32 \
  esp32c3 esp32c6 esp32 esp32p4 esp32s31 esp32c5 network-helper \
  | tar -xf - -C "$ARM_SOURCE"
chmod -R a-w "$ARM_SOURCE"
CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR="$ARM_TARGET" cargo +stable build --release --locked \
  --manifest-path "$ARM_SOURCE/Cargo.toml" \
  --bin esp32sim --bin esp32sim-c3 --bin esp32sim-c6
```

No Rust flags, profile overrides or wrappers are injected. Completed executables
are copied into separate read-only `$BASE_BINS` and `$CANDIDATE_BINS` directories.
The 20 ROM, firmware, model and manifest inputs are independent read-only copies
whose hashes match EX248's final frozen inputs. [Build and input receipt](inputs.json)
records exact artifacts, source fingerprints, compiler metadata and commands.
ROM/demo inputs can be fetched with `tools/fetch-demo-assets.sh --no-linux` and
Pocket Tank inputs with `tools/fetch-pocket-tank.sh`; verify the resulting files
against the receipt before copying them into `$FROZEN_ASSETS`. These commands
are not run during measurement.

## Measurement method

Each workload runs for 30 guest seconds: S3 hello, Pocket Tank, C3 hello and C6
hello. One warmup per arm precedes five measured pairs; pairs alternate
base/candidate and candidate/base. Every sample is retained. User CPU is the
`RUSAGE_CHILDREN.ru_utime` delta around one child process; no other child runs in
that measurement interval. System CPU, wall time and before/after machine-wide
load averages are recorded separately. Load average is not CPU utilization.

The [reproducible harness](disabled-cpu.py) derives from EX248's harness with only
experiment/build metadata and the native-option comment updated. Its timing
loop, input flags, pair order and equality checks are unchanged:

```sh
python3 docs/evidence/stampfly-flight-disabled-cpu/disabled-cpu.py \
  --base "$BASE_BINS" --candidate "$CANDIDATE_BINS" \
  --base-revision 8b20e74a45a1095a3c3bb3b61100493766a1969f \
  --candidate-revision 160c7a6d815a1c76f28cc01e83a2eec27096fd02 \
  --assets "$FROZEN_ASSETS" --repo "$REPO" \
  --pairs 5 --workloads s3-hello,pocket-tank --output s3-pocket.json
# Repeat with --workloads c3-hello,c6-hello --output controls.json
```

Equality checks reported per-core instruction/dispatch counts, cycles, guest
duration, exceptions, interrupts and the SHA-256/length of unmodified UART0
stdout. RV32 counts include WFI idle slots and must not be described as retired
instructions. This receipt measures no C5 workload or enabled-flight throughput.
Raw guest consoles are omitted after hashing; the receipts contain no private
process inventories, host/user identity or absolute local paths.

## Results

All 48 runs completed with exit 0 and stopped at 30 guest seconds. Every
work/output signature, including all warmups, matches between arms and matches
the corresponding historical EX248 workload. Each median excludes warmups and
uses five measured samples. “Slower pairs” counts candidate user CPU exceeding
the paired baseline; it is not a significance test.

| Workload | Main median user (s) | Candidate median user (s) | Change | Main range (s) | Candidate range (s) | Slower pairs |
| --- | ---: | ---: | ---: | --- | --- | ---: |
| S3 hello | 0.293279 | 0.284930 | -2.847% | 0.286167–0.300507 | 0.272958–0.292156 | 1/5 |
| Pocket Tank | 46.916614 | 46.282062 | -1.353% | 45.816245–47.024651 | 46.078087–47.643462 | 3/5 |
| C3 hello | 3.669366 | 3.731153 | +1.684% | 3.449779–3.766911 | 3.631510–3.810326 | 3/5 |
| C6 hello | 1.971921 | 1.991233 | +0.979% | 1.951411–2.049872 | 1.953810–2.145538 | 4/5 |

The [S3/Pocket Tank receipt](s3-pocket.json) and [C3/C6 receipt](controls.json)
retain every individual sample, paired change, order, user/system/wall time and
load sample. Summed child wall time was 655.504177 seconds. No additional timing
batch was selected, and no sample was removed.

The positive C3 and C6 median changes remain visible. C3's first measured pair
was **+10.451%**, while its later pairs range from −1.503% to +2.039%; the largest
increase was not discarded. Pocket Tank's median ratio is −1.353% even though
three of five paired runs are slower and the median paired change is **+1.017%**.
These are different summaries of the same samples, not contradictory raw data.
All four measured ranges overlap; that overlap does not prove equivalence.
This batch neither proves zero disabled cost nor isolates a regression or speedup.

| Workload | Reported per-core work counts | Cycles | Console SHA-256 |
| --- | --- | ---: | --- |
| S3 hello | 11,990,305; 6,798,164 | 7,200,000,000 | `c74f198e19e94c47772b3403433b5532057180aa63df46ff90ffe9a51710966f` |
| Pocket Tank | 4,595,734,479; 6,171,560,740 | 7,200,000,000 | `5f9d2aecbb20769a9f076516c33955f82a9fa13fba79249edde134aed8986e0a` |
| C3 hello | 4,800,000,000 | 4,800,000,000 | `3b9d8d3eb053a97670f11896674e70ffc968fd7a803cb2cdc5c051414af94efd` |
| C6 hello | 4,800,000,000 | 4,800,000,000 | `bd69fbb9ac5c677337a293c1ec0ff7941306eec82c499290917a7c42360f86ad` |

Equality additionally checks exception/interrupt counts, UART byte length, guest
duration and stop reason. RV32 work counts include idle dispatch slots as noted
above; they are not retired-instruction throughput measurements.

## Conditions, audit and limits

The host was Apple M5 Max with 18 logical CPUs, macOS 27.0.1. Concurrent project
builds, tests, guest execution, browser rendering and model generation had stopped
before timing. Lightweight text/JSON curation and an idle static HTTP service
remained. The host was shared and not isolated: one-minute load ranged from
**3.621 to 12.996**, rising during the later controls. The exact before/after
load samples remain in the receipts. No affinity, fixed-frequency or thermal
control was applied. User CPU excludes descheduled time but remains sensitive
to core placement, frequency, cache effects and compiler/code layout. Five pairs
are a small sample, especially for the subsecond S3 hello.

The source/build audit found the same compiler, configuration, release profile,
features and empty Rust flags in both native CLI Cargo fingerprints. No runtime
`ESP32SIM_` or `SIM_` environment overrides were set. All six frozen executables,
20 inputs and both 555-file source archives matched their recorded hashes/Git
blobs after timing. Integration revision `0c81b420` retained the candidate's
Rust/Cargo fingerprint. The source changes remain the cold board/API additions
described above. No command, work-budget, input or build-feature mismatch was
found; no extra timing was justified by an identified harness flaw.

This evidence supports exact disabled-native guest work and UART0 preservation for
these four inputs. It does not qualify enabled-flight physics, JavaScript/worker
CPU cost, display cadence, browser frame rate or physical motor/sensor timing.
Those have separate correctness and browser receipts. Earlier EX117, EX248 and
EX249 results are neither overwritten nor reinterpreted by this new control.

## Validation and privacy

Independent receipt checks recomputed medians, ranges and percentages, checked
pair order and all 48 signatures against the historical records, and rehashed
the frozen sources, binaries and inputs. Each JSON receipt is below 50 KB.
`git diff --check` and `node tools/check-evidence-privacy.mjs` are run before commit.
Only relevant source/build metadata, normalized commands, hashes and numerical
samples are retained. Raw guest consoles and private process/session inventories
are absent; exact output equality uses hashes computed before omission. The
original historical evidence remains unchanged.
