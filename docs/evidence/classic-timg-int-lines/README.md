# Classic TIMG interrupt lines ignore INT_ENA

On the classic ESP32 the timer-group interrupt lines (T0, T1 and LACT; level and edge) now
follow INT_RAW and each timer's own EDGE_INT_EN (bit 12) / LEVEL_INT_EN (bit 11) bits.
INT_ENA (0x98) only masks INT_ST (0xa0). Before, a line also needed INT_ENA, so firmware that
arms an alarm with LEVEL_INT_EN alone never got the interrupt. esp-hal 1.x does exactly that
on the ESP32 and never writes INT_ENA there, so its embassy/esp-rtos timers never fired.
IDF 4.4 clears both enables when disabling interrupts. IDF 5.x can clear INT_ENA alone
while an alarm is armed; the resulting asserted level line changes emulator behaviour.

One function changes, `ClassicTimer::irq_bits` in `esp32/src/timers.rs`. INT_ST, INT_CLR, the
alarm logic, the shared `esp_periph::TimerGroup` (S3/C3/C6/P4) and the WDT are untouched.

## Sources and silicon observation

1. esp-hal 1.1.2 `esp-hal/src/timer/timg.rs:517-525`, `set_interrupt_enabled` on
   `cfg(any(esp32, esp32s2))`: "On ESP32 and S2, the `int_ena` register is ineffective -
   interrupts fire even without int_ena enabling them. We use level interrupts so that we
   have a status bit available." It writes only `level_int_en`. File sha256 `316d4fab…`.
2. ESP32 TRM v5.9, chapter 10.4, Register 10.1 (TIMGn_TxCONFIG_REG): Tx_EDGE_INT_EN "When set,
   an alarm will generate an edge type interrupt", Tx_LEVEL_INT_EN "... a level type
   interrupt". Register 10.22 (INT_ST): "the masked interrupt status". Register 10.20 (INT_ENA):
   "the interrupt enable bit". The TRM does not say whether INT_ENA gates the line to the
   interrupt matrix.
3. ESP-IDF v6.0.1 `components/esp_hal_timg/esp32/include/hal/timer_ll.h`:
   `timer_ll_enable_alarm` (line 82) sets `tx_alarm_en` and `tx_level_int_en` together;
   `timer_ll_enable_intr` (line 237) sets `int_ena_timers` separately. Bit positions: ESP-IDF
   v5.5.4 / v6.0.1 `components/soc/esp32/register/soc/timer_group_reg.h` (EDGE bit 12, LEVEL
   bit 11, lines 42-53).
4. Espressif QEMU (`espressif/qemu`, esp-develop `febae182`, `hw/timer/esp32_timg.c`)
   **differs**: `esp32_timg_timer_cb` (lines 352-362) raises the level line and pulses the
   edge line only when `s->int_ena & int_mask`. Its WDT path (line 574) raises the level line
   regardless, with the comment "should be checking s->int_ena & mask, but ESP32 seems to
   raise interrupt regardless?". File sha256 `8d4dae15…`. QEMU also sets INT_RAW only when
   LEVEL_INT_EN is set; esp32sim keeps setting INT_RAW on every alarm (unchanged here).

