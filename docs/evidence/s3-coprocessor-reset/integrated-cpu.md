# EX248 integrated StampFly CPU comparison

This follow-up compares main `49a277876d7f276d4b7d576e2bc167ffa9078482`
with integrated StampFly revision `81d419f2959191fdff5a6d1fb062c7c4321dedb4`.
The candidate's Rust and Cargo files are unchanged from `07564197` through
`81d419f2`. This extends the original EX248 correctness-only comparison to the
integrated board and sensor support and measures user CPU cost with equal work.
It does not retry an execution or scheduling optimization.

All 60 completed runs (including warmups and the S3 confirmation) preserve each
workload's instructions, cycles, exception/interrupt counts and console hash.
CPU results are noisy: the initial S3 hello increase is retained, while its
confirmation changes both the baseline cost and the relative difference. No
performance-equivalence pass, isolated regression or attributable speedup is
claimed.

## Method and reproduction

Both arms were built with Rust stable 1.99.1 (`58cf8f9c3`, LLVM 23.1.1),
`aarch64-apple-darwin`, and the repository's release profile: optimization 3,
fat LTO, one codegen unit, debug level 1. The baseline was extracted from the
pinned commit using `git archive` into a separate temporary source directory,
made read-only, and built with its own `CARGO_TARGET_DIR`. The candidate was
rebuilt from its clean native sources. Executables were copied to separate
directories and made read-only before timing. The earlier EX248 baseline binary
`6e7b6cf8adcb8f442b9a6879afc556eb38d8372e2d3023ff01152fee35be5a34`
was verified but was not reused: this follow-up uses three fresh baseline
executables from the same build command. The receipt records the actual measured
binary hashes and source fingerprints, rather than substituting the old hash.

```sh
CARGO_TARGET_DIR="$BASE_TARGET" cargo +stable build --release --locked \
  --manifest-path "$BASE_SOURCE/Cargo.toml" \
  --bin esp32sim --bin esp32sim-c3 --bin esp32sim-c6
cargo +stable build --release --locked --manifest-path "$CANDIDATE_SOURCE/Cargo.toml" \
  --bin esp32sim --bin esp32sim-c3 --bin esp32sim-c6
python3 docs/evidence/s3-coprocessor-reset/integrated-cpu.py \
  --base "$BASE_BINS" --candidate "$CANDIDATE_BINS" \
  --base-revision 49a277876d7f276d4b7d576e2bc167ffa9078482 \
  --candidate-revision 81d419f2959191fdff5a6d1fb062c7c4321dedb4 \
  --assets "$CANDIDATE_SOURCE/web/wasm/fw" --repo "$CANDIDATE_SOURCE" \
  --pairs 5 --workloads s3-hello,pocket-tank --output integrated-cpu.json
```

Run the same harness and pinned arguments with
`--workloads c3-hello,c6-hello,s3-hello --output integrated-cpu-controls.json`
for the second receipt. Its final S3 workload is the confirmation, not a
replacement for the initial result.

The caller supplies paths; no machine-specific path or remote target is embedded.
The assets are the checked-in manifests and firmware plus ROMs fetched with
`tools/fetch-demo-assets.sh --no-linux` and the model fetched with
`tools/fetch-pocket-tank.sh`. Every actual input's byte count and SHA-256 is in
the [S3 receipt](integrated-cpu.json) and
[controls/confirmation receipt](integrated-cpu-controls.json). Each command is
expanded there with normalized `$ASSETS` paths, including the Pocket Tank model
at flash offset `0x290000`. Native C3/C6 executables select their chips directly
and use their implicit zero-PSRAM configuration; the web manifest's chip-selector
board name and `psram_mb: 0` are not passed as native options.

Each workload runs for **30 guest seconds**, including Pocket Tank. One warmup
per arm precedes five measured pairs. Pair order alternates base/candidate and
candidate/base; every sample, including warmups, is retained. User CPU is the
`resource.RUSAGE_CHILDREN.ru_utime` difference around a single child process;
there are no concurrent child processes in the measuring harness. System CPU and
wall time are retained separately. The console hash covers unmodified stdout
from `--console uart0`. The stop report supplies per-core instructions, cycles,
emulated seconds, exceptions and interrupts.

## Results

Medians use five measured pairs per row; warmups are excluded. Ranges are the
minimum and maximum measured user CPU seconds. “Slower” counts pairs whose
candidate user time exceeds the paired baseline, without a significance test.

