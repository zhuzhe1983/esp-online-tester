# Internal U001-D sensor fixture

Build using **ESP-IDF v5.5.1**:

```sh
idf.py -C tools/fixtures/unit-env4-firmware -B "$PWD/output/unit-env4-build" -D SDKCONFIG="$PWD/output/unit-env4-build/sdkconfig" set-target esp32s3 build
node tools/probe-unit-env4.mjs --build output/unit-env4-build
```

Use the matching ESP-IDF Python environment when sourcing its export script. Binaries stay in ignored `output/`; this fixture is not a public firmware catalog entry.

I2C0 G47/G48 controls StopWatch M5PM1. I2C1 G10/G11 addresses U001-D SHT40 0x44 and BMP280 0x76. Readout goes through real guest registers, CRC and calibration. `main/upstream/unit_BMP280_data.hpp` is copied byte-for-byte from M5Unit-ENV `28c521a137cb6696f4860cdce93ca7df9ce9a6af/src/unit/unit_BMP280_data.hpp`; MIT license is adjacent. The guest uses this independent manufacturer compensation code.

UART0: `q` sample; `p` address probe; `o/i` power off/on; `w/c` swapped/correct SDA/SCL; `a` wrong addresses; `t` SHT40 early/ready/read-twice; `x` sensor reset; `r` guest restart. Waits use two FreeRTOS ticks: one tick does not ensure a full 8.2ms interval when a call starts late in the tick. Early-read testing intentionally omits the wait.

This measures implemented protocol behavior, not physical accuracy or compatibility of an arbitrary published firmware. Source receipts are in the U001-D evidence directory; the probe prints firmware, ROM, WASM and implementation hashes.