5. [Maintainer's PR #314 silicon follow-up](https://github.com/mikroverk/esp32sim/pull/314#issuecomment-6467469287):
   ESP32-D0WDQ6-V3, IDF v5.5.4 legacy T0 auto-reload at 10 ms. Masking INT_ENA with the ISR
   installed hung after `hits=9`; a 25 s capture saw a TG0 watchdog reset. Removing the
   disable/enable pair completed with `hits_during_mask=20` and `done hits=39`. The maintainer
   observed the same stuck line in this PR's emulator; main instead completed. This supports
   the T0 level rule, not T1, edge lines or LACT.

## Tests

`esp32/src/timers.rs`:

- `alarm_lines_follow_raw_status_and_int_en_bits_not_int_ena`: T0, T1 and LACT each fire
  with INT_ENA 0; for INT_ENA 0, own bit, other bits and all bits the level, edge, both and
  neither lines match the EN bits; INT_CLR drops the line with INT_ENA set.
- `int_st_is_raw_masked_by_int_ena`: raw 0b1011 (T0, T1, LACT) against all 16 INT_ENA values
  (INT_ENA reads back, INT_ST = INT_RAW & INT_ENA, lines unchanged), then INT_CLR per bit and
  INT_ENA still set with every raw bit cleared (INT_ST 0).

`nice -n 10 cargo +1.99.0 test --release -p esp32 --lib timers`: 5 passed.

## Mutations

Twelve single replacements in `esp32/src/timers.rs`, each followed by
`cargo +1.99.0 test --release -p esp32 --lib`. All twelve build and are killed by an
assertion failure (no panics, no compile errors); the restored tree passes.

| Mutation | Killing tests (`timers::tests::` unless named) |
| --- | --- |
| Line gated by INT_ENA (main) | `alarm_lines_…`, `int_st_is_raw_masked_by_int_ena` |
| Line = INT_RAW \| INT_ENA | both, plus `bus::…irq_events_timer_alarm_and_clear…`, `periph::…timer_edges_use_idf_sources…`, `periph::gpio_irq_tests::…restore_takeover…` |
| INT_ST reads INT_ENA | `int_st_is_raw_masked_by_int_ena` |
| INT_ST reads INT_RAW | `int_st_is_raw_masked_by_int_ena` |
| INT_ENA write dropped | `int_st_is_raw_masked_by_int_ena` |
| INT_CLR ignored | both, plus 4 `bus`/`periph` tests |
| Only T0 follows raw | both |
| LACT line still gated by INT_ENA | both |
| LACT edge line on bit 3 | `alarm_lines_…` |
| T1 level line on bit 0 | both |
| T0/T1 EDGE and LEVEL enables swapped | both, plus 4 `bus`/`periph` tests |
| Edge line only with LEVEL_INT_EN set | `alarm_lines_…`, `periph::…timer_edges_use_idf_sources…` |

## Equal work vs main

Native release CLIs of main `2e36224d` and this branch, built with
`nice -n 10 cargo +1.99.0 build --release --bins`, ROM `esp32_rev300_rom.elf`
(esp-rom-elfs 20260528). Whole output minus the `wall =` line is identical on all seven
fixtures (instruction counts, console, exception and interrupt totals):

| Workload | Emulated s | Instructions core0 + core1 (main = PR) | Interrupts | Console SHA256 |
| --- | ---: | ---: | ---: | --- |
| ESP-IDF v6.0.1 `examples/wifi-station`, WPA2 + DHCP (`--wifi`) | 15 | 63,580,464 + 4,138,320 | 4,512 | `edc5583ce048…` |
| Arduino-ESP32 starter sketch, T-Display | 10 | 20,696,614 + 33,290,613 | 29,952 | `4814f70b205a…` |
| Arduino-ESP32 busy sketch, T-Display | 10 | 20,354,465 + 2,388,671,719 | 20,151 | `073fbc7fb1a7…` |
| Arduino-ESP32 T-Display sketch | 10 | 20,915,056 + 2,244,908,078 | 19,989 | `848bf07c04ac…` |
| Arduino-ESP32 polling sketch | 10 | 20,416,858 + 2,279,258,183 | 20,642 | `c9bc5b1165b1…` |
| ESP-IDF classic hello (`web/wasm/fw/public`) | 10 | 5,545,092 + 1,499,305 | 2,004 | `2fac558f7e4e…` |
| Arduino-ESP32 3.3.11 WPA2 station + 10 Hz gptimer ISR (`--wifi`) | 12 | 77,994,063 + 15,579,757 | 25,271 | `0fbe99741bdb…` |

App image sha256: wifi-station `ac34c47d…`, starter `78232b5a…`, busy `e1e900fb…`,
T-Display `a9517b90…`, polling `fe25884c…`, hello `fa4307fe…`, gptimer `37d2f68c…`. The
four Arduino T-Display/polling sketches are not published; they are identified by hash only.
No Arduino-ESP32 2.x (IDF 4.4) Wi-Fi image was run.

Behaviour change, esp-hal 1.1.2: the `sta_scan` example of
[esp32-open-mac/FoA](https://github.com/esp32-open-mac/FoA) @ `2b7ec134` (merged image sha256
`2786fc81…`, not committed), 4 s without `--wifi`, so only its TIMG alarm can wake the
executor. Main: 2 interrupts, 9,268,546 instructions; the scan sets channel 1 twice and the
dwell timer never fires. This branch: 59 interrupts, 9,802,757 instructions, 21 channel
changes (1, 1, 6, 11, 2, 7, 12, 3, 8, 13, 4, 9, 5, 10, 1, …).

## Codegen vs main

aarch64, rustc 1.99.0, `esp32sim` binary; every symbol of the `esp32` crate disassembled,
addresses, branch targets, literal-pool pages and alignment `nop`s normalized: 301 of 303
identical. The two that differ are the two callers of the inlined `irq_bits`:

| Symbol | main | PR | Difference |
| --- | ---: | ---: | --- |
| `SocBus::tick` (with `Peripherals::tick` inlined) | 2,259 | 2,261 | Each of the 4 TIMG `irq_bits` evaluations (before/after tick, 2 groups) drops one `ldr` (INT_ENA) and one `and`. The block after the T1 test is tail-duplicated, so the unchanged `ClassicTimer::tick` call (`mov w8; add x0; mov x1; bl`) appears twice per site; each path still makes exactly one call. |
| `Dispatch::source_status` | 288 | 284 | Two `ldr` (INT_ENA) and two `and` fewer, one per group. |

No new fields, calls or per-tick work.

## CPU comparison

Pending (not measured on a quiet machine yet).

## Limits

T1 level, edge lines and LACT extrapolate the T0 observation and esp-hal; they were not
measured on silicon. QEMU gates timer lines with INT_ENA. IDF 5.x `timer_disable_intr()`,
`timer_group_intr_disable()` and `gptimer_register_event_callbacks()` with `on_alarm` NULL
can mask an armed alarm with INT_ENA alone while leaving the ISR installed. The ISR sees
INT_ST = 0 and does not clear INT_RAW; the level line remains high. The maintainer observed
the legacy T0 case hang on silicon and in this emulator; the classic TIMG watchdog interrupt
is not modelled, so the emulator spins rather than reproducing the hardware watchdog reset.
The equal-work fixtures above do not cover these masking paths. Classic ESP32 only; S2 is not
emulated, and the S3/C3/C6/P4 `TimerGroup` keeps
INT_ENA gating, which matches their esp-hal path (same function, lines 526-535: it writes
`int_ena` on every other chip).
