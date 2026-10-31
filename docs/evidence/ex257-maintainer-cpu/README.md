# EX257 maintainer CPU comparison

The maintainer ran this comparison and posted it in [PR #316](https://github.com/mikroverk/esp32sim/pull/316#issuecomment-6540590011). These are his measurements, not a rerun.

`cpu_pairs.py` from `docs/evidence/p4-cpu` as on `66eb9704` with one workload added in a local copy: `classic-hello`, the `hello_world_classic` golden's command (`--chip esp32 --board esp32dev --boot rom --console uart0`, classic-hello bootloader/ptable/app) for 30 emulated seconds, new = `716a1a35`, base = `66eb9704` (the PR's base). Both trees were extracted with `git archive` into sibling directories with paths of the same length and built with `cargo build --release --bins`, rustc 1.99.1 (58cf8f9c3 2026-10-13), aarch64-apple-darwin. Machine: Apple M5 Max (18 cores), macOS 27.0.1. Each workload started only with the 1-minute load below 5, nothing building. ROMs: esp-rom-elfs 20260528 (`tools/fetch-rom-elfs.sh`). One warmup pair, then 7 alternating pairs per workload, from the base tree's root.

| workload | base median (range) | new median (range) | change | new slower | instructions, console | load before → after |
| --- | --- | --- | --- | --- | --- | --- |
| s3-hello (30 s) | 0.260 s (0.260–0.270) | 0.240 s (0.240–0.250) | -7.7% | 0/7 | identical in all 16 runs | 3.52 → 3.72 |
| c3-hello (30 s) | 2.520 s (2.440–2.560) | 2.510 s (2.480–2.540) | -0.4% | 3/7 | identical in all 16 runs | 3.72 → 3.50 |
| c6-hello (30 s) | 1.590 s (1.580–1.670) | 1.620 s (1.590–1.700) | +1.9% | 5/7 | identical in all 16 runs | 3.50 → 3.66 |
| pocket-tank (8 s) | 7.220 s (7.160–7.260) | 7.180 s (7.120–7.270) | -0.6% | 3/7 | identical in all 16 runs | 3.66 → 4.12 |
| classic-hello (30 s) | 0.380 s (0.370–0.390) | 0.370 s (0.370–0.380) | -2.6% | 2/7 | identical in all 16 runs | 4.12 → 3.95 |

The maintainer reports that nothing is flagged as slower (the flag threshold is slower in at least 6 of 7 pairs). S3 hello is 7.7% faster, with non-overlapping ranges and 0/7 slower; classic hello is 2.6% faster, 2/7 slower. T-Display is not included in this CPU table or JSON; the earlier untimed equal-work check included it, but that is not a CPU comparison.

The [posted JSON](316-716a1a35-66eb9704.json) is preserved as posted. The maintainer replaced only the ROM directory with `$ESP32SIM_ROM_DIR`; he reported raw SHA-256 prefix `385b9c7b…` and sanitized SHA-256 prefix `a73b9c71…`. No full raw hash or private original was supplied. This path redaction does not change measured values. The local script copy differs from `cpu_pairs.py` only by the classic-hello entry.
