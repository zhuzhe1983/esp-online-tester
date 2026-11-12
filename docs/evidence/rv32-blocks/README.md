# EX265: decoded blocks for the RV32 cores, measured on the ESP32-S31

Branch `riscv/jit`, on `s31/takeover` `732f8a2a` (the base). Commits, oldest first:

| Commit | What |
| --- | --- |
| `75ca7b29` | `emu_core::Bus::fetch_ahead`: an instruction read without `fetch`'s side effects |
| `6e63d000` | S31 code versions (`page_versions`/`code_page`) and `fetch_ahead` |
| `c4e4dc95` | `riscv-rv32::block`: decoded basic blocks, wired into `Core::run`; differential tests |
| `13a1ec33` | S31: a store to plain memory ends the dispatch only if the interrupt refresh can change something |
| `df53d825` | S31: plain-memory fast paths in `read`/`write`; remembered CLIC routing |
| `c5c4f378` | `exec_insn` forced inline in the block loop only (fixes the C3 flag of round 1) |

## Workload

`s31-mosaico`: the ESP-Mosaico factory firmware from a private flash dump, booted from the mask
ROM (`esp32s31_rev0_rom.elf`), board `mosaico`, 16 MiB flash, a USB host script, 10 emulated
seconds, `--no-dump`. The image, the USB script and everything derived from them (console text,
console hashes) stay out of this receipt; the harness takes them from `S31_FLASH_IMAGE` and
`S31_USB_HOST`. Work check: both cores' instruction counts (core0 3,200,000,000, core1
3,125,158,720), the console's SHA-256 (compared, not recorded) and, in separate runs, the whole
end-of-run report minus wall time and the block statistics line (exception and interrupt
counts and the per-line interrupt histogram; compared, not recorded) are identical for base, new
and new `--no-jit`.

The C3 and C6 workloads are the public `c3-hello`, `c6-hello`, `c3-wifi-station` and
`c6-wifi-station` of the earlier CPU tables (firmware in `web/wasm/fw/public`).

## Method

`cpu_pairs.py` (this folder): CONTRIBUTING's "Speed" method. Per workload one warm-up pair, then
alternating base→new and new→base pairs; child user CPU from `wait4`; the change is the ratio of
the medians. Base and new are `cargo build --release --bins` of `732f8a2a` and of the branch in
separate target directories. Apple M5 Max (18 cores), macOS 27.2.1, rustc 1.99.1; other work on
the machine (load averages are in the JSON); the user's own emulator process kept running.

## Results

Median child user CPU, alternating pairs, identical instruction counts and console in every run.

| Workload | Base s | New s | Change | New slower | Round |
| --- | --- | --- | --- | --- | --- |
| s31-mosaico | 41.49 | 14.03 | **-66.18%** (2.96x) | 0/7 | 3 (`c5c4f378`) |
| c6-hello | 1.59 | 1.53 | -3.77% | 0/7 | 3 |
| c6-wifi-station | 1.09 | 1.07 | -1.83% | 1/7 | 3 |
| c3-hello | 2.52 | 2.49 | -1.19% | 1/7 | 2 (`c5c4f378`) |
| c3-wifi-station | 1.53 | 1.53 | +0.00% | 4/7 | 2 |

Ranges (s31-mosaico): base 41.32-41.96 s, new 13.99-14.12 s (see `pairs-r3.json`). The S31
emulates 10 s of both cores in 14 s of one host core: about 0.71x real time single-threaded, from
0.24x; 451 M instructions (idle-skipped included) per CPU second.

Attribution, S31 only: blocks against the same build with `--no-jit` (bus changes in both),
5 pairs: 26.28 s → 14.06 s, **-46.50%**, 0/5 slower (`pairs-blocks.json`). The bus changes alone
are then about 41.5 → 26.3 s (-37%), a figure derived across two runs, not a paired measurement.

Round 1 (`df53d825`, `exec_insn` forced inline everywhere) flagged the C3: c3-hello +6.40%
(6/7 slower), c3-wifi-station +3.92% (6/7); S31 -66.23%, c6-hello -3.11%, c6-wifi-station
+0.00% (2/7). `c5c4f378` removed the cause (round 2).

## Where the time went (macOS `sample`, top of stack, one 10-15 s window each)

