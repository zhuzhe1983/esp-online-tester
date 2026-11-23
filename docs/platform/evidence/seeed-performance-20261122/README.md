# Seeed native performance checkpoint, 2026-11-22

**This is an incomplete checkpoint, not final performance acceptance.** Three
hello workloads each completed three alternating main/candidate pairs. Pocket
Tank completed two pairs before the device scope expanded. Its third pair and
the planned longer S3 hello check were not started. All 22 successful runs and
the setup/pause failures are retained in
[`checkpoint-84e13293.json`](checkpoint-84e13293.json).

| Workload | Guest window | Main user seconds, by pair | Candidate user seconds, by pair | Three-pair medians | Candidate change |
| --- | ---: | --- | --- | --- | ---: |
| S3 hello | 30 s | 0.56, 0.59, 0.69 | 0.70, 0.67, 0.71 | 0.59 → 0.70 s | +18.64% |
| C3 hello | 30 s | 6.31, 5.82, 5.48 | 5.65, 5.57, 5.61 | 5.82 → 5.61 s | −3.61% |
| C6 hello | 30 s | 3.53, 3.54, 3.50 | 3.29, 3.59, 4.10 | 3.53 → 3.59 s | +1.70% |
| Pocket Tank | 30 s | 193.96, 191.73 | 194.35, 192.72 | Incomplete: 2 of 3 pairs | Not evaluated |

For each workload, every recorded run has the same complete stdout byte count
and SHA-256, per-core instruction counts, cycle count, exception count and
interrupt count. The hello consoles reach `app_main`, print the greeting and
restart countdown. Pocket Tank reports token rates. Total retired instructions
are 18,710,559 for S3 hello, 4,800,000,000 for each RISC-V hello and
10,767,295,219 for Pocket Tank. The receipt gives the per-core counts and full
input/console digests. Raw output, which may contain generated device values,
is retained locally rather than committed.

The S3 short-window increase is unresolved. The sample takes less than one host
CPU second, but that alone does not establish that its increase is noise.
A separate 300-guest-second comparison was planned to test stability with more
work per sample. It must be reported alongside the 30-second result, not used
to replace it. No broad claim of unchanged performance is supported yet.

## Build and measurement identity

The baseline is original main `573f5a6c1bcc6dbf0388383d178a3935ec8ddcce`, built
with `CARGO_BUILD_JOBS=1 cargo build --release --bins`. Its common CLI SHA-256 is
`22954f6f7ef68a1d12e76a07ed3494c25e9cc2c231781ea912d479af8f3a87be`.
The candidate CLI SHA-256 is
`84e13293e0524d108f4c08817fe09b0352fc525c8e134cecb29a81c7b80ef7b1`.
The recorded candidate Git HEAD is an **unpublished integration checkpoint**,
not the final published commit. Executable and firmware/ROM digests identify
the exact measurement. No release-profile, LTO or optimization setting changed.

Task-owned builds, tests and other emulator probes were stopped before timing.
CPU affinity and frequency were not fixed. The environment used an AMD EPYC
9V74, Rust 1.100.0 and Bash 5.2.37. This container has no `/usr/bin/time`, so
both arms used Bash's POSIX `time -p`; reported times have 0.01-second precision.

Two initial setup failures are preserved. The absent external timer prevented
any emulator from starting. A later C3 invocation was rejected before guest
execution because the common argument builder supplied `--psram-mb 0`; that
option was removed from C3/C6, as in `CONTRIBUTING.md`. The six valid S3 samples
already completed were retained. The scope-expansion pause completed Pocket
pair 2, then stopped the driver before pair 3. No timed guest was interrupted.

## Reproduction

[`compare.py`](compare.py) consumes separately built baseline/candidate common
CLIs, the committed public firmware fixtures and the ROM directory obtained
with `tools/fetch-rom-elfs.sh`. Obtain Pocket Tank's pinned model with
`tools/fetch-pocket-tank.sh` before opening a quiet window. `--run` is explicit;
without it, the script only verifies inputs and writes a prepared manifest.
Use a fresh output directory so previous samples and failures are not replaced.

```sh
python3 docs/platform/evidence/seeed-performance-20261122/compare.py \
  --base "$BASE/target/release/esp32sim" \
  --candidate "$CANDIDATE/target/release/esp32sim" \
  --root "$CANDIDATE" --rom-dir "$ROM_DIR" \
  --output target/seeed-performance-30s --pairs 3 --seconds 30 \
  --pause-file target/seeed-performance.pause --run
```

Defaults pin the two executable hashes above. A newly built final candidate
requires an explicitly recorded `--candidate-sha256`; it cannot silently reuse
this checkpoint's validation. Creating the pause file lets an active pair
finish before stopping the next one. Remove it before starting a new run.

The proposed stability check changes only the S3 hello guest window and is
**not yet measured**:

```sh
python3 docs/platform/evidence/seeed-performance-20261122/compare.py \
  --base "$BASE/target/release/esp32sim" \
  --candidate "$CANDIDATE/target/release/esp32sim" \
  --root "$CANDIDATE" --rom-dir "$ROM_DIR" \
  --output target/seeed-performance-s3-300s \
  --workloads hello --pairs 3 --seconds 300 --run
```
