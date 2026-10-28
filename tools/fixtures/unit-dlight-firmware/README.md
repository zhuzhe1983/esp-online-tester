# Internal U136 BH1750 fixture

Build with **ESP-IDF v5.5.1** and its matching Python environment:

```sh
idf.py -C tools/fixtures/unit-dlight-firmware -B "$PWD/output/unit-dlight-build" -D SDKCONFIG="$PWD/output/unit-dlight-build/sdkconfig" set-target esp32s3 build
node tools/probe-unit-dlight.mjs --build output/unit-dlight-build
```

Binaries remain in ignored output and are not catalog firmware. I2C0 G47/G48 controls StopWatch M5PM1; I2C1 G10/G11 reads U136 at0x23. `main/upstream/M5_DLight.cpp/.h` are unchanged from `m5stack/M5-DLight@4dd8d2bd862abce5e995a2e7efea2a691baa8c75`; the adjacent MIT license is retained. Minimal Arduino/Wire headers adapt to real ESP-IDF I2C operations, never mock sensor responses.

The vendor `getLUX()` divides raw by1.2 regardless of MTreg/H2. The fixture prints that value separately from a datasheet-compensated result; the difference is intentional and tested.

UART0 commands: `q` sample; `p` probe0x23; `a` probe0x5c; `o/i` Grove power off/on; `w/c` wrong G14/G15/correct G10/G11; `h/2/l` one-shot H1/H2/L; `m/n` MT138/MT31; `g/H/L` continuous H1/H2/L; `d` PowerDown; `z` PowerOn+reset; `t` early/late conversion reads; `e` ENV SHT40/BMP280 coexistence; `r` guest restart. UART and USB mirrors can duplicate aggregate console lines; timing assertions compare actual values, not line counts.

This validates modeled protocol behavior, not individual hardware accuracy or arbitrary published firmware compatibility. Source receipts and real worker/WASM results are in the U136 evidence directory.
