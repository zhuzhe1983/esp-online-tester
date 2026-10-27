# AtomS3 protocol fixture

Internal ESP-IDF 5.5.1 fixture for the C123 GC9107 variant. It drives the documented
SPI and I2C pins, paints a red/green screen, polls GPIO41 and prints MPU6886 samples.
It is not catalog firmware; generated files belong in ignored `output/`, never in
the public website.

```sh
idf.py -C tools/fixtures/atoms3-firmware -B "$PWD/output/atoms3-build" -D SDKCONFIG="$PWD/output/atoms3-sdkconfig" build
tools/wasm-build.sh
node tools/probe-atoms3.mjs --build output/atoms3-build
```

The probe uses the real browser worker/WASM protocol, checks exact guest pixels,
then observes guest serial output after physical button and sensor inputs. These
checks do not certify arbitrary firmware or hardware timing.
