# StampFly UART Demo 0.1.0

A separately authored, publicly reviewable firmware for **ESP Online Tester**. It runs real ESP32-S3 instructions, receives UART0 commands, reads BMI270 over SPI and bottom VL53L3CX over I²C, and writes four LEDC outputs. It is not M5Stack factory firmware. The original and v1.1 factory selections remain the defaults. No physical-flight validation or flashing is claimed.

One merged image supports K138 (`stampfly`) and K138-V11 (`stampfly-v11`), 8 MiB Flash / no PSRAM. It drives G38 high before G21 RMT status output so the v1.1 Stamp-S3A LED is powered; original Stamp-S3 does not require that gate. Motor order is **FL/G5, FR/G42, RL/G10, RR/G41**. LEDC is 150 kHz, 8 bit: requested duty is rounded to `duty × 256`, capped at 255; `1` therefore produces 255/256, and STOP produces exact zero. The RGB status is STOP red, MOTOR blue, AUTO green, LAND amber. Carrier LEDs are not used.

## Build from source

Activate an existing **ESP-IDF v5.5.1** installation (Xtensa GCC `esp-14.2.0_20241119`, esptool 4.12.0). Supply the public M5StampFly checkout at `2b96752c72c465c63b260996046c71c7675f81b2`. `check-sources.py` verifies all 76 ST `.c/.h` files by their aggregate SHA-256 before compilation. The ST algorithm remains unmodified; only its OS/I²C transport is replaced by the IDF transport in `main/tof_platform.c`. No driver or firmware is downloaded by the build script.

```sh
. /path/to/esp-idf-v5.5.1/export.sh
firmware/stampfly-uart-demo/build.sh /absolute/private/build /absolute/M5StampFly
```

The output `stampfly-uart-demo.bin` contains bootloader at 0, partition table at 0x8000 and application at 0x10000. It uses normal S3 ROM boot, no ROM patches or application-direct shortcut. Build configuration and intermediate files stay in the caller's private directory; concurrency is limited to three Ninja jobs. `CONFIG_APP_REPRODUCIBLE_BUILD` and explicit project version remove build timestamps/version guesses. Exact rebuild comparison belongs in the release receipt; this option alone is not proof of byte reproducibility.

The Bosch configuration in `vendor/bmi270_config.h` is the unchanged 8192-byte array from BMI270_SensorAPI `41129fcfe39c583ee5462d79195741945d51c1fe`, retaining its BSD license. `LICENSES.txt` accompanies public binaries and covers this project, Bosch, ST and the linked IDF/runtime dependencies.

## UART protocol

Send ASCII lines to **UART0**, ending in LF. A complete line is at most 159 bytes excluding LF; long lines are discarded through the next LF. The browser UART API permits at most 128 bytes per input; longer test lines must be deliberately fragmented. Normal commands are much shorter. UART0 is the simulator's logical ingress; physical pin wiring and baud timing are not validated.

| Command | Meaning |
| --- | --- |
| `SF1 STOP` | Immediately write all four motor duties to zero; clear controller state. |
| `SF1 MOTOR fl fr rl rr` | Four independent finite duty values, each 0–1. No automatic timeout or sensor substitution. |
| `SF1 TAKEOFF height_m` | From STOP, request 0.10–1.50 m above the model's ground reference. Requires fresh IMU and initial roll/pitch within 15°. |
| `SF1 TARGET height_m roll_deg pitch_deg yaw_rate_dps` | While AUTO, update height 0.10–1.50 m, roll/pitch ±10°, yaw rate ±45°/s. No horizontal position hold. |
| `SF1 LAND` | Lower height setpoint at 0.20 m/s, then write zero PWM. An unmeasured launch is stopped immediately. |

