# StampFly protocol fixture

Internal ESP-IDF 5.5.1 source fixture for StampFly K138 and StampFly v1.1
K138-V11. It is deliberately absent from public firmware catalogs. Generated
files belong in ignored `output/` or a temporary directory, never Pages assets.

After activating ESP-IDF 5.5.1:

```sh
idf.py -C tools/fixtures/stampfly-firmware -B "$PWD/output/stampfly-build" -D SDKCONFIG="$PWD/output/stampfly-sdkconfig" build
tools/wasm-build.sh
node tools/probe-stampfly.mjs --build output/stampfly-build
```

The probe defaults to both products, independently. `--product m5stack-k138-v11`
selects one; `--no-jit` selects the interpreter. It uses the actual platform
worker and WASM with the S3 ROM, local firmware loading and isolated memory
storage. It reports artifact hashes and fails if fresh guest output disagrees
with an input. It never supplies a replacement firmware response.

The source uses IDF GPIO, RMT, LEDC, SPI and I2C drivers. GPIO0 press/release
cycles the module RGB LED through red, green, blue and off. GPIO39 drives two
separate carrier LEDs. Four LEDC channels drive FL/G5, FR/G42, RL/G10 and RR/G41
at 150 kHz with distinct 8-bit duties, including a zero-duty step. At startup,
GPIO38 remains low for one second and then high for one second without another
RMT frame. This distinguishes the original Stamp-S3 LED from the v1.1
Stamp-S3A supply gate; a new frame finally lights the v1.1 LED.

I2C3/4 reads INA3221 address 0x40 identity and channel-2 bus voltage, and checks
that address 0x41 NACKs. SPI2 uses CLK44/MOSI14/MISO43/CS46 to read BMI270,
soft-reset it, upload the complete Bosch configuration and read acceleration
and angular velocity. The same read on CS12 must return open-bus 0xff.
Explicit host virtual IMU inputs are checked against fresh guest register
reads; the 3D-to-sensor mounting transform is not calibrated. Pause and reset
checks cover halted guest output, cleared angular velocity and released G0.

Routing sources are the [official StampFly documentation](https://docs.m5stack.com/en/app/StampFly),
StampFly schematic v1.0, Stamp-S3A schematic v0.3.3 and
[M5StampFly commit 2b96752c72c465c63b260996046c71c7675f81b2](https://github.com/m5stack/M5StampFly/tree/2b96752c72c465c63b260996046c71c7675f81b2).
ESP-IDF 5.5.1 drivers supply SoC register definitions. The unchanged 8192-byte
Bosch array is reused from `../atoms3r-firmware/main/bmi270_config.h`, retaining
its BSD-3-Clause notice and BMI270_SensorAPI source revision
`41129fcfe39c583ee5462d79195741945d51c1fe`. The simulator checks the complete
upload but does not execute the sensor's embedded program.

This fixture demonstrates only the tested board subset. It does not establish
official factory firmware compatibility, flight dynamics, calibrated thrust
or RPM, physical power behavior, radio-controller operation, or hardware sensor
timing. It does not exercise VL53L3CX, BMP280, BMM150 or PMW3901. No framebuffer
is expected on either product.