- Base: interrupt-line refresh after every store 33% (`SocBus::refresh_irq` 2544 of ~12,800
  samples, `present_irqs` 772, `peripheral_irqs` 568, USB `gintsts_now` 356); instruction fetch
  ~24% (`fetch` 2480, cache `read_byte_core`, MMU `translate`, APM checks); `step_outcome` 20%;
  the device dispatch chain of loads and stores most of the rest.
- The guest profile of the private image (`--profile`) was read to steer the work and is not
  recorded here.
- New: the block loop and `exec_insn` ~40%; device time per round (`tick`, USB OTG) ~10%;
  interrupt refresh at dispatch starts ~15%; `Power::is_mmio` and the rest of the chain for
  device accesses ~10%.

## Steps and negative results inside the experiment (single 10 s runs, user s, same work)

Development runs, before the branch moved onto `732f8a2a` (base then `3fb2214f`, whose tree is
the same as `732f8a2a` without the hardware loops; same instruction counts). One run each,
some under load: indicative only.


| Step | user s |
| --- | --- |
| base | 41.9 |
| blocks only (SRAM/ROM/flash, CSR reads ending a block) | 37.7 |
| + memory stores no longer end the dispatch | 26.9 |
| + PSRAM-window blocks, CSR reads inside blocks, self-loops | 21.2 |
| + only fetch-relevant cache registers invalidate (cache maintenance moved every PSRAM version 121,850 times) | 20.3 |
| + SRAM/LP/ROM fast path, CLIC routing memo | 16.4 |
| + `exec_insn` inline in the block loop | 15.8 |
| + 64 K-entry table, slot reuse (builds 2.03 M → 0.60 M on core0) | 15.7 |
| + cache-window fast path | 14.5 |

- Bumping the flash version on every SPI1 command (103,029 in 10 s, firmware reads through
  SPI1) did not change the block-build count here; it is now limited to commands that can change
  the array (`Mspi::flash_generation`).
- A self-loop shortcut that also covered blocks ending in `mret` was wrong (an interrupt became
  pending after `mret` set MIE); the fuzz found it at seed 676 and it now needs a branch or jump
  at the end.
- `#[inline(always)]` on `exec_insn` everywhere slowed the C3 interpreter (round 1 below); the
  block loop alone gets the forced inlining now.

## Round 4: music playback on the Mosaico (2026-11-12)

**This round measures the TSF latch fix of #354 (`19386788`), not the block engine:** base and
new do different guest work, so its gain is not an EX265 speed result. It is kept here because it
answers the question that followed EX265 (does music play at real time?) on the EX265 build.

