# EX247, step 1: saved PC, FP traps and the SD card on rev ≥ 3 (`p4/rev3-step1`)

Branch commit `f7deb4cb` (ASSIST_DEBUG's saved PC per core, `Cpu::ext_ill_all_fp`, the rev ≥ 3
register audit, merged with `p4/rev3` at `d0d8260b`) against `p4/rev3` at `d0d8260b`, the
branch it is stacked on, as [README.md](README.md) describes. Results:
[08-rev3-step1.json](08-rev3-step1.json).

The other chips' paths gain one `bool` in the shared `Cpu` (`ext_ill_all_fp`, read only when an
F instruction or an FP CSR traps with FS = Off, which no other chip's core has) and its copy in
`Cpu::reset`; the saved-PC recording is the P4's own. Every run of a workload retired the same
instructions and printed the same console on both builds.

## run 1, started 2026-10-26T09:09:48+0100

Machine: an Apple M5 Max (18 cores) on macOS 27.0.1, not otherwise idle (a test suite had just
finished: the 1-minute load starts at 9.4); rustc 1.99.1 (58cf8f9c3 2026-10-13). Both trees
extracted with `git archive` into sibling directories of the same path length and built with
`cargo build --release --bins`. Unlike the earlier receipts, the script ran from a third checkout
of the branch, not from the base tree, because the base tree from `git archive` has no fetched
firmware; both binaries read the same `web/wasm/fw` files.

| workload | `p4/rev3` median (range) | branch median (range) | change | branch slower | instructions | console SHA-256 | 1-min load before / after |
| --- | ---: | ---: | ---: | ---: | ---: | --- | --- |
| s3-hello | 0.27 (0.27-0.28) | 0.27 (0.26-0.27) | +0.00% | 0/7 | 11,990,305 + 6,798,164 | `04d4912461cb…` | 9.4 / 9.1 |
| c3-hello | 2.47 (2.44-2.50) | 2.43 (2.41-2.51) | -1.62% | 1/7 | 4,800,000,000 | `3b9d8d3eb053…` | 9.1 / 6.6 |
| c6-hello | 1.63 (1.62-1.71) | 1.62 (1.60-1.66) | -0.61% | 1/7 | 4,800,000,000 | `bd69fbb9ac5c…` | 6.6 / 5.9 |
| pocket-tank | 7.30 (7.18-7.44) | 7.33 (7.20-7.52) | +0.41% | 5/7 | 1,115,772,031 + 946,151,715 | `1d1d57f7c150…` | 5.9 / 5.4 |
| c6-wifi-station | 1.07 (1.06-1.10) | 1.08 (1.05-1.12) | +0.93% | 6/7 | 2,240,000,000 | `75b99af7d413…` | 5.4 / 4.9 |

Flag: `c6-wifi-station`, slower in 6 of 7 pairs, by one 10 ms tick of the timer's resolution
(1.08 s against 1.07 s); the ranges overlap. Not attributed: the station's path has no FP trap,
and the C6 `hello` on the same core is 0.61% faster. The +2.55% and +1.31% C6 `hello` seen on
this branch before (2026-10-25, against an older base and not kept) do not reproduce.

The JSON's ROM paths were written as `$ESP32SIM_ROM_DIR` before it was committed: SHA-256 of the
file as the script wrote it `154db04c59aaa8d8e0c86285fe99bf83fa3cbed86b7bc9201a21f5310de95c3f`,
as committed `ca3936f99db8fae62debadac4ab86fab3eb502fa4e843cfdcb636f3d86a1d3fd`; nothing else
was changed.
