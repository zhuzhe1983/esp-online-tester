# StampFly runtime

`stampfly` / `m5stack-k138` and `stampfly-v11` / `m5stack-k138-v11` are separate boards. This is a digital peripheral model, without flight dynamics or a radio controller. An official image reaching a sensor driver is a separate acceptance check; board registration and unit tests do not establish that the factory application finishes booting.

## Wiring and output

The [official M5StampFly source at 2b96752c72c465c63b260996046c71c7675f81b2](https://github.com/m5stack/M5StampFly/tree/2b96752c72c465c63b260996046c71c7675f81b2) supplies the carrier routes:

| Device | Physical route | Implemented subset |
| --- | --- | --- |
| BMI270 | SPI CLK44, MOSI14, MISO43, CS46 | Register reads/writes, address and dummy phase, complete configuration upload, power/range and ideal samples |
| INA3221 | SDA3/SCL4, address `0x40` | Identity/configuration, signed shunt and bus conversion registers, enabled channels, single/continuous conversion deadlines and CVRF |
| Bottom/front VL53L3CX | SDA3/SCL4; XSHUT7/9, IRQ6/8 | Separate dynamic addresses, boot/NVM/patch sequence, timed raw histograms and latched GPIO interrupts |
| Module RGB | G21, one GRB WS2812 | Completed RMT frames; v1.1 requires actively driven G38 supply enable |
| Carrier RGB | G39, two GRB WS2812 | Completed RMT frames |
| User button | G0 | Active-low GPIO input |
| Motors FL/FR/RL/RR | G5/G42/G10/G41 | Cold observations of actual routed PWM frequency and duty |

The Stamp-S3A [v0.3.3 schematic](https://m5stack-doc.oss-cn-shenzhen.aliyuncs.com/1150/Sch_StampS3_v0.3.3.pdf) connects G38 to AW35122 EN, supplying the module LED through BL_3V3. Removing that drive clears the module LED; carrier LEDs have a separate supply. Original Stamp-S3 has no such gate. The v1.1 source receipt is in its product knowledge.

`motion_snapshot()` returns `type:"stampfly-motors"`, `model:"latched-pwm"`, `flightSimulated:false`, `clockHz:240000000`, monotonic `cycle` and a change `version`. Its four motor entries use the IDs `front-left`, `front-right`, `rear-left`, `rear-right`, the GPIOs above, normalized duty and `frequencyHz`. Unrouted or disabled PWM becomes zero. Host observation never writes a motor command. Guest reset preserves the board time and resets the SoC PWM registers; the next observation clears disabled outputs. A new board starts at zero.

## Sensor inputs and limits

BMI270 shares the existing [polled register subset](stopwatch-imu.md). SPI reads return a dummy byte after the address. Completing all 8192 configuration bytes is required; the embedded Bosch feature program is not executed or checked for a particular hash. `set_imu_sample` supplies finite sensor-frame vectors without advancing time. The initial ideal sample is `[0,0,1]` g with zero angular velocity. This is an explicit virtual sensor input, not a closed flight loop or a claim about physical mounting alignment. FIFO, ODR/filter timing, BMI interrupts, feature engine and auxiliary magnetometer remain absent.

INA3221 samples explicit ideal bus/shunt voltages at the completion of a configured channel cycle. Default virtual channel 2 is 4.0 V with zero shunt drop; other channels are zero. The model includes reset values, read-only IDs/data, channel masks, conversion/averaging duration, powerdown retention, 40 μs recovery and conversion-ready clearing. It does not model a battery, motor load, averaging noise, shunt summation or alert pins. Input changes affect the next completed conversion. Calibration and physical battery behavior are not implied.

VL53L3CX supplies raw histograms to the original ST host driver. Both XSHUT lines have 4.7 kΩ pullups, so both devices initially respond at `0x29`; firmware must isolate and renumber the front sensor. The board attaches independent devices with actual dynamic address matching, physical pin pairs and IRQ edges at their scheduled completion cycles. Default optical input is **no target**, not an invented floor distance. The sensor model's optional ideal one-target stimulus and uncalibrated nominal NVM are documented and tested separately. No factory calibration or optical accuracy is claimed.

BMP280, BMM150 and PMW3901 are not modeled. The current official source does not read them, but requires VL53L3CX interrupts before proceeding beyond sensor initialization. Buzzer audio and ESP-NOW control have not been verified.

## Sources and focused checks

- BMI SPI framing: [Bosch BMI270 datasheet](https://www.bosch-sensortec.com/media/boschsensortec/downloads/datasheets/bst-bmi270-ds000.pdf), §6.4, and official `lib/bmi270/common.c`. Existing register semantics cite BST-BMI270-DS000-08 in the model.
- INA registers and timing: [TI SBOS576C](https://www.ti.com/lit/ds/symlink/ina3221.pdf), §§7.4–7.5 and 8.2. Fetched `2026-11-03T19:58:16Z`, 2,117,980 bytes, SHA-256 `d622c93ffe599b246fd3c6285996f219786235b635a30cca512229919a5f4ebc`; HTTP Last-Modified `2026-10-24T06:08:30Z`. Revision C retains the digital interface used by the factory library; no analog accuracy claim is made.
- Official source archive SHA-256 `00835b000aa203f9f7a5071a1596f79dbccb771820cd3c16d85e96058200811c`, 1,375,427 bytes. The binary factory reports ESP-IDF v4.4.5; the board adds no SoC registers or interrupt sources.

Focused commands:

```sh
cargo test --release -p esp-periph --lib i2c::bmi270
cargo test --release -p esp-periph --lib i2c::ina3221
cargo test --release -p esp-periph --lib i2c::vl53l3cx
cargo test --release -p esp32s3 --lib stampfly
cargo clippy -p esp-periph -p esp32s3 --all-targets -- -D warnings
```

Tests exercise dummy-byte alignment, incomplete configuration, register scaling, conversion boundaries, protected registers, powerdown/reset, wrong physical SPI routes, LED supply isolation and motor output clearing. Full factory boot and real-browser evidence are tracked separately; no remote firmware binary is committed.