Boot emits `SF1 READY stampfly-uart-demo 0.1.0` with motors zero. Accepted lines emit `SF1 ACK STOP`, `MOTOR`, `TAKEOFF`, `LAND` or `TARGET`. Rejected lines emit `SF1 ERR FORMAT`, `RANGE`, `NOT_READY`, `MODE` or `LINE_TOO_LONG` and leave the prior command/output unchanged. NaN, infinities, overflow, missing/extra arguments and unknown commands are rejected. A valid STOP is required to stop an existing MOTOR command; an invalid command does not reinterpret itself as a new motor command.

Every 200 ms, the guest reports:

```text
SF1 STATE mode uptime_ms height_m roll_deg pitch_deg yaw_rate_dps fl fr rl rr imu_valid tof_valid
```

The last four floats are requested duties; independently observed peripheral outputs include actual 8-bit quantization. `SF1 SENSOR` reports actual SPI read count, processed ST measurement count, raw ToF status and range; logs are not used to animate motors. `SF1 SENSOR_INIT BMI270=1 VL53L3CX=1 code=0` confirms initialization only.

## Sensor/control boundaries

BMI270 uses CLK44/MOSI14/MISO43/CS46, 8g and 2000°/s ranges. Sensor raw axes become FRD body axes `x=rawY, y=rawX, z=-rawZ`; static specific force is body z = −1g. Gyroscope integration with a small accelerometer correction estimates roll/pitch. Only the bottom VL53L3CX is enabled (XSHUT7, I²C3/4, address0x29); front XSHUT9 remains low. ST MEDIUM mode and 33000 µs timing budget retain the original histogram history. A processed object must have status0; no object is never interpreted as a valid zero distance.

Control nominally runs at 200 Hz of guest time. ToF readiness is polled every5 ms, and its original history may delay rising ranges by about360 ms. Height uses `range × cos(roll) × cos(pitch) − 0.035 m`; the mounting offset is an explicitly approximate dynamics parameter, not hardware calibration. Valid measurements expire after350 ms, IMU after50 ms. A controller interval over100 ms, excessive tilt (50°) or lost established feedback stops motor output.

When the optional dynamics scene returns no near-ground ToF target, TAKEOFF has a bounded two-second acquisition phase: stabilize attitude and ramp collective from0.50 toward0.66. It must acquire a real valid ST result before continuing. No valid distance or contact sensor is invented. Once acquired, altitude uses conservative PI/velocity feedback around the dynamics model's nominal hover duty0.612459, with a 0.30 m/s rising setpoint. Initial release qualification of the motor path is separate from closed-loop flight qualification; consult the final evidence for the accepted modes and numeric behavior.

The four-motor mixer uses positive FRD roll/pitch/yaw conventions: roll increases FL+RL, pitch increases FL+FR, yaw increases FR+RL. Its feedforward is for the platform's 4.0V/37g idealized motor model and is not a calibrated real-aircraft controller. BMP280, BMM150, PMW3901, ESP-NOW pairing, battery dynamics, obstacle avoidance and horizontal position control are absent.

Loading this firmware does not silently enable physics. The user must explicitly enable `configureFlight({enabled:true})` or the supported portable `scene.flight` configuration in the platform. With physics disabled it remains a real PWM/propeller demonstration. Pause freezes guest execution; platform reset boots again with all motors zero.

## Verification

```sh
cc -std=c11 -D_POSIX_C_SOURCE=200809L -Wall -Wextra -Werror \
  -Ifirmware/stampfly-uart-demo/main firmware/stampfly-uart-demo/main/control.c \
  firmware/stampfly-uart-demo/tests/control_test.c -lm -o /tmp/stampfly-control-test
/tmp/stampfly-control-test
node tools/probe-stampfly-demo.mjs --firmware /private/build/stampfly-uart-demo.bin --assets-root /built/repository
```

The C test covers parsing, mixer signs and control failure boundaries; it is not execution evidence. The Node probe uses actual worker/WASM plus S3 ROM on both SKUs and checks fresh UART input against routed PWM, invalid commands, pause, STOP and reset. Browser rendering and enabled rigid-body flight require their separate checks. Never replace the public demonstration's identity with the older internal protocol fixture.
