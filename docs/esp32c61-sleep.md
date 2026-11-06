# ESP32-C61 timed sleep

The digital implementation in `esp32c61/src/sleep.rs` follows ESP-IDF **v5.5.4**,
commit `735507283d5b2f9fb363a1901172dbd9e847945d`. It is separate from the external
CoreMatrix M5PM1 power controller: an ESP32 internal sleep request does not turn
off the board's PM1 rails or reset its I2C peripherals.

## Register contract

| Block | Address | Implemented sleep behavior |
| --- | --- | --- |
| PMU | `0x600b0000` | `+0x124` bit31 requests sleep; `+0x12c` bit4 enables timer wake. `+0x128` bit31 and source mask control immediate rejection. |
| PMU status | `+0x144/148` | Read-only wake/reject causes, timer bit4. `+0x134` bit31 clears rejection cause. |
| PMU HP interrupt | `+0x160/164/168/16c` | RAW/ST/ENA/W1C; wake bit31, reject bit30. CPU interrupt masking does not disable the PMU wake input. |
| LP_TIMER | `0x600b0c00` | Two 48-bit target registers at `+0/4` and `+8/c`; target-enable is a write trigger at high-word bit31. Comparator0 wakes the HP system. |
| LP_TIMER snapshots | `+0x10/14/18/1c/20` | UPDATE bit27 shifts current counter to buffer0 and previous buffer0 to buffer1; each high word is 16 bits. |
| LP_TIMER interrupt | `+0x28/2c/30/34` | HP RAW/ST/ENA/W1C; alarm bit31, 48-bit overflow bit30. Comparator1 has separate LP status at `+0x38..44`. |
| LP_AON | `0x600b1000` | STORE0..9 remain across CPU resets. STORE8 bit0 selects deep vs light wake in the pinned driver. |
| LP_CLKRST | `0x600b0400` | `+0x10` reset cause is hardware-owned; deep wake sets cause5, plus documented cause-clear and flag controls. |

The ROM header's legacy `TIMER_EXPIRE` enum is bit3, but C61's actual
`pmu_bit_defs.h` defines `PMU_LP_TIMER_WAKEUP_EN` as **bit4** and
`esp_pmu.h` maps `RTC_TIMER_TRIG_EN` to it. Original UIFlow writes `0x10`
to PMU wake-enable; bit3 is the Wi-Fi beacon wake source.

The ROM header's prose says STORE9 holds sleep mode, but its
`RTC_SLEEP_MODE_REG` macro and `lp_aon_ll_inform_wakeup_type` use **STORE8**.
The implementation follows the actual driver access. Sleep entry is a write
trigger, not a value that firmware can read back as a latched request.

The RTC counter uses the pinned C61 nominal **136000 Hz** RC_SLOW frequency,
with a rational accumulator against the 160 MHz simulation reference. This
preserves fractional progress across uneven execution batches. Nonzero slow-clock
selectors use a nominal 32768 Hz source; crystal/external-clock presence and
oscillator startup are outside this model. The calibration result uses the same
nominal frequencies, the 40 MHz XTAL, and C61's PCR `CTRL_32K_CONF` selection plus
`FOSC_TICK_NUM+1` for the nominal 17.5 MHz RC_FAST source. It does not reuse the
shared timer group's 150 kHz calibration value.

## Scheduling and retention

While sleeping the CPU must be `CoreState::Held`, preserving its PC, register
file and instruction count. The bus advances its global clock accounting and LP
state, while digital TIMG/SYSTIMER clocks stop; they must not catch up after wake.
The next LP alarm is reported in CPU-reference cycles. Board devices keep their
own clocks and PM1 state. A light timer wake releases the same CPU execution
state and latches wake status. A deep timer wake requests the emulator's ordinary
reset path with cause5; the caller reboots through the unmodified C61 ROM.
LP time, store registers and wake cause survive that digital reset.

This is the timer-wake subset. GPIO/ext1/UART/USB/BLE wake, analog power transitions,
REGDMA retention, CPU power-down wake stubs and detailed memory power-domain
retention are not modeled by this module. Unimplemented PMU setup registers are
backed by register storage. The 48-bit comparator treats past/now targets as due
using modular half-range ordering; the supported future interval is less than
2^47 slow ticks. Overflow and immediate rejection are deterministic digital
semantics, not analog timing claims. Generic interrupts cannot wake a sleeping
CPU unless an implemented PMU wake source is enabled.

## Checks

Seven release unit tests cover the exact 200 ms boundary, fractional accounting,
48-bit wrap/high word, independent wake and IRQ masks, read-only/W1C registers,
snapshot history, immediate rejection, reset5 and retained LP state. Release
Clippy with all targets and `-D warnings` passes in the standalone module harness.

