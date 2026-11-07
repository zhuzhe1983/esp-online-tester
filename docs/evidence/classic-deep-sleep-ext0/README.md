# Classic ESP32 deep sleep and EXT0 wake

Base: upstream main `77e43c20`, including the RTC FAST-memory CRC completion
fix merged in #238 (`c93278e3`). Only the deep-sleep change remains above main.
The original comparisons below used CRC source `837ec1ce300ec5cc5f3a7bb5427698ab7a64cd91`;
they are historical measurements, not measurements of the rebased binary.
This change does not change the explicitly approximate CRC parameters and does
not patch ROM, IDF or firmware. Experiment [EX233](../../experiments.md#ex233)
extends [EX230](../../experiments.md#ex230).

## What and why

A valid deep-sleep call previously got past the ROM CRC poll, wrote SLEEP_EN,
and spun in `rtc_sleep_start` forever. GPIO0 could not wake the chip.

On classic ESP32 only, a SLEEP_EN write with digital-domain power-down now:

- Retires no further guest instructions on either CPU until reset. A cold
  peripheral-change callback parks the CPU snapshots using the scheduler's
  existing idle mechanism. The callback runs after both pre-instruction and
  after-round IRQ presentation, inside their existing dirty/event arms. Digital
  interrupts, including late or latched NMI and local timer interrupts, cannot
  release powered-off CPUs. Releasing the secondary
  core from reset during sleep cannot release it from power-down.
- Retains RTC slow/fast RAM unless its PWC power-down setting requests loss;
  FORCE_PU overrides automatic PD_EN. Powered-down bytes become zero as a
  deterministic model choice. Code page versions are invalidated on RAM loss.
- Discards digital timer-group state and gates the existing Wi-Fi, LEDC, RMT and
  I2S clocks once at entry, instead of adding sleep branches to their ticks.
- Keeps virtual time and the existing RTC slow clock running. An enabled EXT0
  at the selected RTC IO's level requests the existing system-reset path with
  reset reason 5 and EXT0 wake cause. ROM runs again, with RTC registers and
  retained memory available to its normal wake-stub handling.

EXT0 uses RTC IO numbering (RTC IO11 is GPIO0), the pad's RTC mux, the programmed
polarity, and a powered RTC peripheral domain. Detection runs at SLEEP_EN and
on host/board GPIO input changes or release; there is no EXT0 tick poll.
An already active EXT0 wakes immediately, not SLP_REJECT. GPIO/SDIO rejection
is not implemented by this change.

## Register sources and model limits

All sources below are public ESP-IDF v5.5.4 files at
https://github.com/espressif/esp-idf/tree/v5.5.4 .

| Meaning | Source |
| --- | --- |
| STATE0 +0x18, SLEEP_EN bit31 | components/soc/esp32/register/soc/rtc_cntl_reg.h:246-252 |
| DIG_PWC +0x84, DG_WRAP_PD_EN bit31 | same header:1281-1287; components/esp_hw_support/port/esp32/rtc_sleep.c:199-212 |
| WAKEUP_STATE +0x38, ENA[22:11], CAUSE[10:0] | rtc_cntl_reg.h:505-529 |
| EXT0 trigger bit0 | components/esp_hw_support/port/esp32/include/soc/rtc.h:584 |
| EXT_WAKEUP_CONF +0x60, EXT0 level bit30 | rtc_cntl_reg.h:795-807 |
| RTC_IO +0xbc, EXT_WAKEUP0_SEL[31:27] | components/soc/esp32/register/soc/rtc_io_reg.h:1895-1901 |
| RTC IO to GPIO mapping | components/soc/esp32/rtc_io_periph.c:10-50,54-74; sleep_modes.c:1912-1932 |
| PWC +0x80: RTC peripheral/slow/fast RAM PD_EN, FORCE_PU, FORCE_PD | rtc_cntl_reg.h:1134-1194; rtc_sleep.c:159-181 |
| Deep sleep returns through reset, not SLP_WAKEUP resume | rtc_sleep.c:249-270,360-369; components/esp_hw_support/sleep_modes.c deep-sleep path |
| Only GPIO/SDIO can reject on classic, not EXT0 | rtc_sleep.c:249-258 |
| DEEPSLEEP_RESET = 5 | components/esp_rom/esp32/include/esp32/rom/rtc.h:79 |

The wake cause is latched read-only; programming wake enables cannot erase it.
No analog delay, electrical validation or silicon timing claim. Entry, RAM loss
and digital-engine gating are synchronous coarse model choices. The retained
CPU debug snapshots use the existing idle accounting, so CCOUNT can advance
while instruction counts and PCs stay fixed; these are not physical running CPUs.
RTC pad analog input gating, full RTC GPIO hold behavior, all digital-domain
power sequencing, and retention of powered-down RTC peripheral registers are
not modeled. The existing reboot path retains RTC IO configuration. This scope
models deep sleep with EXT0 and, since #336, the RTC timer alarm (see "RTC timer
wake" below): no light-sleep resume, EXT1, touch, ULP, GPIO/SDIO wake/rejection,
or automatic host reboot policy change.
Other enabled wake bits do not acquire fabricated wake behavior.

## Reproduction

Portable tests use MMIO and machine execution and need no external firmware.
`esp32/tests/deep_sleep.rs` covers both CPUs, wake/reset/cause/retention, disabled,
wrong/invalid pin, RTC mux/domain, polarity, active-level entry, RAM loss and
force-power-up, decoder invalidation, secondary release and the light-sleep
control. The sleep module's unit test covers digital-engine power-down.
Eight additional late-interrupt cases drive a GPIO4 rising edge at 2000 bus
cycles after configuration, targeting PRO or APP, NMI or maskable, and the
modeled or unmodeled scheduler. Each includes an awake positive control that
takes the same interrupt. During deep sleep both PCs and retired counts stay
fixed, no interrupt is taken and no RTC reset is requested; even the maskable
pending snapshot is cleared after presentation. Removing only the after-round
callback reproduces all eight runtime failures (including four NMI escapes).
The deliberately bounded runs may stop at the instruction budget or time limit;
the modeled idle scheduler can also report Halted when no runnable work remains.

The original seven-test portable suite compiles on the accepted CRC base: six
fail at runtime, while the deliberately unchanged light-sleep control passes.
On the candidate all fifteen portable tests pass, as does the new digital-engine
unit test.

Optional public-starter reproduction:

    python3 docs/evidence/classic-deep-sleep-ext0/repro.py \
      BASE_BINARY FIX_BINARY ROM_ELF FIXTURE_DIR BUTTON_SCRIPT OUTPUT_DIR

The supplied fixture directory contains firmware.bin, firmware.elf,
bootloader.bin and partitions.bin. The button script drives GPIO0 low at 1s,
high at 1.15s, low at 3s, high at 3.15s. No firmware is distributed here.
The curated input hashes and assertions are in repro.json; full local logs
are retained separately from source. Trace probes record `_Z5setupv` entry.

No further starter failure was observed after wake: ROM printed
`rst:0x5 (DEEPSLEEP_RESET)` and setup ran again. This is a bounded five-second
starter check, not certification of every deep-sleep sketch or wake stub.

## Reproduce checks

Use Rust 1.99.0, separate base/candidate CARGO_TARGET_DIR directories, and
`nice -n 10` for builds and executions. A shared target between worktrees can
reuse stale Cargo outputs and is not a valid base comparison.

    cargo +1.99.0 test --release -p esp32 --lib --test deep_sleep
    CARGO_TARGET_DIR=... python3 docs/evidence/classic-deep-sleep-ext0/mutate.py OUTPUT_DIR
    python3 docs/evidence/classic-deep-sleep-ext0/codegen.py BASE FIX OUTPUT_DIR
    python3 docs/evidence/classic-deep-sleep-ext0/controls.py BASE FIX OUTPUT_DIR

`mutations.json` maps compile-valid mutations to failing tests. `codegen.json`
uses the EX230 normalizer (address normalization, field offsets/opcodes kept).
The four public hello controls compare per-core instructions, console SHA-256,
and exception/IRQ totals plus source reports, not wall-clock performance.

## Observed starter result

| Run | Core0 PC / retired instructions | Core1 PC / retired instructions |
| --- | --- | --- |
| CRC base, 2 s | 0x40084790 / 39,374,924 | 0x40086b97 / 125,911,570 |
| CRC base, 5 s | 0x40084790 / 159,374,924 | 0x40086b97 / 245,911,570 |
| Fix, 2 s | 0x40084795 / 6,972,154 | 0x40086b91 / 93,508,796 |
| Fix, 2.5 s | 0x40084795 / 6,972,154 | 0x40086b91 / 93,508,796 |

Core1's sleeping PC is `rtc_sleep_start+0x7d`, immediately after its sleep
request rather than the RAW-poll PC `+0x83` on the base. The fixed counts/PCs
establish no guest execution in the measured sleeping interval. At 5 s the
fix has resumed normal execution after the 3 s wake.

    [fn] i=704325 t=0.0621s c1 _Z5setupv(...) ret=0x400da1b4
    [emu] chip reset at t=3.000s: cause 0x5 (DEEPSLEEP)
    rst:0x5 (DEEPSLEEP_RESET),boot:0x13 (SPI_FAST_FLASH_BOOT)
    [fn] i=94213759 t=3.0123s c1 _Z5setupv(...) ret=0x400da1b4

After wake, RESET_STATE is 0x145 (5 for each core) and WAKEUP_STATE is 0x801
(EXT0 enable plus EXT0 cause). `repro.json` records the complete compact values.
The base has one setup entry and no deep-sleep reset after the second press.

## Static code comparison and controls

100/106 selected normalized native functions are identical against the accepted
CRC SHA. All selected bus ticks/reads/fetches, Xtensa execution functions and
S3/C3/C6 scheduler functions are identical. Six classic-specific selections differ:

| Selection | Base / fix instructions | Explanation |
| --- | --- | --- |
| SocBus::store | 2722 / 2761 | SLEEP_EN MMIO check/call and GPIO-edge wake callback; first 335 normalized instructions and 0xc10 frame allocation match. |
| ClassicRtc::write | 58 / 60 | Preserve the read-only wake cause when programming enable bits. |
| classic after_round | 84 / 134 | Park powered-off CPUs after late IRQ presentation inside the existing dirty/ticked arm; first 13 normalized instructions and 0x40 frame unchanged. The added loop is behind the existing event gate and the deep-sleep branch. |
| classic run_modeled | 3445 / 3454 | Existing dirty peripheral boundary invokes CPU parking; 0x280 frame unchanged. |
| classic run_unmodeled | 1708 / 1784 | Same cold boundary and secondary-release arm; compiler register allocation/spill/layout changes; 0x1d0 frame unchanged. |
| classic step_core | 838 / 894 | Inlined cold event callback, allocation/spill/layout changes; 0x140 frame unchanged. |

The dirty=false early return is still before CPU parking. No new tick or clean
instruction-dispatch sleep poll is introduced. This is not whole-binary identity
or proof of zero CPU-time impact: modified cold MMIO/event paths and compiler
layout changes are explicit. The upstream review's historical classic-hello CPU
table is recorded below; rebased-head timing remains unmeasured. The new entry helper
is out of line. `codegen.json` and `layout-summary.json` retain method, hashes,
normalized counts and frame/prefix details; full disassemblies are retained locally.

All four three-second public hello controls match per-core instruction counts,
console SHA-256 and full exception/IRQ reports; see `controls.json` for input
hashes and results. No firmware goldens were changed.

## Original reviewed-head checks

The following results describe `schematik-engineering/esp32sim@91b204cf`.
Rebased-head checks are recorded separately in `rebase-checks.json`.

Rust 1.99.0; all builds/executions niced. Native Clippy `--workspace
--all-targets -- -D warnings` and release wasm32 `jit-tests` Clippy pass.
Empty-HOME CI-policy release workspace tests (explicit ESP32SIM_ROM_DIR,
`--include-ignored --skip external_`): 1003 passed, zero failed/ignored.
Plain empty-HOME release workspace tests (ROM override unset): 975 passed,
zero failed, 50 ignored. Cargo/rustup homes were retained explicitly.
Production `tools/wasm-build.sh` and all eight required WASM demos pass.
All 32 mutations compile and fail runtime assertions; `mutations.json` names
the killing tests. Restored candidate tests pass as part of the full suite.

Workspace `cargo +1.99.0 fmt --all -- --check` fails on both base and fix:
4176 versus 4178 diff hunks in the existing unformatted workspace; added code
splits existing hunks. No mass reformat. New sleep source/test files pass
`rustfmt +1.99.0 --edition 2021 --check`. No golden changes. Evidence privacy
pattern check and manual review pass. Complete commands/results are in
`checks.json`. JIT implementation was not changed.

## Rebased-head verification (October 21, 2026)

On main `77e43c20`, the full `ci.yml` command set passes locally with Rust
1.99.0, `nice -n 10` and two build jobs, including both Clippy targets, native
release build, virtual-quantum tests, nine production WASM demos, section checks,
timing/BLE ABI tests, worker/browser/retained-receipt contracts, generated-WASM
differential tests and profiling-feature compilation. The native AArch64 core
check ran on macOS arm64 rather than CI's Ubuntu arm64.

Empty-HOME CI-policy tests: 1,026 passed, zero failed/ignored. Plain empty-HOME
tests with no ROM override: 998 passed, zero failed, 50 ignored. The JIT suite
passes 111,126 differential cases plus 24 GPIO-waveform cases. All 32 removal
mutations compile and fail runtime assertions; the restored classic unit and
deep-sleep tests pass (137 tests). Privacy patterns and manual review pass.
`rebase-checks.json` records commands, counts and limits; `rebase-mutations.json`
names each killing test.

The same Hairclip firmware/ROM/button-script hashes reproduce all five original
snapshots exactly: both-core PCs/counts, RTC registers, setup traces, reset causes
and console hashes. Four public hello controls also match the retained reviewed
binary's instructions, console hashes and interrupt reports. No tracked golden
or firmware fixture changed. These are correctness runs, not CPU measurements;
no new hot-path disassembly identity is claimed for the rebased binary.

## CPU comparison

### Upstream review measurement (October 21, 2026)

Measured by joakimeriksson in the [upstream review](https://github.com/mikroverk/esp32sim/pull/240#issuecomment-6309114052)
(at 05:52:33 UTC): classic hello, 30 emulated seconds, main `63670bb1`
versus reviewed head `91b204cf`, seven alternating pairs, identical instruction
counts and console hashes.

| Workload | Main median user CPU | Reviewed head median user CPU | Change | Slower pairs |
| --- | --- | --- | --- | --- |
| Classic hello, 30 s | 0.370 s | 0.370 s | +0.0% | 2/7 |

Neutral for this workload, which does not enter deep sleep. The review did not
provide raw samples, a load gate or warmup details. This is the maintainer's
measurement of the original reviewed head, not a new timing result for the
rebased binary and not a hardware sleep-timing claim.

### Independent night measurement

Not measured on the rebased head. No local CPU timing was run for this rebase;
the independent night-window table remains separate from the upstream result.

## RTC timer wake (#336)

[#336](https://github.com/mikroverk/esp32sim/pull/336) adds the RTC timer as a
deep-sleep wake source. While `deep_sleep` holds, `SocBus::tick` compares the
RTC slow counter, which keeps running through sleep, with SLP_TIMER0/1 when
WAKEUP_ENA has RTC_TIMER_TRIG_EN. At or past the target it wakes through the
EXT0 path's reset: cause bit 3, reset cause 5, SLEEP_EN cleared, so it fires
once. Awake, the tick pays one `deep_sleep` branch. A target of 0 counts as none
written, because WAKEUP_ENA resets with the timer bit set; that is a model
choice, not a silicon measurement. There is no device deadline: the idle step
is already capped at `IDLE_CHUNK` (512 cycles on classic) and a deadline can
only shorten it.

| Meaning | Source (ESP-IDF v5.5.4) |
| --- | --- |
| `rtc_cntl_ll_set_wakeup_timer` writes SLP_TIMER0/1 only, not MAIN_TIMER_ALARM_EN | components/hal/esp32/include/hal/rtc_cntl_ll.h:19-23 |
| SLP_TIMER0 +0x04, SLP_TIMER1 +0x08 (16-bit high word, ALARM_EN bit16) | components/soc/esp32/register/soc/rtc_cntl_reg.h:194-214 |
| WAKEUP_ENA [21:11], reset 0b1100; WAKEUP_CAUSE [10:0] | rtc_cntl_reg.h:505-523 |
| RTC_EXT0_TRIG_EN bit0, RTC_TIMER_TRIG_EN bit3 | components/esp_hw_support/port/esp32/include/soc/rtc.h:584,587 |

### Upstream review (November 3, 2026)

By joakimeriksson in the [upstream review](https://github.com/mikroverk/esp32sim/pull/336#issuecomment-6599264367)
of head `512f9bd4`:

- ESP-IDF v5.5.4 `examples/system/deep_sleep` for classic (sdkconfig defaults:
  RC slow clock, RTC HRT time, EXT1 off), `--board none --max-seconds 50`. Base
  `04f07602` logs "Entering deep sleep" and never wakes. The branch resets at
  t=21.046 s and t=42.072 s with `rst:0x5 (DEEPSLEEP_RESET)` and prints "Wake up
  from timer. Time spent in deep sleep: 20021ms". With `CONFIG_EXAMPLE_EXT0_WAKEUP=y`
  and GPIO25 driven high at 6 s: "Wake up from ext0" at t=6.000 s, then "Wake up
  from timer" at t=27.026 s.
- The modeled 150 kHz slow clock matches the TIMG calibration, about 2.4 ppm
  short with 1024 calibration cycles (about 1.4 ms over 600 s).
- Without the target-0 guard, the eight `late_*` tests and
  `secondary_release_cannot_escape_power_down` in `esp32/tests/deep_sleep.rs`
  fail, plus `a_target_never_written_wakes_nothing`.
- Sleep time is skipped, not paced: the 50 s run takes 0.19 s user CPU;
  `--realtime --max-seconds 23` takes 23.0 s wall and 0.15 s CPU.
- Rust 1.99.1: both Clippy runs clean; `--include-ignored --skip external_`
  1464 passed, 0 failed; evidence privacy check passes.

CPU: classic hello, 30 s, new `512f9bd4` vs base `04f07602`, one warmup and
seven alternating pairs, user CPU, Apple M5 Max, 1-minute load 4.34 → 4.31,
identical instruction counts and console hashes in all 16 runs.

| Workload | Base median (range) | New median (range) | Change | New slower |
| --- | --- | --- | --- | --- |
| Classic hello, 30 s | 0.380 s (0.380–0.400) | 0.390 s (0.380–0.390) | +2.6% | 1/7 |

Not flagged: one 10 ms rusage tick is 2.6% here. The firmware never sleeps, so
this times the `deep_sleep` branch in the tick, not the wake. Samples:
[`336-512f9bd4-04f07602.json`](336-512f9bd4-04f07602.json), the reviewer's file
with the ROM directory written as `$ESP32SIM_ROM_DIR`; raw SHA-256 `b6b08bd2…`,
sanitized `519c3987…` (this file).

### After the review

The device deadline (`ClassicRtc::next_deadline`) was removed after the review:
it keyed off SLEEP_EN rather than deep sleep, so an attempted light sleep left
the bus deadline at `Some(1)`, and `idle_budget` only takes the minimum of it
and `IDLE_CHUNK`, so it could not skip time. Not re-measured on the classic
hello workload.