| Workload | Main median (s) | Integrated median (s) | Change | Main range (s) | Integrated range (s) | Slower |
| --- | ---: | ---: | ---: | --- | --- | ---: |
| S3 hello, initial | 0.260735 | 0.271223 | +4.022% | 0.258561–0.272733 | 0.264970–0.280415 | 4/5 |
| Pocket Tank | 65.009239 | 54.742631 | −15.793% | 58.475565–66.519512 | 46.410909–59.397372 | 0/5 |
| C3 hello | 4.554586 | 4.451180 | −2.270% | 4.436908–4.642239 | 4.326785–4.604727 | 0/5 |
| C6 hello | 2.199205 | 2.170459 | −1.307% | 2.169304–2.210057 | 2.129912–2.196920 | 1/5 |
| S3 hello, confirmation | 0.345191 | 0.350141 | +1.434% | 0.326997–0.348897 | 0.329824–0.359080 | 3/5 |

The S3 confirmation retains the same binaries, inputs, guest duration and flags.
Its main median is 32.4% higher than the initial main median; the observed
relative increase falls from 4.022% to 1.434%. Pocket Tank's warmup initially
shows the opposite direction from its measured pairs: main 49.609097 s versus
candidate 61.352480 s. Main's first measured run then takes 65.867192 s and the
candidate 54.742631 s. These reversals and drift prevent an isolated performance
claim; they are not discarded or used to select a favorable batch. Overlapping
ranges alone do not establish equivalence. The S3 increases remain visible as
unresolved observations on this host.

Every completed run stops at 30.000 guest seconds. Equality covers all warmups,
measured samples and both S3 batches:

| Workload | Reported instructions (core 0; core 1 where present) | Cycles | Console SHA-256 |
| --- | --- | ---: | --- |
| S3 hello | 11,990,305; 6,798,164 | 7,200,000,000 | `c74f198e19e94c47772b3403433b5532057180aa63df46ff90ffe9a51710966f` |
| Pocket Tank | 4,595,734,479; 6,171,560,740 | 7,200,000,000 | `5f9d2aecbb20769a9f076516c33955f82a9fa13fba79249edde134aed8986e0a` |
| C3 hello | 4,800,000,000 | 4,800,000,000 | `3b9d8d3eb053a97670f11896674e70ffc968fd7a803cb2cdc5c051414af94efd` |
| C6 hello | 4,800,000,000 | 4,800,000,000 | `bd69fbb9ac5c677337a293c1ec0ff7941306eec82c499290917a7c42360f86ad` |

Retained harness failures: the initial C3 command copied `--board esp32c3` from
the web manifest and exited 2 with `--board is not available on the C3`.
Removing that option exposed the separately unsupported `--psram-mb 0`, also
exit 2. Each rejection was reproduced once to isolate its diagnostic. Neither
rejection executed guest instructions or contributed a timing sample. The
corrected native mapping produced the C3/C6 samples above; the already completed
S3/Pocket Tank samples were retained. This was a harness error, not a guest
runtime failure.

## Conditions and limits

The host is an Apple M5 Max with 18 logical CPUs, macOS 27.0.1. Native release
builds finished before timing. Other validation was winding down at the start:
short offline model rebuilds and browser shutdown could overlap the initial S3
samples. Some concurrent source-checkout/file-copy I/O occurred during the first
measured Pocket Tank pair. The host was shared and was not otherwise isolated.
Load averages before and after every run are retained. One-minute load ranged
**8.81–38.75** in the first receipt and **14.80–20.20** in the controls receipt;
load average is not CPU utilization. No affinity, fixed clock,
thermal control or independent-machine replication was used. User CPU excludes
time descheduled but remains sensitive to core placement, frequency and cache
effects. Five pairs are a small sample, especially for the subsecond S3 hello.

Source inspection of `49a27787..81d419f2` finds cold S3 construction/reset writes,
new board construction alternatives, and board-specific sensor/callback code.
There is no change to the shared execution loop or scheduler. This narrows the
source change but does not exclude compiler/code-layout effects and does not
attribute a measured difference to any specific change. No emulator source was
modified for this comparison.

The JSON contains curated measurements and hashes only. Raw guest consoles,
process inventories, host/user identity and absolute temporary paths were not
retained. Console bytes were hashed before omission; this limits inspection of
the original text to reproduction, not the exact equality check. The original
EX248 correctness receipt and its historical artifact hashes remain unchanged.

Receipt validation recomputed every median and percentage, checked all 60
completed run signatures, and confirmed that binary hashes agree across both
receipts. All 506 candidate Rust/Cargo working-tree files still matched the
pinned revision after measurement. `git diff --check` and
`node tools/check-evidence-privacy.mjs` pass (2,213 tracked evidence files,
including 22 gzip files); the new text and JSON were also manually reviewed.

## Final Stamp integration follow-up

The final candidate `aca1effd` is measured against the same frozen baseline in
[the final comparison](integrated-cpu-final.md). It retains 48 additional fixed-work
runs: median user CPU S3 +1.541%, Pocket Tank +0.110%, C3 −20.940%, C6 +0.183%.
All signatures match this receipt. The lower load and large baseline drift
between periods are recorded, without replacing the original samples or
attributing the C3 decrease to a specific change.
