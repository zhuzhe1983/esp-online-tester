# EX251: CoreMatrix integration on existing native chips

The C158 adaptation adds a separate ESP32-C61 runner and cold board/WASM hooks. It does not change the S3 execution core or its timing model. This is a new-chip integration regression check, related to the cold input routes in [EX217](../../experiments.md#ex217) and the separate chip sleep implementations in [EX233](../../experiments.md#ex233), not a repeat of their optimization experiments.

Baseline `587a3b3dfab6b55aff2524d4a52066c4febfaef1` (main, including the StamPLC correction) and candidate `4573f5d36eca3db26f89ecbbe231e3fb68217605` were built with Rust stable 1.99.1 using the same locked release command. The [receipt](results.json) contains source fingerprints, binary/input hashes, all samples, order and environment. Candidate changes after this revision are documentation only.

All 48 runs, including six warmups, preserve exact instruction counts, cycles, simulated time, exceptions, interrupts and unmodified UART byte hashes within each workload. Seven alternating measured pairs use 30 guest seconds per run; one run per arm per workload is retained as warmup but excluded from medians. No guest timing or golden output was changed.

| Workload | Baseline median user CPU (s) | Candidate median user CPU (s) | Change | Candidate slower pairs |
| --- | ---: | ---: | ---: | ---: |
| s3-hello | 0.255856 | 0.257367 | +0.591% | 4/7 |
| c3-hello | 2.607971 | 2.602548 | -0.208% | 2/7 |
| c6-hello | 1.762883 | 1.767992 | +0.290% | 3/7 |

Sample ranges overlap for every workload and paired directions are mixed. This shared host had one-minute load averages 6.180–7.337 during the run. These samples do not establish zero overhead, a speedup or statistical equivalence. No other build or benchmark from this adaptation ran concurrently. The S3 CPU/JIT implementation has no diff against the baseline, so Pocket Tank was not required by the S3-core check. C61 throughput and physical timing are not inferred from these older-chip measurements.

## Reproduce

Build the two revisions in separate worktrees and target directories:

```sh
CARGO_BUILD_JOBS=2 CARGO_TARGET_DIR="$ARM_TARGET" cargo +stable build --release --locked --manifest-path "$ARM_SOURCE/Cargo.toml" --bin esp32sim --bin esp32sim-c3 --bin esp32sim-c6
python3 docs/evidence/corematrix-cpu/compare.py   --base "$BASE_TARGET/release" --candidate "$CANDIDATE_TARGET/release"   --base-revision 587a3b3dfab6b55aff2524d4a52066c4febfaef1   --candidate-revision 4573f5d36eca3db26f89ecbbe231e3fb68217605   --assets web/wasm/fw --repo . --pairs 7 --output "$RESULT"
```

Supply the pinned firmware and ROM files listed in the receipt. The harness rejects any work/output mismatch. It retains aggregate load, tool/OS versions and public hardware class, with no local path, hostname, login, process inventory or raw console. UART bytes are hashed before omission. [Product behavior and known limits](../../platform/corematrix-runtime.md) remain separate from this CPU comparison.
