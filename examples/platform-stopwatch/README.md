# StopWatch platform diagnostic

A real ESP32-S3 firmware fixture for the M5Stack StopWatch model. It uses ESP-IDF v5.5.1 drivers to initialize M5PM1/M5IOE1 over I2C, draw RGB565 digits through CO5300 QSPI, read GPIO buttons and CST820 touch, and receive NMEA lines on UART1. The browser never draws the timer itself.

- A / GPIO2: start or pause the timer; serial `BUTTON_A running` / `BUTTON_A paused`.
- B / GPIO1: reset the timer; serial `BUTTON_B reset`.
- Touch: serial `TOUCH down`.
- UART1 (TX10 / RX11, configured 9600 8N1): echo each received line as `GPS_RX …`; the lower digit counts received lines modulo 10.
- Ready signal: `PLATFORM_READY stopwatch`.

This is a platform diagnostic, not M5Stack's official firmware. It has not been tested on physical hardware. The initial emulator GPS adapter injects bytes directly into UART1, bypassing physical pin routing and baud timing; this fixture is not evidence of electrical compatibility. It counts NMEA lines, not validated satellite fixes. It omits BLE, Wi-Fi, RTC, IMU and battery behavior.

## Rebuild

With ESP-IDF v5.5.1 activated:

```sh
idf.py -C examples/platform-stopwatch build
python3 tools/package-stopwatch-demo.py
node tools/wasm-test.mjs demo-stopwatch
```

`sdkconfig.defaults` selects S3, 16 MiB flash and a single core. The board runtime provisions 8 MiB PSRAM; the diagnostic itself uses internal RAM. `CONFIG_APP_REPRODUCIBLE_BUILD` and `PROJECT_VER=1.0.0` exclude machine paths and timestamps from the application descriptor. Compiled artifacts and their hashes are in `web/wasm/fw/public/platform-stopwatch*`; the ROM is fetched separately.

Application source is MIT, like this repository. ESP-IDF runtime code is Apache-2.0; see the bundled notice and license.
