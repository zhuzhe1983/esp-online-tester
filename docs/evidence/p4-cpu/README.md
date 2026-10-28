# ESP32-P4 series: CPU cost for the existing chips

The ESP32-P4 series ([#215](https://github.com/mikroverk/esp32sim/issues/215)) changes the shared
RISC-V core, the machine and, later, the C6 and the network. Each part of it is compared with
upstream `main` on the workloads CONTRIBUTING.md names: the S3, C3 and C6 hello demos and
Pocket Tank (from part 6 also the C6 WiFi station). One file per part:

| part | branch | receipt | experiment |
| --- | --- | --- | --- |
| 1 | `p4/01-rv32f` | [01-rv32f.md](01-rv32f.md) | [EX240](../../experiments.md#ex240) |
| 2 | `p4/02-core` | [02-core.md](02-core.md) | [EX241](../../experiments.md#ex241) |
| 3 | `p4/03-chip` | [03-chip.md](03-chip.md) | [EX242](../../experiments.md#ex242) |
| 4 | `p4/04-sdmmc` | [04-sdmmc.md](04-sdmmc.md) | [EX243](../../experiments.md#ex243) |
| 5 | `p4/05-usb` | [05-usb.md](05-usb.md) | [EX244](../../experiments.md#ex244) |
| 6 | `p4/06-coproc` | [06-coproc.md](06-coproc.md) | [EX245](../../experiments.md#ex245) |
| rev ≥ 3 | `p4/rev3` | [07-rev3.md](07-rev3.md) | [EX247](../../experiments.md#ex247) |
| rev ≥ 3, step 1 | `p4/rev3-step1` (against `p4/rev3`) | [08-rev3-step1.md](08-rev3-step1.md) | [EX247](../../experiments.md#ex247) |
| rev ≥ 3, step 2 | `p4/lcd35-board` (against `p4/rev3-step1`) | [09-lcd35-board.md](09-lcd35-board.md) | [EX247](../../experiments.md#ex247) |

## Method

- Base: upstream `main` at `20117b08`, extracted with `git archive` and built with
  `cargo build --release --bins` (the workspace's release profile: fat LTO, one codegen unit).
  The branch is built the same way in a sibling directory whose path has the same length.
  Toolchain: rustc 1.99.1 (58cf8f9c3 2026-10-13), the newest stable on the day (`rustup update
  stable`), aarch64-apple-darwin.
- After the rebase onto `main` `b97fdc7c` each part was run again with that commit as the base,
  the same toolchain and method; that run is the last section of each receipt, with its own JSON
  (`*-b97fdc7c.json`). The runs before it measured the series as it stood on `20117b08`; those
  commits are kept on Xorlev/esp32sim under the tag `p4-pre-rebase-2026-10-23`.
- [cpu_pairs.py](cpu_pairs.py) runs each workload as one warmup pair (branch, base), then seven
  pairs alternating base-first and branch-first, from the base tree's root, so both binaries
  read the same `web/wasm/fw` files. User CPU per run is the child's from `os.times()` (10 ms
  resolution on macOS); the change is the ratio of the medians. Every run records the stop
  line's instruction count and the SHA-256 of the console; `identical_work_and_console` says
  whether all fifteen runs of a workload agree.
- Workloads, as the script spells them: S3 `hello` (`--board none`, 30 emulated seconds),
  `c3-hello` and `c6-hello` (30 s, `--flash-mb 4`), Pocket Tank (`--board
  waveshare-amoled18-v2`, 16 MB flash, 8 MB PSRAM, the model from `tools/fetch-pocket-tank.sh`
  at 0x290000, 8 s), and `c6-wifi-station` as the `wifi_station_c6` golden runs it (14 s).
- Machine: an Apple M2 Max (12 cores) on macOS 15.1, not otherwise idle: the 1-minute load
  average before and after each workload is in the tables. No load gate.
- Flag, as other receipts here use it: the branch slower in at least 6 of 7 pairs, or ranges
  that do not overlap.

The JSON files hold every run (side, pair, order, user seconds, instructions, console SHA-256)
and the exact commands, with the ROM directory written as `$ESP32SIM_ROM_DIR`.
