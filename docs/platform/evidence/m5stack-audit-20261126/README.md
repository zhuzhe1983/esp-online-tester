# Native bus regression comparison

This is the required whole-change regression check after the M5Stack product
adaptation touched bus/device dispatch. It reuses the exact-work acceptance
contract of EX114 and EX094. It introduces no execution optimization and does not
attribute a combined result to any individual device.

All **40 arms pass exact guest work and console checks**. The candidate is
`3bc45f158dc857a8aa968c2e7b795c1448831c62`, whose 970 workspace Rust/Cargo inputs
are identical to `d33f10c1`; baseline has 914 inputs. Both measured executable
sets use `cargo build --release --bins` with the same release profile and toolchain.
The complete compact receipt is [native-perf.json.gz](native-perf.json.gz), with
compressed/uncompressed hashes in [native-perf-artifact.json](native-perf-artifact.json).

| Workload | Main median user s | Current median user s | Change | Current slower pairs | Main range s | Current range s |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| S3 hello | 0.086541 | 0.135721 | +56.83% | 3/3 | 0.079292–0.127190 | 0.083149–0.142263 |
| C3 hello | 0.462633 | 0.441048 | −4.67% | 1/3 | 0.437493–0.516572 | 0.434394–0.464971 |
| C6 hello | 0.290343 | 0.322309 | +11.01% | 2/3 | 0.273274–0.401746 | 0.287438–0.426945 |
| Pocket Tank | 8.039659 | 9.743471 | +21.19% | 3/3 | 7.702491–9.229857 | 8.808258–9.750420 |
| P4 hello | 2.376364 | 2.609118 | +9.79% | 1/3 | 2.061741–2.691031 | 2.044404–2.624335 |

S3 hello and Pocket Tank are slower in every measured pair. These negative samples
are retained as measured regression evidence; this check provides **no zero-overhead
or no-regression clearance**. All per-arm ranges overlap. S3's sub-0.15-second
samples and the three-pair cohort especially limit precision. C3's lower median
is not a speedup claim, and the mixed C6/P4 pairs do not establish equivalence.
The coordinated window excludes other workspace builds/tests/rendering; the host
remains shared. Linux x86_64 exposes nine logical/affinity CPUs, Python is 3.12.14,
and one-minute aggregate load ranges from 2.027832 to 2.557617. No machine identity
or application/process inventory is retained.

The fixed main baseline is `81bc93c0333a9fd13c16565b245b3d1c0d7ce237`.
The baseline build uses an independent sparse checkout with all workspace crates.
Every local native crate is recompiled from the new source paths; the complete
normalized build log and its original/retained hashes are recorded separately.
The shared Cargo target is retained and builds are sequential. Both arms use:

```sh
PATH="$RUST_BIN:$PATH" CARGO_TARGET_DIR="$SHARED_TARGET" CARGO_BUILD_JOBS=2 \
  CARGO_INCREMENTAL=0 cargo build --release --bins
```

Rust is stable 1.100.0. The committed release profile is unchanged: opt-level 3,
debug 1, fat LTO, one codegen unit. The required S3/C3/C6 binaries plus a P4
control are preserved separately before the candidate build replaces shared-target
outputs. Debug sections are removed only from those copies using
`strip --strip-debug -o COPY ORIGINAL`. `baseline-binaries.json` retains both
file hashes and independently checks identical `.text` hashes. The original
shared-target executables are not modified by this preparation.

The `candidate-workspace-*` files retain an initial, unmeasured candidate build
using `--workspace`. Its wasm crate defaults enable additional features, so it
does not establish a matching build configuration. The final comparison uses
separately prepared `--bins` binaries and records the actual commands for both arms.

`run-native-perf.py` takes caller-supplied source, executable, ROM/firmware,
model and output paths. It compares the S3, C3 and C6 hello manifests and Pocket
Tank, with optional P4 hello coverage. Default duration is two guest seconds
per arm, including startup. Each workload has one excluded warm-up pair and
three measured pairs in alternating AB/BA order. Median child **user CPU** is
the performance metric; emulator wall-time diagnostics are not compared.
The short window does not represent steady-state Pocket Tank inference.

Each arm must exit successfully with exactly one normal stop report at the
requested guest duration (native quantum 64, deadline overrun 0..63 cycles),
nonzero guest work, expected firmware boot text and no fatal diagnostic. Per-core
instructions, cycles, exception and interrupt totals, and the original console
SHA-256 must match across every arm, including warm-up. A mismatch stops the
comparison and leaves the rejected row in the report.

The model is absent from the initial local asset set. It is fetched to an ignored
scratch path from the official script's fixed source:

- URL: <https://raw.githubusercontent.com/mediacutlet/pocket-tank/5cc33b1adf4076315b805e94129b295570a45b39/model/out/model_q4.bin>
- SHA-256: `b200bf87c78dc85c4d727f68941ac0b83cced9827037cc169cd2122d71002c2a`
- Pin source: `tools/fetch-pocket-tank.sh` at the baseline revision.

No model or firmware copy is added to Git. The harness records SHA-256 and size
for all actual inputs, manifests and executables and checks they remain unchanged.
It also records a SHA-256 manifest fingerprint for workspace Rust/Cargo source
files and verifies they match each supplied source revision before execution.

Reproduction, after defining the role paths and the candidate source revision:

```sh
python3 "$EVIDENCE/run-native-perf.py" \
  --baseline-source "$BASELINE_SOURCE" --candidate-source "$CANDIDATE_SOURCE" \
  --baseline-bin "$BASELINE_BIN" --candidate-bin "$CANDIDATE_BIN" \
  --baseline-revision 81bc93c0333a9fd13c16565b245b3d1c0d7ce237 \
  --candidate-revision "$CANDIDATE_REVISION" \
  --firmware-dir "$FIRMWARE" --model-file "$MODEL" \
  --output "$EVIDENCE/native-perf" --raw-output "$IGNORED_OUTPUT" \
  --seconds 2 --pairs 3 --include-p4
```

Measurement requires a window with workspace builds, tests and model rendering
stopped. The host is shared, so unrelated activity remains uncontrolled. Aggregate
load is retained; no process inventory or host/user identity is collected. All
capture paths are normalized before storage; original and retained stream hashes
preserve provenance. Full captures stay in the ignored output directory. Exact
console/work agreement does not establish frame/audio equality, browser speed or
hardware timing. Small and overlapping CPU samples do not establish equivalence.
