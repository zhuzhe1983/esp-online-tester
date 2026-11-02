# StopWatch platform diagnostic

A real ESP32-S3 firmware fixture for the M5Stack StopWatch model. It uses ESP-IDF v5.5.1 drivers to initialize M5PM1/M5IOE1 over I2C, draw RGB565 digits through CO5300 QSPI, read GPIO buttons and CST820 touch, and receive NMEA lines on UART1. The browser never draws the timer itself.

- A / GPIO2: start or pause the timer; serial `BUTTON_A running` / `BUTTON_A paused`.
- B / GPIO1: reset the timer; serial `BUTTON_B reset`.
- Touch: serial `TOUCH down`.
- UART1 (TX10 / RX11, configured 9600 8N1): echo each received line as `GPS_RX …`; the lower digit counts received lines modulo 10.
- Ready signal: `PLATFORM_READY stopwatch`.

This is a platform diagnostic, not M5Stack's official firmware. It has not been tested on physical hardware. The initial emulator GPS adapter injects bytes directly into UART1, bypassing physical pin routing and baud timing; this fixture is not evidence of electrical compatibility. It counts NMEA lines, not validated satellite fixes. It omits BLE, Wi-Fi, RTC, IMU and battery behavior.

## Initialization in 1.0.1

The earlier fixture copied its startup sequence from emulator tests: the host used 400 kHz while PM1 was configured for 100 kHz, the power LED was disabled, and PM1's manual-download lock was set. Version 1.0.1 uses manufacturer register definitions instead of treating model acceptance as evidence of correct hardware initialization.

- I2C uses SDA47/SCL48 at 100 kHz. A dummy wake read may fail; a 10 ms wait follows it. Transfers have a 500 us processing interval and up to three attempts, with a 50 ms wait after failures. Initial PM1 identification gets one further wake sequence after an 800 ms wait. Final errors still fail visibly with the address/register in the log.
- PM1 `0x09=0` keeps the bus at 100 kHz without idle sleep; `0x0a=0` disables its watchdog. `0x06 |= 0x17` enables LED_EN, LDO, DCDC and charging while preserving other bits. The diagnostic never accesses button configuration `0x49` and does not alter charging-current GPIOs. It does not attempt to clear a lock left by another application.
- IOE names are one-based: P1 selects the rear UART/USB mux, P3 enables audio, P4/P5 reset touch/OLED, P8 supplies L3B, and P9/P10 control motor/speaker. Selected pins use push-pull outputs with no pull resistors. The low bank ends at masked value `0x9c` and the high bank at masked value zero. The diagnostic holds the two reset signals low for 20 ms and waits 80 ms after release; these waits are fixture choices, not measured hardware timing limits.
- CO5300 initialization includes sleep-out with a 150 ms wait, `C4=80`, `35=80`, `44=01 D2`, `53=20`, inversion off, rotation zero, RGB565, brightness `A0` and display-on. The visible window remains 468 by 466 with GRAM x offset 6. Several power/display settings changed together; this does not isolate one missing command as the only possible cause of a blank physical screen.

The intended button, touch, timer and UART behavior is unchanged. This version still needs its own runtime checks; earlier fixture results do not certify this binary or any physical device. The diagnostic stays outside the public firmware catalog and Pages artifact.

### Manufacturer references

Pinned downloads, retrieval timestamps, byte sizes and SHA-256 values are recorded in [manufacturer-sources.json](manufacturer-sources.json).

- [M5PM1 driver](https://github.com/m5stack/M5PM1/blob/315d2899c863ae34b59197392b9f4c59490751ef/src/M5PM1.cpp) and [I2C compatibility layer](https://github.com/m5stack/M5PM1/blob/315d2899c863ae34b59197392b9f4c59490751ef/src/M5PM1_i2c_compat.h): default speed, wake, retry and transfer-processing waits.
- [M5PM1 register definitions](https://github.com/m5stack/M5PM1/blob/315d2899c863ae34b59197392b9f4c59490751ef/src/M5PM1.h): power control and download-lock bits.
- [M5IOE1 register definitions](https://github.com/m5stack/M5IOE1/blob/8401119bf171cb8f906fbe22e7490989077bbee8/src/M5IOE1.h) and [driver](https://github.com/m5stack/M5IOE1/blob/8401119bf171cb8f906fbe22e7490989077bbee8/src/M5IOE1.cpp): low/high GPIO banks, output direction, pulls and drive modes.
- [StopWatch IOE initialization](https://github.com/m5stack/M5StopWatch-UserDemo/blob/81099c784991dc77f522de1a5bf62ec5de9ae0b5/main/hal/hal_ioe.cpp) and [CO5300 initialization](https://github.com/m5stack/M5StopWatch-UserDemo/blob/81099c784991dc77f522de1a5bf62ec5de9ae0b5/main/hal/hal_display.cpp): board pin roles, output levels and panel commands. The factory application turns the separate power LED off while using its display; this diagnostic deliberately leaves it on.

## Rebuild

With ESP-IDF v5.5.1 activated:

```sh
idf.py -C examples/platform-stopwatch build
python3 tools/package-stopwatch-demo.py
node tools/wasm-test.mjs demo-stopwatch
```

`sdkconfig.defaults` selects S3, 16 MiB flash and a single core. The board runtime provisions 8 MiB PSRAM; the diagnostic itself uses internal RAM. `CONFIG_APP_REPRODUCIBLE_BUILD` and `PROJECT_VER=1.0.1` exclude machine paths and timestamps from the application descriptor. Compiled artifacts and their hashes are in `web/wasm/fw/public/platform-stopwatch*`; the ROM is fetched separately. The package script also updates the internal fixture's manifest hash and version in `tools/fixtures/platform-stopwatch.json`.

Application source is MIT, like this repository. ESP-IDF runtime code is Apache-2.0; see the bundled notice and license.