Question: does the Mosaico play music at real time or faster on one host core? Base `38920eed`
(the EX265 branch as recorded above); new `39ca9022`, the same plus `s31/takeover`'s `19386788`
cherry-picked (a write setting bit n of 0x2010D814 latches emulated microseconds into
0x2010D820/0x2010D824, the modem TSF timer read of the Wi-Fi library's `hal_get_tsf_time`).
`c66cd31c` adds only a unit test. No emulator speed change was made in this round: the latch is a
behaviour fix, and with it the Wi-Fi library stops spinning on a TSF that never moved.

Workloads (private, as above, plus a touch script `S31_SCRIPT`/`S31_MUSIC_SCRIPT` that opens the
music app and starts playback). An earlier `s31-music` script paused playback part-way, so it is a
whole-run figure, not a playback figure. The
playback phase is measured with the same script minus that last touch (playing until the end of
every run): `play_pairs.py` runs each side to 23 s and to 40 s back to back and takes the
difference (17 emulated seconds of playback). A 40 s `--wav` of the new build has music of
non-zero RMS in every second after playback starts.

Work check: the TSF fix changes what the guest does, so base and new do different work (same
instruction counts, which the clock fixes; different exception and interrupt counts, compared but
not recorded). Within each side every run was identical (both cores' instruction counts, exception
and interrupt counts, console hash; compared, not recorded).
New against new `--no-jit`, 40 s of playback: console and the whole end-of-run report identical.

Median child user CPU, 7 alternating pairs, same machine and method as above (load 3.5-4.6):

| Workload | Base s | New s | Change | New slower | Speed (emulated/CPU) |
| --- | --- | --- | --- | --- | --- |
| playback phase, 23 -> 40 s | 18.53 | 8.73 | -52.89% | 0/7 | 0.92x -> **1.95x** |
| to 23 s (boot, UI, open app) | 30.36 | 21.35 | -29.68% | 0/7 | 0.76x -> 1.08x |
| to 40 s, playing | 49.19 | 30.07 | -38.87% | 0/7 | 0.81x -> 1.33x |
| s31-music (40 s, paused at 23.8 s) | 47.06 | 28.46 | -39.52% | 0/7 | 0.85x -> 1.41x |
| s31-mosaico (10 s boot, no script) | 14.04 | 12.46 | -11.25% | 0/7 | 0.71x -> 0.80x |
| c3-hello | 2.50 | 2.50 | +0.00% | 2/7 | |
| c6-hello | 1.58 | 1.56 | -1.27% | 3/7 | |
| c3-wifi-station | 1.54 | 1.54 | +0.00% | 3/7 | |
| c6-wifi-station | 1.08 | 1.07 | -0.93% | 2/7 | |

Phase ranges: base 18.29-19.38 s, new 8.31-9.08 s (1.87x-2.05x). C3/C6 did identical work on both
sides; their code did not change, so those rows are a no-regression check only.

Host profile of the new build during playback (macOS `sample`, 6 s, top of stack, ~1,350
samples): block loop 21%, `SocBus::tick` 14%, USB OTG `gintsts_now` 12%, `next_deadline` 4%, I2S
`receive_input` 3%, `run_unmodeled` 3%, USB OTG `tick` 3%, `core_irqs` 3%, `Clocks::rates_of` 2%,
`refresh_inputs` 2%. The guest idles most of playback, so per-round device time (`tick`, USB
OTG, deadlines, I2S) is now the largest cost (~40%); that, not the device dispatch chain, is the
next lever if more margin is wanted. Not tried in this round: table-driven device dispatch, a TSF
page fast path (its earlier -8.6% was measured while the library spun on the frozen TSF).

Files: `play_pairs.py` (the phase harness), `pairs-play.json` (phase pairs), `pairs-r4.json`
(whole runs, S31 console hashes removed and paths replaced). `cpu_pairs.py` gained `s31-music`.

## Correctness evidence

- `riscv-rv32/tests/blocks_differential.rs`: random programs on C3, C6, P4 rev 3 and S31 cores,
  blocks against the interpreter, after every `run` call: iterations, trap, all architectural
  state (x, f, q, CSRs, counters, hardware loops, reservation) and memory. Default 1,600
  programs / 480,000 run calls in the gate; a 12,000-program run (`RV_BLOCK_SEEDS=3000`)
  compared 3,600,000 run calls, 950,493 blocks built, 29,032,135 instructions. Targeted tests:
  a store into its own block, a precise mid-block fault, a bus without versions, a hardware loop
  whose body spans a block.
- Mutations of `block.rs`, each killed by the fuzz (1,500 seeds): no hardware-loop back-edge; no
  block exit after a back-edge; no version re-check after stores; self-loop reuse after `mret`;
  CSR writes not ending a block (killed after the generator was made to set mstatus bits).
- `esp32s31`: `fetch_ahead` equals `fetch` (and records no fault) on SRAM/LP/ROM, flash-window
  mapped and unmapped, PSRAM, device and wrapping addresses; code versions move on page stores,
  next-page-start stores, DMA writes, loading, APM writes and flash MMU writes, and not on other
  stores or cache maintenance; memory stores refresh interrupts only after a device access or a
  dispatch start and break only on a change.
- Gate on `c5c4f378`: both Clippy runs clean; workspace tests 1,722 passed, 2 failed (the known
  S31 hello goldens without IDF v6.1 images: `S31_ROM_ELF: NotPresent`).

## Files

- `cpu_pairs.py`: the harness (S31 paths from the environment).
- `pairs-r1.json`: round 1, all workloads, `df53d825` (C3 flagged).
- `pairs-r2.json`: C3 re-run on `c5c4f378`.
- `pairs-r3.json`: S31 and C6 on `c5c4f378`.
- `pairs-blocks.json`: S31, `c5c4f378` with `--no-jit` (base) against blocks (new).

The S31 records omit the console hashes and replace paths with `$S31_FLASH_IMAGE`,
`$S31_USB_HOST` and `$ESP32SIM_ROM_DIR`.
