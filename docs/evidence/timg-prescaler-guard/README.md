# EX239: skip the TIMG prescaler division below the divisor

Branch `classic-speed-lever-9`: `ca18773a` (the code, `esp-periph/src/timg.rs`) and two docs commits
on top. Written on main `f1b5c5d5`, rebased onto `ddf753a5`, code diff identical. Row:
[EX239](../../experiments.md#ex239).
Rebased onto `66eb9704`; code diff identical.

## Change

`Timer::step` (classic T0/T1/LACT through `ClassicTimer::tick`, and the shared `TimerGroup::tick`
on the S3, C3, C6 and P4) computed `prescale_acc / div` and `prescale_acc %= div` on every enabled
step, also when the accumulator was still below the divisor and the quotient was 0. It now returns
before the division while `prescale_acc < div`. The one-shot "already reached" alarm check still
runs first and the residual is kept, so counters, alarms and interrupts are unchanged; the old
`if steps == 0 { return; }` is subsumed. No new fields, no scheduler or dispatcher change.

## Checks

Rust 1.99.0 on `cf7e9df3` (the code is the same on this head):

1. `timg::prescaler_tests::short_prescaler_advances_match_division_reference` compares the step
   with the always-dividing version: divider fields 0/1/2/3/40/80/65535, 54- and 64-bit widths, all
   enable/direction/reload/alarm flag combinations, residuals at and above a rewritten divisor, a
   tick sequence through 0/1/div-1/div/3·div/u32::MAX, and 100,000 seeded random states.
2. Six mutants, each killed by that test's assertion (not a compile error): inclusive threshold
   (`<=`), discarded residual, guard before the add, guard before the reached check, guard after the
   modulo, guard on the ticks instead of the accumulator. Control and restored tree pass.
3. Native and WASM Clippy `-D warnings`; empty-HOME workspace tests with `ESP32SIM_ROM_DIR`,
   `--include-ignored --skip external_`: 1323 passed, 0 failed; plain: 1270 passed, 76 ignored;
   WASM build and the eight demos; `node tools/check-evidence-privacy.mjs`. Goldens unchanged.

## Equal work against main

Native, untimed, ROM boot; instruction counts, console SHA-256 and `[emu] stop` lines are the same
on main and the branch. The `tdisplay-prices`, classic hello and 30 s c3-hello rows come from the
uninstrumented `cargo build --release --bins` builds on `ddf753a5` and on the branch; the others
are from the original receipt on `f1b5c5d5`:

| Workload | Chip | Emulated s | Instructions (main = branch) | Console SHA-256 |
| --- | --- | ---: | ---: | --- |
| `tdisplay-prices` (MicroPython v1.29.0) | classic | 15 | 39,313,704 + 109,051,650 | `a831e4615eba…` |
| classic hello | classic | 30 | 16,672,061 + 4,515,255 | `13aadb96ac2d…` |
| hello | S3 | 20 | 7,984,148 + 4,531,031 | `02fd43728071…` |
| pocket-tank | S3 | 10 | 1,391,637,204 + 11,901,258 | `ca6ab239dbf4…` |
| c3-hello | C3 | 30 | 4,800,000,000 | `3b9d8d3eb053…` |
| c3-hello | C3 | 20 | 3,200,000,000 | `a4d0dd5c5968…` |
| c6-hello | C6 | 20 | 3,200,000,000 | `9535b19b279d…` |
| c6-contiki | C6 | 10 | 1,600,000,000 | `101f1060911f…` |
| p4-hello | P4 | 10 | 3,600,000,000 + 0 | `db6c8bcfa9ae…` |

## Operation census on committed firmware

How often the division is skipped. The counting diff is in [census-main.patch](census-main.patch)
(for main) and [census-head.patch](census-head.patch) (for the branch); both count every
`Timer::step` call, the enabled ones, the ones below the divisor after the add, and the executed
prescaler divisions, and print the totals when a `TimerGroup` is dropped. Untimed. Commands,
counts and stop lines: [census.json](census.json).

```sh
tools/fetch-demo-assets.sh --no-linux && tools/fetch-micropython.sh
git apply docs/evidence/timg-prescaler-guard/census-head.patch   # census-main.patch on main
cargo +1.99.0 build --release -p esp32sim --bin esp32sim --bin esp32sim-c3
target/release/esp32sim --chip esp32 --rom web/wasm/fw/esp32_rev300_rom.elf --board ttgo-tdisplay \
  --boot rom --console uart0 --flash-mb 4 \
  --flash-at 0x1000=web/wasm/fw/ESP32_GENERIC-20260824-v1.29.0.bin \
  --flash-at 0x200000=web/wasm/fw/public/tdisplay-vfs.bin --max-seconds 15 --no-dump 2>&1 | grep PRESCALER
git apply -R docs/evidence/timg-prescaler-guard/census-head.patch
```

| Workload | Step calls | Enabled | Below the divisor | Divisions main → branch |
| --- | ---: | ---: | ---: | ---: |
| `tdisplay-prices`, 15 s | 39,067,824 | 6,219,219 | 366,835 (5.9%) | 6,219,219 → 5,852,384 |
| classic hello, 30 s | 57,835,410 | 9,523,445 | 40,269 (0.4%) | 9,523,445 → 9,483,176 |
| c3-hello, 30 s | 300,023,984 | 0 | 0 | 0 → 0 |

The step calls, enabled and below-divisor counts, the instructions and the consoles are the same
on both trees; main divides on every enabled step and the branch on every enabled step not below
the divisor.

On the committed firmware the change skips few divisions: 5.9% of the enabled steps on
`tdisplay-prices`, 0.4% on classic hello, and none on c3-hello, whose two timers are never enabled.
The larger share that motivated the change comes from two Arduino-ESP32 images whose sources are
not published, so they are identified by image hash only and are not reproducible from this
repository: a serial echo sketch (IDF 5.5.4; `firmware.bin`
`78232b5acd23dd6d0514c803228247e167759e9e45237ae6e988d1e1ca4f47d3`, `bootloader.bin`
`83e9239d016f18e0b90fb0cac8cbe91cd506fb14852e1b618bc2844579c2a993`, `partitions.bin`
`148b959cbff1c38aa8e1d5c0ba9d612c54997b945e56a63f41223eef650653a1`, `boot_app0.bin`
`f94c5d786a7a8fab06ac5d10e33bf37711a6697636dc037559ea19cc410a17f0`), 120 s: 64,497,453 enabled
steps, 4,372,187 (6.8%) below the divisor; and a TTGO T-Display sketch (`firmware.bin`
`a9517b9068446721928bd8807995692789b2c1c7b49b5f231713b4116b4ac360`, `bootloader.bin`
`a227e3ab93f15f1155efb3144810cc06ff7a259e9bc9b4542417afcc6c238214`, same `partitions.bin`), 5 s:
16,619,027 enabled steps, 7,628,855 (45.9%) below the divisor. Those two were counted with the same
three counters on `f1b5c5d5` (no division counter; divisions there are enabled minus below).

## Hot-path disassembly against main

aarch64, rustc 1.99.0. Two symbols change, both by the same transformation:

| Symbol | main | branch |
| --- | --- | --- |
| `ClassicTimer::tick` (3 inlined steps) | 262 insns, 62 branches, 6 udiv / 6 msub | 253 insns, 50 branches, 6 udiv / 6 msub |
| `TimerGroup::tick`, S3/C3/C6/P4 (2 inlined steps) | 184 non-nop insns, 40 branches, 4 udiv / 4 msub | 178 non-nop insns, 32 branches, 4 udiv / 4 msub |

Main already branched on `acc < div` (the old `steps == 0` return) but computed `udiv` + `msub` +
the residual store before that branch; now they sit behind it. No branch is added; the branch
count falls because the divisor select (field 0 → 65536, 1 → 2) became branch-free (`csel`). The
`udiv` count is unchanged: one prescaler division per step, now behind the branch, plus the cold
autoreload `steps %= p`. `Bus::tick` (classic/S3/C3/C6/P4), classic `Bus::read32`, Xtensa
`Core::run` and `Machine::run_unmodeled` are unchanged; in the multi-chip `esp32sim` binary on
`ddf753a5` some of them differ only by a uniform rodata page-offset shift, and match after
normalising it. Scoped, not whole-binary identity.

## CPU comparison

The maintainer ran `docs/evidence/p4-cpu/cpu_pairs.py` from base `66eb9704` against new
`e657bbf2`, on 2026-10-31. Both trees were extracted with `git archive` to sibling paths of the
same length and built with `cargo build --release --bins`, rustc 1.99.1 (58cf8f9c3 2026-10-13),
aarch64-apple-darwin. Apple M5 Max (18 cores), macOS 27.0.1, no concurrent builds; ROMs
esp-rom-elfs 20260528 via `tools/fetch-rom-elfs.sh`. User CPU, one warmup pair and seven
alternating pairs per workload, from the base tree root. The [posted JSON](290-e657bbf2-66eb9704.json)
contains commands, individual results, instruction counts, console hashes and loads; the maintainer
redacted the ROM directory as `$ESP32SIM_ROM_DIR` (four places), preserving the raw SHA-256 prefix
`bf713d94…ed85` and sanitized prefix `3c562fdd…1933`. No other edits were reported.

| Workload | Base median (range) | New median (range) | Change | New slower | Instructions; console SHA-256 | 1-min load before → after |
| --- | --- | --- | --- | --- | --- | --- |
| s3-hello, 30 s | 0.260 s (0.250–0.270) | 0.260 s (0.250–0.260) | 0.0% | 1/7 | 11,990,305 + 6,798,164; `04d4912461cb…` | 2.77 → 2.79 |
| c3-hello, 30 s | 2.490 s (2.460–2.580) | 2.470 s (2.430–2.670) | −0.8% | 3/7 | 4,800,000,000; `3b9d8d3eb053…` | 2.79 → 3.73 |
| c6-hello, 30 s | 1.630 s (1.590–1.700) | 1.670 s (1.630–1.690) | +2.5% | 4/7 | 4,800,000,000; `bd69fbb9ac5c…` | 3.73 → 3.93 |
| pocket-tank, 8 s | 7.150 s (7.100–7.190) | 7.180 s (7.110–7.190) | +0.4% | 4/7 | 1,115,772,031 + 946,151,715; `1d1d57f7c150…` | 3.93 → 3.85 |

Instructions and console hashes were identical in all 16 runs per workload. The maintainer's
verdict: nothing is flagged (new slower in at least 6/7 pairs, or non-overlapping ranges). C6's
+2.5% is 4/7 with overlapping ranges, noise at the 10 ms user-CPU resolution. S3 hello mostly
idles, so says little about the timer path. This run did not include classic workloads.

### Review cross-check (October 26, 2026)

From the review of `cf7e9df3` (rustc version not reported): user CPU, 1 warm-up and 7 alternating pairs, `cargo build --release
--bins`, 18 cores, 1-minute load about 3, identical instruction counts and consoles in every pair:

| Workload | main | branch | Change | Branch slower |
| --- | ---: | ---: | ---: | ---: |
| c3-hello, 30 s | 2.38 | 2.42 | +1.68% | 7/7 |
| c3-hello, 30 s (repeat) | 2.37 | 2.42 | +2.11% | 5/7 |
| c6-hello, 20 s | 1.09 | 1.10 | +0.92% | 2/7 |
| S3 hello, 20 s | 0.17 | 0.17 | +0.00% | 0/7 |
| classic hello, 30 s (test-built branch binary) | 0.36 | 0.36 | +0.00% | 0/7 |
| `tdisplay-prices`, 15 s | 1.02 | 1.01 | −0.98% | 0/7 |

### The C3

Where the C3's +1.7% to +2.1% can and cannot come from ([c3-attribution.json](c3-attribution.json)):

1. **c3-hello never enables a TIMG timer.** The census counts 300,023,984 `Timer::step` calls in
   30 s and 0 enabled ones, so every call returns at the `TIMG_T0_EN` test. In
   `TimerGroup::tick` that path is the same five instructions on both builds (`ldr`, `tbz` to the
   second timer, `ldr`, `tbnz`, `ret`); on rustc 1.99.0 against `ddf753a5` the entry address is the same, and the shared exit moves
   from +0xc4 to +0xb8.
2. **Nothing else the C3 runs changes.** In `esp32sim-c3` (built with `--bins` as for the timing),
   on rustc 1.99.0 against `ddf753a5`, 2812 of 2813 text symbols sit at the same address on both builds and 3171 of 2,373,120 `__text`
   bytes differ: 667 in `TimerGroup::tick`, and the rest in the classic `ClassicTimer::tick`,
   `ClassicRmt::advance` (which follows it and moved), one call site in `ClassicRmt::tick` and one
   byte in a classic `mmio::write32` instance, none of which a C3 run executes. `__const` and
   `__cstring` are identical.
3. **A `sample` profile does not show `TimerGroup::tick` growing.** Four 6 s profiles of c3-hello,
   alternating, 1-minute load about 5: `TimerGroup::tick` holds 96 / 106 top-of-stack samples on
   main (2.0%, 2.1%) and 66 / 108 on the branch (1.4%, 2.2%), with `SocBus::refresh_irq` (about
   57%), `SocBus::next_deadline` (13%) and `Bus::tick` (11%) on top in the same order. Its samples
   sit on the two `TIMG_T0_EN` tests and the exit.
4. **A daytime smoke cannot resolve 2%.** c3-hello 30 s, 1 warm-up and 3 alternating pairs, on a
   loaded machine (1-minute load 6 to 16): branch +26.9% (slower 3/3), −1.7% (0/3) and −6.5%
   (0/3); a main-against-main control in the same session gave +8.2% (2/3). These are not CPU
   results.

The checked C3 instructions are unchanged, but layout depends on toolchain and base. The maintainer
rechecked rustc 1.99.1 against `66eb9704`: 2,726 of 2,919 text symbols moved by −0x40, including
`TimerGroup::tick` and `SocBus::refresh_irq`. Thus the same-layout observation above is specific to
rustc 1.99.0 and its base, not a general claim. The maintainer's 1.99.1 CPU run above has no C3 flag
(3/7 slower). His earlier cross-check flagged one C3 run; layout is a plausible explanation, not a
proven cause.

## Limits

- The census counts operations; it is not a timing. The gain on the committed firmware is small
  by construction (5.9% / 0.4% / 0% of enabled steps skip a division).
- The two Arduino images with the larger shares are identified by hash only; their sources are not
  in this repository.
- The `sample` profiles and the smoke ran on a shared, loaded machine; they locate cost, they do
  not measure it.
- No hardware-timing claim.
