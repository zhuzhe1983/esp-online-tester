# Shared-bus native regression check

[The complete receipt](native-regression.json) compares commit `86cf258dff067c6eb6327474c774a189541eb43f` with the recorded PaperMono SD/RGB working-tree source hashes. Both active native binaries were matched to their Cargo fingerprints: identical Rust compiler, profile and empty Rust flags. Both builds used `cargo build --release --bins`; WASM compiler flags were not inherited by the measured native builds.

Each workload runs for 30 emulated seconds, with three alternating baseline/candidate pairs: B,C; C,B; B,C. All **24 runs** have identical instruction counts, cycle counts and stdout hashes within each workload. No golden files or user firmware were changed or used.

| Workload | Baseline median user CPU | Candidate median user CPU | Change |
| --- | ---: | ---: | ---: |
| S3 hello | 0.25 s | 0.25 s | 0.00% |
| C3 hello | 2.36 s | 2.32 s | -1.69% |
| C6 hello | 1.54 s | 1.57 s | +1.95% |
| Pocket Tank | 38.65 s | 38.56 s | -0.23% |

Pocket Tank's three paired differences are +1.79%, -0.23% and -1.28%; their mixed directions support a near-flat result in this screen, not a speedup or formal equivalence. The hello differences are small absolute values with the `time -p` 0.01-second resolution. One-minute aggregate host load ranged from about 2.65 to 3.80 during capture. There were no concurrent task-owned builds or probes during timing, but exclusive host access was not established.

To reproduce, build the baseline and candidate separately with the default release configuration, then run each binary through `/usr/bin/time -p` using the exact relative arguments and input hashes in `native-regression.json`. Use the listed six-run order per workload. The Pocket Tank additional flash input is the existing public model fixture, identified by SHA-256; its local temporary filename is normalized in the receipt. The executed local orchestration script is `output/benchmark-papermono-sd-core.py`; the JSON includes all commands, input/source/binary hashes, raw numeric samples and comparison calculations needed to reproduce without its workspace paths.

Only aggregate load and relevant build/work values were retained. Local temporary paths, process identifiers and machine identifiers are omitted. Candidate source hashes remained unchanged after the run. The separate [WASM JIT test isolation check](../../../evidence/wasm-jit-test-isolation-2026-10-27/README.md) repairs a CI test-process heap failure and makes no production execution change.
