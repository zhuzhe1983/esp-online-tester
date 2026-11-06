# CoreMatrix C158 digital peripherals

`esp32c61::board::corematrix::CoreMatrix` models the C158 v0.7 schematic's
physical I²C bus, power-gated TM1680 white matrix, three PM1 buttons,
PM1 IRQ/power events, BMI270 polled samples, SPI microSD and IOE1 PWM buzzer.
This document describes protocol tests; it is not proof that a particular
unmodified firmware boots. Firmware integration evidence is recorded separately.

## Sources and matrix layout

[Source receipts](evidence/corematrix-peripherals/sources.json) retain actual
retrieval URLs/times, media types, lengths and hashes. The driver is pinned to
[M5GFX 28d47949](https://github.com/m5stack/M5GFX/tree/28d47949bdc84f30881623476f555398e2194313),
`src/lgfx/v1/panel/Panel_TM1680.hpp` and `.inl`. Electrical routing comes from
[CoreMatrix v0.7 schematic](https://m5stack-doc.oss-cn-shenzhen.aliyuncs.com/1274/CoreMatrix_V0.7_SCH_PDF_20260609.pdf)
and the [LED board schematic](https://m5stack-doc.oss-cn-shenzhen.aliyuncs.com/1274/CoreMatrix_SCH_LED.pdf).
TM1680 commands and defaults are from Titan Micro's v1.1 datasheet,
pp. 5, 8–9, 17; the manufacturer-authored PDF is distributed by LCSC.

The source-backed RAM presentation is 16 rows × 16 columns. Each four-bit
RAM address selects four COM outputs in a ROW; the first wire byte carries
columns 0–3 in its high nibble and columns 4–7 in its low nibble. A full
frame is address `00` followed by 32 bytes. There is no SSD1306-style control
prefix. Read address phases NACK; RAM writes accept all byte values, including
bytes with bit 7 set. Consecutive commands are allowed before a RAM address.
The datasheet's page-write diagram labels the first transmitted nibble
inconsistently with its command summary; the implemented order follows the
summary and independently agrees with the manufacturer's M5GFX packer.

SYS enable, LED enable, RC clock selection, COM mode, 1/16–16/16 PWM and
0.5/1/2 Hz blinking affect actual stored pixel output. Blink uses guest cycles;
there is no JavaScript animation. External-clock modes have no clock source on
this board. Opposite-polarity COM modes do not drive the board's NMOS wiring.
Unsupported RAM addresses and write overruns are ignored; wrapping into LED0
is deliberately not invented. Deterministic dark RAM after a physical supply
loss is a simulator initialization choice; silicon power-on RAM is unspecified.
Sleep/LED-off retain RAM; a CPU-only reset retains the independently supplied
PM1, IOE1, TM1680, BMI270 and SD state.

Both `display()` (RGB565, 16×16) and `leds()` (256 white RGB intensities,
row-major) derive from the same TM1680 RAM/enable/brightness state. Brightness
is the average digital duty mapped to 0–255, not a calibrated optical luminance
or diffuser/glow measurement. The schematic establishes ROW/COM topology;
physical optical orientation still needs the product model/photo validation.

## Power, inputs and routing

All four devices attach at physical SDA0/SCL1 on I²C0: PM1 `0x6e`, IOE1
`0x4f`, BMI270 `0x68`, and TM1680 `0x72`. Hardware and GPIO bit-banged
transfers share the same device instances. A peripheral address cannot bypass
its physical route or power gate. PM1 survives L3A removal; IOE1 and the downstream LED/SD rails reset when
that supply is lost. BMI270 VDD/VDDIO are on the independent L1 LDO rail,
so clearing DCDC enable does not reset the IMU. Its INT1 pin4 connects to
PM1 GPIO4; INT2 only reaches test point TP10. LDO removal resets the sensor.

PM1 GPIO0/1/2 read A/B/C. GPIO3 IRQ drives ESP GPIO2 active-low. The semantic
power key calls PM1's actual digital button state machine; PM1 reset, wake and
download requests are exposed through optional board power hooks. The C61
SoC must consume those hooks to hold/restart the MCU and select its ROM boot
strap. The hooks alone do not prove those SoC effects. [PM1 protocol details](../m5-i2c.md)
describe status, masks, write-zero-to-clear, hold, wake sources and digital
button timing. The button model does not reproduce analog debounce.

IOE1 IO4 gates the matrix; IO1 gates SD; IO2 gates the buzzer; IO5 gates
Grove 5V. GPIO direction/output/push-pull must all agree before a rail rises.
IO8 reads a default not-charging input; IO10 is active-low card detect.
Charge-current selection and connector 5V state are digital observations,
not a charger, battery discharge, regulator or cable electrical model.

SPI SD uses SCK25/MOSI27/MISO26/CS28. Wrong routes/selects, removal or a
power-disabled socket cannot exchange commands. Power loss resets SPI startup
and partial commands while keeping the SD backing store. It reuses the shared
SDHC digital protocol and persistent storage path, including its documented
immediate-transfer, CRC and timing limitations; this is not native SDMMC.

The buzzer takes IOE1 PWM channel 3 (IO11), frequency registers `25/26`,
12-bit duty, enable and polarity. Captured mono PCM uses 48 kHz guest-time
sample centers and a digital square at ±8192. A disconnected/gated/constant
output produces silence. Capture draining preserves fractional sample time
and phase. There is no host-time stretch or speed change. This is a preview
of the electrical PWM, not a calibrated MLT-8530 acoustic or analog model.

BMI270 uses the existing Bosch register engine and complete 8192-byte
configuration-transfer check. Host acceleration/gyro samples pass through
power and range registers. FIFO, feature-engine execution and IMU wake
interrupts are outside this implementation; a successful WHOAMI/readout must
not be described as their validation.

## Reproduction

```sh
CARGO_BUILD_JOBS=2 cargo test -p esp-periph tm1680 --lib
CARGO_BUILD_JOBS=2 cargo test -p esp32c61 board::corematrix --lib
CARGO_BUILD_JOBS=2 cargo clippy -p esp-periph --all-targets -- -D warnings
```

The matrix tests include an independent one-hot walk through all 256 visible
positions, write-only/address behavior, partial writes, invalid overruns,
brightness, sleep, physical power loss and guest-clock blink. The board tests
check real I²C writes, firmware-visible button/status registers and IRQ edges,
shutdown/wake state, SPI routes/detect/power restart, PWM sample counts and
frequency transitions, and BMI270 initialization/range/power recovery.
No generated test bitmap substitutes for actual firmware validation.
