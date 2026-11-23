# Final Seeed native CPU comparison, 2026-11-22

**All 30 planned runs completed with identical guest work and output within each
case.** The measured candidate median changes are S3 hello 30 s **0.00%**, C3
hello **+0.91%**, C6 hello **−2.13%**, Pocket Tank **−0.86%**, and the independent
S3 hello 300 s check **−0.18%**. Each case has only three alternating pairs, and
the measured ranges overlap in every case. These observations do not establish
zero overhead or a stable speed improvement.

[receipts.json](receipts.json) preserves every sample, all executable/input
identities, normalized CLI arguments, console/log hashes, exact guest counters
and source-IRQ reports. [samples.csv](samples.csv) gives all 30 measurements,
including real, user and system seconds. No sample was discarded, interrupted,
replaced or selected from additional runs. No timed run failed in this final
batch.

## All results

The comparison metric is the ratio of medians,
`100 × (candidate_median / main_median − 1)`, using POSIX `time -p` **user CPU
seconds**. A positive number means more user CPU time. Main/candidate order is
strictly alternating within each case.

| Workload | Guest window | Main user seconds, pairs 1–3 | Candidate user seconds, pairs 1–3 | Main median | Candidate median | Change |
| --- | ---: | --- | --- | ---: | ---: | ---: |
| S3 hello | 30 s | 0.60, 0.54, 0.63 | 0.69, 0.56, 0.60 | 0.60 | 0.60 | 0.00% |
| C3 hello | 30 s | 5.50, 5.51, 5.61 | 5.56, 5.60, 5.46 | 5.51 | 5.56 | +0.91% |
| C6 hello | 30 s | 3.29, 3.28, 3.31 | 3.22, 3.22, 3.39 | 3.29 | 3.22 | −2.13% |
| Pocket Tank | 30 s | 205.67, 194.14, 188.91 | 198.55, 192.48, 180.58 | 194.14 | 192.48 | −0.86% |
| S3 hello stability check | 300 s | 5.44, 5.70, 5.54 | 5.44, 5.53, 5.72 | 5.54 | 5.53 | −0.18% |

| Workload | Main range, user seconds | Candidate range, user seconds | Candidate slower pairs | Equal pairs |
| --- | --- | --- | ---: | ---: |
| S3 hello, 30 s | 0.54–0.63 | 0.56–0.69 | 2 / 3 | 0 / 3 |
| C3 hello, 30 s | 5.50–5.61 | 5.46–5.60 | 2 / 3 | 0 / 3 |
| C6 hello, 30 s | 3.28–3.31 | 3.22–3.39 | 1 / 3 | 0 / 3 |
| Pocket Tank, 30 s | 188.91–205.67 | 180.58–198.55 | 0 / 3 | 0 / 3 |
| S3 hello, 300 s | 5.44–5.70 | 5.44–5.72 | 1 / 3 | 1 / 3 |

The C3 positive median change remains part of the result; it is not dismissed as
proven noise. Pocket Tank is faster in all three candidate pairs, but times
decrease across rounds in **both** arms and the ranges overlap substantially.
Fixed ordering, lack of a separate warmup, and uncontrolled host conditions
prevent attributing that decrease to a particular code change. The first pair
is retained along with every other pair.

The 30-second S3 samples take less than one host CPU second. The 300-second
guest workload was measured separately to check the earlier short-window risk
with more work per sample. Its median difference is −0.01 user seconds, with
nearly identical ranges; it does not replace the short-window measurements.

## Exact-work and output contract

For each workload/window, all six runs have identical complete stdout bytes
(length and SHA-256), stop reason, per-core instruction counts, total instruction
count, exact guest seconds/cycles, exception count and total IRQ count. After
the timing finished, every per-source IRQ report line was also compared across
the six samples and matched exactly. No raw console body is published because
it can contain generated device values; its complete hash is retained.

| Case | Per-core instructions | Total instructions | Guest cycles | Exceptions | Interrupts |
| --- | --- | ---: | ---: | ---: | ---: |
| S3 hello, 30 s | 11,934,783; 6,775,776 | 18,710,559 | 7,200,000,000 | 27,229 | 6,032 |
| C3 hello, 30 s | 4,800,000,000 | 4,800,000,000 | 4,800,000,000 | 0 | 3,028 |
| C6 hello, 30 s | 4,800,000,000 | 4,800,000,000 | 4,800,000,000 | 0 | 3,029 |
| Pocket Tank, 30 s | 4,595,734,479; 6,171,560,740 | 10,767,295,219 | 7,200,000,000 | 1,048,080 | 100,430 |
| S3 hello, 300 s | 114,416,277; 65,857,999 | 180,274,276 | 72,000,000,000 | 271,280 | 60,326 |

The hello runs use board `none`. Pocket Tank uses the committed
`waveshare-amoled18-v2` fixture and the pinned model at flash offset `0x290000`.
All three chips are selected through the same common CLI's `--chip` option.
This measures native release regression behavior of these fixtures, not the
browser/WASM speed or full performance of every newly added product.

The intentionally changed `panel_sid` output is a separate correctness review:
[four revised goldens and complete waveform/frame comparison](../seeed-panel-golden-20261122/README.md).
The exact equality asserted here applies to the five benchmark cases above; it
does not erase that deliberate LCD timing change.

## Artifact and profile identity

| Arm | Executable bytes | SHA-256 |
| --- | ---: | --- |
| Original main | 34,395,672 | `22954f6f7ef68a1d12e76a07ed3494c25e9cc2c231781ea912d479af8f3a87be` |
| Final candidate | 35,153,728 | `4dd4d549dabb2e11282e692119558bb3a4907baa02be345aac9c5ff71176c9a4` |

