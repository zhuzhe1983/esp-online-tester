# M5Stack PM1 and IOE1 I2C devices

`esp_periph::i2c::m5` provides `M5pm1` at `0x6e` and `M5ioe1` at `0x4f`.
Both use one-byte register pointers, repeated-start reads and auto-incrementing
transfers. PM1 stores registers with its shared input/event state; IOE1 wraps
`Reg8Device`. They ACK immediately, so the firmware's I2C wake retry loop succeeds
on its first attempt. No bus sleep delay or watchdog countdown runs.
StopWatch exposes [hardware checks](platform/hardware-diagnostics.md) for
nominal master/device rate mismatches, download-lock requests and LED state;
these observations do not turn this register subset into a wake timing model.

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
`I2C_CFG` bit4 selects the 100/400 kHz slave setting. Cold address-phase
observation records matching nominal master rates and retains mismatches.
Unknown/GPIO-bitbang clocks do not produce a fabricated rate. DL_LOCK writes
and clear attempts are observed without claiming physical unlock behavior.

The board retains an `Arc<Mutex<M5pm1State>>` when it boxes the device for I2C.
`battery_mv` supplies `VBAT_L/H` (`0x22/23`) in millivolts and `charging`
supplies the active-low GPIO2 bit at `0x12`. The default 3900 mV, not charging,
is synthetic host input, not a battery discharge or charger model. Zero volts
lets the firmware report an invalid battery reading.
Explicit host `vref_mv`, `vin_mv`, and `ext5v_mv` supply the read-only
little-endian pairs at `0x20/21`, `0x24/25`, and `0x26/27`. They default to
zero to preserve existing boards; a board can declare its nominal virtual USB
supply independently. These values survive PMIC construction/reset along with
the battery input and do not imply analog ADC or charging simulation.

