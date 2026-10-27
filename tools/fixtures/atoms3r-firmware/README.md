# AtomS3R protocol fixture

Internal ESP-IDF 5.5.1 fixture for the C126 ST7735 batch. It drives the documented
SPI and I2C pins, reads the panel ID, paints a red/green screen, polls GPIO41,
cycles LP5562 white PWM through 0/128/255 and prints BMI270 samples.
It checks PICO package identity, initializes 8 MiB Octal PSRAM and reads/writes an
extra 4 KiB allocation after ESP-IDF's startup memory test.
It is not catalog firmware; generated files belong in ignored `output/`, never in
the public website.

```sh
idf.py -C tools/fixtures/atoms3r-firmware -B "$PWD/output/atoms3r-build" -D SDKCONFIG="$PWD/output/atoms3r-sdkconfig" build
tools/wasm-build.sh
node tools/probe-atoms3r.mjs --build output/atoms3r-build
```

The probe uses the real browser worker/WASM protocol, checks exact guest pixels,
then observes guest serial output after physical button and sensor inputs. These
checks do not certify arbitrary firmware or hardware timing. The Bosch 8192-byte
configuration source array is unchanged from BMI270_SensorAPI commit
41129fcfe39c583ee5462d79195741945d51c1fe/bmi270.c; its BSD-3-Clause notice remains
in main/bmi270_config.h. The simulator requires a complete transfer but does not
execute or validate the sensor's embedded configuration program.
