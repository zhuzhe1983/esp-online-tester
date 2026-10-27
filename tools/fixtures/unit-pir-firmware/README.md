# Unit PIR U004 internal GPIO fixture

This source-only ESP-IDF 5.5.1 program uses StopWatch M5PM1 internal I²C G47/G48, genuine GPIO11 input and GPIO10 pull-up. It is not factory firmware and must not be added to the public firmware catalog. Build output belongs under ignored `output/`.

```sh
# Activate ESP-IDF 5.5.1 first. Resolve -B from repository root.
idf.py -C tools/fixtures/unit-pir-firmware -B "$PWD/output/unit-pir-fixture" build
tools/wasm-build.sh
node tools/probe-unit-pir.mjs --build output/unit-pir-fixture
```

The guest prints `U004_READ G11=… G10=… US=…` from gpio_get_level and esp_timer_get_time. UART0 commands: `q` sample; `o/i` Grove power off/on; `u/d` G11 pull-up/down; `h/n` Grove hold on/off; `s` PMIC shutdown; `r` esp_restart. None of these changes host motion state or writes a synthetic GPIO result.

API motion means sustained infrared variation, not static presence. The named `pir-u004-m5-2s` profile follows M5 nominal 2 s retrigger behavior; AS312 mirrored manuals' 2.3 s on/lock table is retained as an unresolved source mismatch. This fixture verifies the selected virtual contract, not physical AS312 calibration. G10 must remain pulled up across motion and connection changes because U004 yellow is NC.