The baseline source is
`573f5a6c1bcc6dbf0388383d178a3935ec8ddcce`. Its recorded build command is
`CARGO_BUILD_JOBS=1 cargo build --release --bins`; the candidate was built with
`cargo build --release -p esp32sim`. No rebuild or profile adjustment occurred
during the measurement window. Both root release profiles are byte-identical:

```toml
[profile.release]
opt-level = 3
debug = 1
lto = "fat"
codegen-units = 1
```

The profile-text SHA-256 is
`80631c1ff99076b232a92ce98b0496ea49eb968d82bbe9c328281cd7567179ea`.
The candidate Git HEAD recorded by both batches is
`3665649677d59c5e2f797b352d279e375e8a38e7`, an **unpublished integration checkpoint**.
The executable and firmware/ROM hashes identify the measured artifact regardless
of the final published commit name. Remote revision `43de45a` differs from the
original baseline only in three catalog JSON files; it contains no Rust or
Cargo change and does not require a different native baseline.

Each executable was rehashed before and after each of its runs. All input files
were checked again after each batch, and both executables plus every input were
checked once more after run 30. All hashes matched. The receipt includes every
ROM, bootloader, partition-table, application and model digest. In particular,
Pocket Tank's model is 7,557,640 bytes with SHA-256
`b200bf87c78dc85c4d727f68941ac0b83cced9827037cc169cd2122d71002c2a`.

## Conditions and limitations

The standard batch ran from 11:54:20 to 12:15:16 UTC on 2026-11-22. The separate
long S3 batch ran from 12:15:31 to 12:16:06 UTC. Builds, tests, other emulator
probes, compression, packaging and large unrelated hashes were stopped during
the declared quiet window. Concurrent task activity was limited to small
document/JSON operations. No process inventory was captured or published.

The environment was Linux 6.18.57, x86-64, AMD EPYC 9V74, with nine CPUs in the
available affinity set; Rust 1.100.0, Python 3.12.14 and Bash 5.2.37. There is no
`/usr/bin/time`, so both arms used Bash's builtin POSIX `time -p` through the
same positional-argument wrapper. Times are reported to 0.01 seconds. There was
no CPU pinning, frequency/thermal control, randomized ordering or hidden warmup.
A quiet task window does not eliminate host scheduling or neighboring activity.

Only three pairs were planned and run for each case. No confidence interval or
hardware/browser speed conclusion is claimed. No extra run was added to seek a
more favorable number. The reported ranges and full sample order should be used
when assessing the small median differences.

## Earlier incomplete result remains separate

The earlier candidate
`84e13293e0524d108f4c08817fe09b0352fc525c8e134cecb29a81c7b80ef7b1`
had an incomplete 22-run checkpoint, including a **+18.64%** S3 short-window
increase and only two Pocket Tank pairs. Its timer/C3 setup failures and scope
pause remain preserved in
[the original checkpoint](../seeed-performance-20261122/README.md).

That short increase did not recur in this final artifact's medians, but the
candidate binary and measurement window changed. The new data do not establish
that the earlier increase was noise, and do not turn the earlier incomplete
checkpoint into a completed acceptance. The earlier numbers and failures are
not overwritten.

This is the product-integration CPU follow-up to the existing
[EX212 RGB/VSYNC record](../../../experiments.md#ex212), not a new performance
optimization experiment.

## Reproduction

The unchanged [comparison harness](../seeed-performance-20261122/compare.py)
has SHA-256
`706854adfaec8cb58c48f4b39ce9cc81a919ea200a54b7b3a3a174b81566f83b`.
Obtain the ROMs and pinned Pocket Tank model as described by the existing
`tools/fetch-rom-elfs.sh` and `tools/fetch-pocket-tank.sh` workflows. Supply
separately built executables and use fresh output directories. The explicit
candidate hash is necessary because the harness default still identifies the
historical checkpoint.

```sh
python3 docs/platform/evidence/seeed-performance-20261122/compare.py \
  --base "$BASELINE_BIN" --candidate "$CANDIDATE_BIN" \
  --root "$CANDIDATE_ROOT" --rom-dir "$ROM_DIR" \
  --baseline-sha256 22954f6f7ef68a1d12e76a07ed3494c25e9cc2c231781ea912d479af8f3a87be \
  --candidate-sha256 4dd4d549dabb2e11282e692119558bb3a4907baa02be345aac9c5ff71176c9a4 \
  --pairs 3 --seconds 30 --pause-file "$PAUSE_FILE" \
  --output "$OUTPUT_ROOT/standard-30s" --run

python3 docs/platform/evidence/seeed-performance-20261122/compare.py \
  --base "$BASELINE_BIN" --candidate "$CANDIDATE_BIN" \
  --root "$CANDIDATE_ROOT" --rom-dir "$ROM_DIR" \
  --baseline-sha256 22954f6f7ef68a1d12e76a07ed3494c25e9cc2c231781ea912d479af8f3a87be \
  --candidate-sha256 4dd4d549dabb2e11282e692119558bb3a4907baa02be345aac9c5ff71176c9a4 \
  --pairs 3 --seconds 300 --workloads hello --pause-file "$PAUSE_FILE" \
  --output "$OUTPUT_ROOT/s3-stability-300s" --run
```

`PAUSE_FILE` must not exist when starting. The harness retains a completed pair
before honoring a requested pause; no pause was used in these final batches.
Without `--run`, it only prepares and validates the inputs. Raw measurement
files remain private; their hashes, every measured value and all exactness
checks are preserved in this curated evidence.