The ignored external fixture `external_official_uiflow_timed_light_and_deep_sleep`
uses an unchanged UIFlow image. It checks CPU execution and SYSTIMER freeze during
`machine.lightsleep(200)`, elapsed guest time after returning, and timed
`machine.deepsleep(200)` followed by ROM reboot and the guest's
`machine.reset_cause() == machine.DEEPSLEEP_RESET` result. Run after SoC integration:

```sh
ESP32SIM_ROM_DIR=<rom-directory> ESP32SIM_COREMATRIX_FIRMWARE=<original-image> \
CARGO_BUILD_JOBS=2 cargo test --release -p esp32c61 --test sleep \
  external_official_uiflow_timed_light_and_deep_sleep -- --ignored --nocapture
```

## Source receipt

Sources retrieved 2026-11-06; hashes are SHA-256 of the original source bytes.

| Source | SHA-256 |
| --- | --- |
| [pmu_reg.h](https://github.com/espressif/esp-idf/blob/735507283d5b2f9fb363a1901172dbd9e847945d/components/soc/esp32c61/register/soc/pmu_reg.h) | `290e211625cd2318d825bfabdba004696a4a7eba46d1e66fe544044a4e6094b7` |
| [lp_timer_reg.h](https://github.com/espressif/esp-idf/blob/735507283d5b2f9fb363a1901172dbd9e847945d/components/soc/esp32c61/register/soc/lp_timer_reg.h) | `8faaca3f1fa7915438d6f4a7a99a08fe13ccac06efb21884118fd44af02b6fba` |
| [lp_aon_reg.h](https://github.com/espressif/esp-idf/blob/735507283d5b2f9fb363a1901172dbd9e847945d/components/soc/esp32c61/register/soc/lp_aon_reg.h) | `cb3fcbf6f46b8d213b1d50a8593d22a9d034ff221fa673231655c3579b66da66` |
| [lp_clkrst_reg.h](https://github.com/espressif/esp-idf/blob/735507283d5b2f9fb363a1901172dbd9e847945d/components/soc/esp32c61/register/soc/lp_clkrst_reg.h) | `0d5fca5161c20b7bbbd50f464bf267508f58d2a526888e31c5764324b623a3bb` |
| [pmu_ll.h](https://github.com/espressif/esp-idf/blob/735507283d5b2f9fb363a1901172dbd9e847945d/components/hal/esp32c61/include/hal/pmu_ll.h) | `8982ff25145a5b40344264714b18b76c7661b58426205c06a8660860b55d1b5e` |
| [lp_timer_hal.c](https://github.com/espressif/esp-idf/blob/735507283d5b2f9fb363a1901172dbd9e847945d/components/hal/lp_timer_hal.c) | `d2c87f42c61f828be64a54e56ab4abf1909b35d31afe7af2d8879a0e8f3990d5` |
| [pmu_sleep.c](https://github.com/espressif/esp-idf/blob/735507283d5b2f9fb363a1901172dbd9e847945d/components/esp_hw_support/port/esp32c61/pmu_sleep.c) | `593522475d01ffa948fb293528d9337f19bcff6a23ca2b2771741f00bd0383e6` |
| [rtc.h](https://github.com/espressif/esp-idf/blob/735507283d5b2f9fb363a1901172dbd9e847945d/components/esp_rom/esp32c61/include/esp32c61/rom/rtc.h) | `3a4ac558733be12853cd105c2c0b79af3cccdc761f7d81e87bbbd28179e2ff8d` |
| [clk_tree_defs.h](https://github.com/espressif/esp-idf/blob/735507283d5b2f9fb363a1901172dbd9e847945d/components/soc/esp32c61/include/soc/clk_tree_defs.h) | `c4f69a1a12f35167646c0a469b037fcc233129002eff9236d2650378025767e9` |
| [rtc_time.c](https://github.com/espressif/esp-idf/blob/735507283d5b2f9fb363a1901172dbd9e847945d/components/esp_hw_support/port/esp32c61/rtc_time.c) | `c4f269b3c52686682d42684fdb2f4f1ecf5df81c2852b098e5360f34c0f237d9` |
| [pmu_bit_defs.h](https://github.com/espressif/esp-idf/blob/735507283d5b2f9fb363a1901172dbd9e847945d/components/esp_hw_support/port/esp32c61/private_include/pmu_bit_defs.h) | `c7804ec266f82899b5e628a211a496b7c7e67ff5ac06d5380b1261e781c62ba7` |
| [esp_pmu.h](https://github.com/espressif/esp-idf/blob/735507283d5b2f9fb363a1901172dbd9e847945d/components/esp_hw_support/include/esp_private/esp_pmu.h) | `cfa4ec9dab6f88fa37e264058389fdecdf55a4a1744182d503694a6445055366` |
