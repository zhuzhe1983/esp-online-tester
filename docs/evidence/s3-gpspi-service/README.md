# S3 GP-SPI service: maintainer CPU comparison

The following is **joakimeriksson's run**, posted on [PR #312](https://github.com/mikroverk/esp32sim/pull/312#issuecomment-6540589822), not a measurement by the author of the change. Base `66eb9704`, new `fd5751f5`; `cpu_pairs.py` from `docs/evidence/p4-cpu` at the base. Both trees were extracted with `git archive` into equal-length sibling paths and built with `cargo build --release --bins`, rustc 1.99.1 (58cf8f9c3 2026-10-13), aarch64-apple-darwin. Apple M5 Max (18 cores), macOS 27.0.1; each workload started with 1-minute load below 5 and nothing building. ROMs: esp-rom-elfs 20260528 (`tools/fetch-rom-elfs.sh`). One warmup pair, then 7 alternating pairs per workload, from the base tree root. Times are user CPU.

| workload | base median (range) | new median (range) | change | new slower | instructions, console | load before → after |
| --- | --- | --- | --- | --- | --- | --- |
| s3-hello (30 s) | 0.260 s (0.250–0.260) | 0.260 s (0.250–0.260) | +0.0% | 3/7 | identical in all 16 runs | 3.62 → 3.73 |
| c3-hello (30 s) | 2.480 s (2.460–2.520) | 2.500 s (2.480–2.760) | +0.8% | 4/7 | identical in all 16 runs | 3.73 → 3.38 |
| c6-hello (30 s) | 1.610 s (1.590–1.690) | 1.620 s (1.600–1.690) | +0.6% | 4/7 | identical in all 16 runs | 3.38 → 3.50 |
| pocket-tank (8 s) | 7.150 s (7.130–7.310) | 7.200 s (7.100–7.230) | +0.7% | 4/7 | identical in all 16 runs | 3.50 → 3.52 |

The maintainer's verdict: nothing flagged (slower in at least 6/7 pairs). Pocket Tank, the workload driving the QSPI panel over SPI2, is +0.7% with 4/7 slower and overlapping ranges: noise at 10 ms resolution.

[Posted JSON](312-fd5751f5-66eb9704.json) preserves the data and the `$ESP32SIM_ROM_DIR` substitution as posted. The maintainer reports raw SHA-256 prefix `53769732…` and sanitized prefix `389eba3f…`; the JSON as embedded in the comment and extracted here has SHA-256 `6a900addb3543056dd6625f03913b9195818ea129164d84b03d0bb30d64ccb29`. The original raw file was not supplied; formatting in the posted code block may differ from the hashed file. The ROM path redaction changes only the local directory, not measured values or workload selection.