Writing `SYS_CMD` (`0x0c`) with `0xa1` latches `power_off_requested` in that
shared state. Clearing LED enable and hold config alone does not request
shutdown. The board can observe the latch after attachment and stop execution;
the chip does not halt the CPU or remove itself from I2C. The digital DCDC rail turns off; the LDO, Grove boost and ordinary GPIO outputs
follow their hold bits. Analog voltage transitions are not simulated. PMIC
restart/download commands and power-button events are described below.
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
`digitalRead(IO8)` observes the enabled OLED rail. Board-supplied `input_mask`/`input_high` levels are visible on input pins. Other
inputs and released open-drain pins retain a conservative low value; no analog
pull or contention model is implied.

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
ADC conversion, PM1 PWM, RTC clocks or timers. IOE1 PWM and PM1 GPIO IRQ behavior
are described below. The [`stopwatch` board](boards.md#stopwatch--m5stack-stopwatch-esp32-s3)
attaches shared devices to both controllers, gated by physical SDA47/SCL48,
and to the GPIO bit-banged bus used by M5GFX detection. Both state handles
remain shared for panel power/reset gating and shutdown handling. BMI270 has
a virtual-input register subset; RX8130 at `0x32` has register RAM only,
without calendar progression, alarms or battery-domain retention. Absent
addresses still NACK. See the [M5Guruguru boot investigation](platform/stopwatch-burner-startup.md).

## CoreMatrix PM1 digital control

The CoreMatrix pin assignment is [documented by M5Stack](https://docs.m5stack.com/en/core/CoreMatrix):
KEY1/KEY2/KEY3 reach PM1 GPIO0/1/2; PM1 GPIO3 reaches ESP32-C61 GPIO2;
PM1 GPIO4 receives the IMU interrupt. PM1 `PWR_CFG[1]` controls the
`3V3_L3A` rail. The [official power guide](https://docs.m5stack.com/en/arduino/corematrix/m5pm1_m5ioe1)
configures GPIO2 as an ordinary input and GPIO3 as the IRQ output.

`M5pm1State::set_input(pin, high)` supplies a physical digital level immediately.
Direct legacy writes to `input_high` are sampled at the next addressed transfer.
`release_input(pin)` removes that external driver and resolves GPIO pull-up/down
from registers `0x14/15`. An unpulled floating input reads conservatively low
but is unknown for edge detection; only known-to-known changes can wake or
interrupt. This prevents an unconnected BMI270 INT1/R37 NC from inventing a
falling edge. External drive overrides a pull; GPIO output direction overrides
the input level. The legacy StopWatch charging-status fallback on GPIO2 remains.
When at least one GPIO has function `01` (IRQ output), changes on ordinary
input-mode GPIOs latch in `IRQ_STATUS1` (`0x40`). Output pins, alternate
functions, invalid pin numbers and the IRQ-output pins do not generate these
input events. Both rising and falling edges latch. An IRQ mask suppresses
output without discarding its pending event. `irq_pins` identifies the assigned
IRQ pins and `irq_low` reports the active-low aggregate after all three masks.
Removing every IRQ function clears all three status registers, as the manual
requires. Board code routes this signal to the connected SoC GPIO.

`WAKE_SRC` (`0x05`) and `IRQ_STATUS1..3` (`0x40..42`) are **write-zero-to-clear**.
Writing ones preserves pending bits and cannot fabricate an event. A retained
wake source reasserts `IRQ_STATUS3[1]` until firmware clears the source and then
the status. The library's `CLEAN_ONCE` implementation writes the complement of
the observed status; it is not a write-one-to-clear register.

The manual v1.9 (pp. 21–23, 28–29) is authoritative for IRQ direction/scanning.
The driver's header comment says an IRQ-function GPIO causes an edge event,
but its ordinary-input/IRQ-output example and the manual describe the behavior
implemented here. The header also calls PWR_CFG bit1 a 5 V rail; the manual
and CoreMatrix wiring identify it as 3.3 V DCDC. Those comments are not adopted.
The manual inconsistently writes `0x2F`/`0x04` in wake-IRQ prose; the register
map and driver both use `WAKE_SRC=0x05`.

An enabled, matching GPIO wake edge while shut down or with DCDC disabled
requests a board wake and restores PMIC startup state. GPIO1 is excluded by
the manual. The shared interrupt pairs GPIO0/2 and GPIO3/4 are mutually
exclusive; enabling both pins in a pair is rejected for wake in this digital
model because the manual does not define which conflicting selection wins.
Other enabled, unambiguous pins remain active. GPIO wake enable/edge and RTC
RAM survive PMIC reset. GPIO direction/function/output and holds reset.

A command with the exact key `0xA2` requests reset; `0xA3` requests download
unless DL_LOCK is set. The board consumes `wake_requested`, `reset_requested`
and `download_requested` and must implement the corresponding SoC lifecycle;
a request alone is not proof the CPU rebooted or entered its downloader.
`M5pm1::new` / `with_power_config` is a **PMIC cold start**, which clears these
requests and the retained RTC RAM. CPU-only software reset must preserve the
existing PMIC device, not call this constructor again.

`power_button(pressed, now_ms)` supplies PWR_BTN and `advance_to_ms(now_ms)`
advances its digital event classifier using guest time. `next_deadline_ms()`
lets the board schedule the next pending click/long event, including CPU-off
intervals. `BTN_STATUS[0]` reads
the held state; bit7 records a press and clears on read. `BTN_CFG1` supplies
single/double/long intervals; `BTN_CFG2[0]` disables double-click shutdown.
With single reset disabled, a qualified click latches button IRQ bit0;
with double shutdown disabled, a double click latches bit2. Otherwise they
request reset or shutdown. A long press requests download only if unlocked.
A press while powered off wakes the device and is not also counted as a click.
The model uses release-to-next-press for the double-click gap and waits the
maximum of the single/double windows before a single event. It does not claim
PM1 firmware scan scheduling, contact debounce, analog thresholds or calibrated
press timing. No firmware output is synthesized from these button events.

IOE1 state now includes `pwm_frequency_hz` from little-endian registers
`0x25/26`. CoreMatrix's buzzer PWM is IO11/PWM3 (array index 2); its effective
duty still requires output direction, push-pull drive, PWM enable and a nonzero
frequency. The board also applies IO2 buzzer supply. Exposing these parameters
does not by itself claim audible output or measured loudness.

System voltage insertion/removal IRQ generation, battery discharge/charging,
PM1 watchdog/countdown wake, independent I2C sleep timing, PM1 NeoPixel/PWM and
analog supply transitions remain outside this device model. The board's IMU
interrupt source and SoC wake/download path require their own validation.

### Pinned sources and checks

The pinned source files below were fetched again at `2026-11-06T04:17:13.992435Z`
(header), `2026-11-06T04:17:14.547358Z` (driver), and
`2026-11-06T04:17:15.281377Z` (manual). All URLs are HTTPS and returned
without redirection; fetched byte lengths and SHA-256 are listed.
The header and source `main` resolved to commit
`315d2899c863ae34b59197392b9f4c59490751ef`; pinned links reproduce the bytes.
No private capture or machine identifier is retained.

| Source (original → pinned) | Media type | Bytes | SHA-256 |
| --- | --- | ---: | --- |
| [M5PM1.h main](https://raw.githubusercontent.com/m5stack/M5PM1/main/src/M5PM1.h) → [315d2899 header](https://raw.githubusercontent.com/m5stack/M5PM1/315d2899c863ae34b59197392b9f4c59490751ef/src/M5PM1.h) | text/plain | 135849 | `47a2d475ab687dc8104a6c7a571c91ff36c76d138a0be66c4f2f6894b20428e3` |
| [M5PM1.cpp main](https://raw.githubusercontent.com/m5stack/M5PM1/main/src/M5PM1.cpp) → [315d2899 driver](https://raw.githubusercontent.com/m5stack/M5PM1/315d2899c863ae34b59197392b9f4c59490751ef/src/M5PM1.cpp) | text/plain | 188068 | `52e68e1900305831da692b207e7e54859d5c14a428c030b6052c9fb3614c9b21` |
| [Manual v1.9, 315d2899](https://raw.githubusercontent.com/m5stack/M5PM1/315d2899c863ae34b59197392b9f4c59490751ef/docs/M5PM1_Datasheet_EN.pdf) | application/octet-stream | 828393 | `f008794c8a6b672b61452c77a9359b6332d82194dce92f9a21494e283783b656` |

`cargo test -p esp-periph --test m5_i2c` verifies the existing StopWatch/Paper
register contract plus CoreMatrix GPIO/IRQ edges, masks, write-zero-to-clear,
rail hold/shutdown, wake exclusions, reset retention, power-key events and
IO11 PWM frequency/duty gating. These are digital protocol tests, not real
hardware measurements or complete CoreMatrix firmware qualification.
