# Native divide: maintainer CPU comparison

The maintainer ran this comparison and posted it in [PR #321](https://github.com/mikroverk/esp32sim/pull/321#issuecomment-6540590182). This is his run, not a new measurement. New: `ddbe74e7ebfd63b57ade54228c8cb86630bdb418`; base: `66eb9704` (the PR base). The script was `docs/evidence/p4-cpu/cpu_pairs.py` from that base, with a local `classic-hello` entry added using the `hello_world_classic` golden's command and 30 emulated seconds. T-Display was not part of this CPU run; its earlier host-instruction screening is recorded in EX261.

Both trees were extracted by `git archive` to sibling directories with equal-length paths, and built using `cargo build --release --bins` with rustc 1.99.1 (58cf8f9c3 2026-10-13), aarch64-apple-darwin, on Apple M5 Max (18 cores), macOS 27.0.1. ROMs: esp-rom-elfs 20260528 (`tools/fetch-rom-elfs.sh`). Each workload started at 1-minute load below 5 with nothing building. From the base tree root, one warmup pair preceded seven alternating pairs per workload; timings are user CPU seconds.

| workload | base median (range) | new median (range) | change | new slower | instructions, console | load before → after |
| --- | --- | --- | --- | --- | --- | --- |
| s3-hello (30 s) | 0.260 s (0.260–0.270) | 0.260 s (0.260–0.260) | +0.0% | 0/7 | identical in all 16 runs | 3.95 → 3.71 |
| c3-hello (30 s) | 2.490 s (2.470–2.660) | 2.490 s (2.450–2.650) | +0.0% | 3/7 | identical in all 16 runs | 3.71 → 3.82 |
| c6-hello (30 s) | 1.610 s (1.600–1.650) | 1.620 s (1.590–1.670) | +0.6% | 4/7 | identical in all 16 runs | 3.82 → 3.68 |
| pocket-tank (8 s) | 7.160 s (7.150–7.500) | 7.180 s (7.130–7.410) | +0.3% | 3/7 | identical in all 16 runs | 3.68 → 3.52 |
| classic-hello (30 s) | 0.380 s (0.370–0.380) | 0.370 s (0.370–0.380) | -2.6% | 2/7 | identical in all 16 runs | 3.52 → 3.48 |

The maintainer's verdict: nothing is flagged (slower in at least 6/7 pairs). The hello demos and Pocket Tank barely divide; this table checks for other movement and does not measure the divide gain itself. The earlier T-Display host-instruction counts are screening evidence, not this CPU timing table.

[Posted JSON](321-ddbe74e7-66eb9704.json) preserves the runs, commands, loads and summaries as posted. The maintainer replaced the ROM directory with `$ESP32SIM_ROM_DIR`; he reported original SHA-256 prefix `f5a23fcb…` and sanitized prefix `822ca944…`. The JSON extracted from the comment without a trailing newline has SHA-256 `822ca944227e9fd143a482765032d372c78fd25200df3f920088984bb5ec96c4`, matching his sanitized prefix; the original hash is known only by the prefix he reported. The original path is omitted; this does not alter the measurements. The locally modified script was described in the comment but not supplied.
