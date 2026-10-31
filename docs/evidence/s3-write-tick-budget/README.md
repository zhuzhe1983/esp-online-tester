# EX254: Maintainer CPU comparison

[Maintainer run on PR #299](https://github.com/mikroverk/esp32sim/pull/299#issuecomment-6540589661), October 31, 2026. The following setup and table are from the maintainer, not a separate local run.

The maintainer used `cpu_pairs.py` from `docs/evidence/p4-cpu` as on `66eb9704`, new = `a7f97fb6`, base = `66eb9704` (the PR's base). Both trees were extracted with `git archive` into sibling directories with paths of the same length and built with `cargo build --release --bins`, rustc 1.99.1 (58cf8f9c3 2026-10-13), aarch64-apple-darwin. Machine: Apple M5 Max (18 cores), macOS 27.0.1. Each workload started only with the 1-minute load below 5, nothing building. ROMs: esp-rom-elfs 20260528 (`tools/fetch-rom-elfs.sh`). One warmup pair, then 7 alternating pairs per workload, from the base tree's root.

| workload | base median (range) | new median (range) | change | new slower | instructions, console | load before → after |
| --- | --- | --- | --- | --- | --- | --- |
| s3-hello (30 s) | 0.260 s (0.250–0.260) | 0.260 s (0.250–0.270) | +0.0% | 1/7 | identical in all 16 runs | 4.94 → 5.02 |
| c3-hello (30 s) | 2.500 s (2.460–2.820) | 2.510 s (2.460–2.820) | +0.4% | 4/7 | identical in all 16 runs | 5.02 → 4.13 |
| c6-hello (30 s) | 1.620 s (1.590–1.710) | 1.600 s (1.570–1.650) | -1.2% | 4/7 | identical in all 16 runs | 4.13 → 3.89 |
| pocket-tank (8 s) | 7.170 s (7.130–7.230) | 7.170 s (7.150–7.210) | +0.0% | 2/7 | identical in all 16 runs | 3.89 → 3.62 |

Nothing is flagged by the maintainer: new slower in at least 6 of 7 pairs, or non-overlapping ranges. Instructions and console are identical everywhere. The c3-hello starting load is 5.02 as posted, despite the stated below-5 starting condition.

[Posted JSON](299-a7f97fb6-66eb9704.json). The maintainer replaced the ROM directory with `$ESP32SIM_ROM_DIR`; posted raw SHA-256 prefix `79b5f22d…`, sanitized prefix `2c1aae65…`. The JSON text extracted from the comment and retained here has SHA-256 `7023700063d8fdfd20385438dd4d4cfd6538dfd2df6711a5881ebb577e0cfab9`; it does not match the posted sanitized prefix, so the original sanitized file bytes cannot be verified from the comment.
