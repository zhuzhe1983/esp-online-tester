# StopWatch M5 I2C devices

`esp_periph::i2c::m5` provides `M5pm1` at `0x6e` and `M5ioe1` at `0x4f`.
Both wrap `Reg8Device`: one-byte register pointers, repeated-start reads and
auto-incrementing transfers. They ACK immediately, so the firmware's wake retry
loop succeeds on its first attempt. No sleep delay or watchdog countdown runs.

The original access sequence followed private StopWatch revision `85c991d`.
The public [StopWatch UserDemo V0.5 source](https://github.com/m5stack/M5StopWatch-UserDemo/tree/6b4aa125288b6fe9dca661f10159f6e1e5ee785c/main/hal)
and [M5IOE1 1.0.8](https://github.com/m5stack/M5IOE1/tree/37db04861687858b6748f5dbd1a84439a6635c46)
now reproduce the power sequence and GPIO input-read polling used by the
released official binary. Public hardware sources follow.
Register definitions come from M5Stack, not Espressif/ESP-IDF:

- [M5PM1 Chip User Manual v1.9](https://m5stack-doc.oss-cn-shenzhen.aliyuncs.com/1207/M5PM1_Datasheet_EN.pdf),
  register map pp. 4–10 and descriptions pp. 12–18, 23.
- [M5IOE1 Chip User Manual](https://m5stack-doc.oss-cn-shenzhen.aliyuncs.com/1210/IO_Expander_Datasheet_EN.pdf),
  sections III and IV, especially direction, output and drive-mode registers.

## StopWatch address and wiring sources

The generic M5IOE1 manual's section V, table 6 describes strap-selected
addresses `0x6f`–`0x76`, not the StopWatch's address. M5Stack's
[StopWatch hardware documentation](https://docs.m5stack.com/en/core/StopWatch)
explicitly lists `M5IOE1(0x4F)` on SDA47/SCL48. Its M5IOE1 table assigns
PYG8 to the OLED rail, PYG5 to OLED reset and PYG4 to touch reset. The same
page's KEY and AMOLED tables give button A/B on GPIO2/1 and TE on GPIO38.
The [M5IOE1 library header at 846eec7](https://github.com/m5stack/M5IOE1/blob/846eec7d05e25c09013be2acdb8804487f48a62e/src/M5IOE1.h)
also defines `M5IOE1_DEFAULT_ADDR_2` as `0x4f` for some slave products,
separately from the generic `0x6f` default. `M5ioe1::ADDRESS` is the
StopWatch attachment address, not a universal chip address.

M5Stack's [power-management guide](https://docs.m5stack.com/en/arduino/stopwatch/m5pm1_m5ioe1)
documents the same expander pin assignments. The hardware page's M5PM1
table identifies PYG2 as `CHG_STAT`; the
[StopWatch schematic](https://m5stack-doc.oss-cn-shenzhen.aliyuncs.com/1242/C152-SCH_Stopwatch_PRJ_Main_VA_20251201_2026_04_24_17_46_22.pdf)
connects the charger status output to `PMG2_CHG_STAT`. The model supplies
that input as active-low host-controlled charging status, not an analog
charger simulation.

## PMIC

The ID reads `50 20` (little-endian `0x2050`). `PWR_CFG` (`0x06`) starts
at `0x17`, including LED enable bit 4; the firmware can clear that bit without
losing the other power bits. I2C config (`0x09`), watchdog (`0x0a`), GPIO
function (`0x16`), GPIO mode (`0x10`), hold config (`0x07`) and button config
(`0x49`, default `0x2a`) retain writes.

The board retains an `Arc<Mutex<M5pm1State>>` when it boxes the device for I2C.
`battery_mv` supplies `VBAT_L/H` (`0x22/23`) in millivolts and `charging`
supplies the active-low GPIO2 bit at `0x12`. The default 3900 mV, not charging,
is synthetic host input, not a battery discharge or charger model. Zero volts
lets the firmware report an invalid battery reading.

Writing `SYS_CMD` (`0x0c`) with `0xa1` latches `power_off_requested` in that
shared state. Clearing LED enable and hold config alone does not request
shutdown. The board can observe the latch after attachment and stop execution;
the chip does not halt the CPU or remove itself from I2C. Restart/download
commands, hold-dependent analog rails and button events are not simulated.
Constructing a new device clears the shutdown latch and restores register
defaults, matching expander reattachment. Host-provided battery voltage and
charging status survive reattachment.

## Expander

The constructor takes a synthetic UID supplied by the board, returned
little-endian at `0x00/01`. This is a per-device UID, not a fixed chip ID.
Direction (`0x03`), output (`0x05`), drive (`0x13`, default `0xff`) and I2C
config (`0x23`) retain writes. Identity registers are read-only on both chips.
GPIO input registers (`0x07/08`) are also read-only and reflect driven
push-pull output levels from direction/output/drive in both banks; the high
bank masks unused bits 6/7. In particular, the official HAL waits until
`digitalRead(IO8)` observes the enabled OLED rail. Inputs and released
open-drain pins remain low because external levels are not modeled.

`Arc<Mutex<M5ioe1State>>` exposes three StopWatch signals after each write:

| State field | Expander pin | Low-byte bit | Meaning when high |
| --- | --- | --- | --- |
| `oled_rail` | IO8 | 7 | OLED rail enabled |
| `oled_reset_high` | IO5 | 4 | OLED reset released |
| `touch_reset_high` | IO4 | 3 | Touch reset released |

A signal is high only if its pin is an output, its latch is high and its
driver is push-pull. Input and released open-drain pins are conservatively
treated as low; external pulls and electrical levels are not simulated.
The firmware explicitly selects push-pull before raising these pins.

The board can hold the OLED off unless `oled_rail && oled_reset_high` and
restore panel state when reset goes low. Reset levels are published at each
write, not coalesced at STOP, so the firmware's high → low → high pulse is
observable. The firmware owns the 10/8/20 ms delays; these devices have no
independent clock.

## Verification and remaining assembly

`cargo test --release -p esp-periph --test m5_i2c` covers the firmware's
read-modify-write masks with preserved unrelated bits, wake IDs, battery
reads, the reset pulse with uninterrupted rail power and shutdown ordering.

Unmodeled registers are zero-initialized register RAM, not implementations of
ADC conversion, PWM, IRQs or timers. The [`stopwatch` board](boards.md#stopwatch--m5stack-stopwatch-esp32-s3)
attaches these devices on bus 0 and retains both state handles for panel
power/reset gating and shutdown handling. BMI270 and RX8130 are not modeled.
