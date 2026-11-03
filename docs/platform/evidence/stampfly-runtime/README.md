# StampFly internal firmware evidence

`fixture.json` records separate passing runs for K138 and K138-V11 through the
real platform worker/WASM, full ESP32-S3 ROM and source-built ESP-IDF 5.5.1
bootloader/application. The tested source revision, firmware/WASM hashes and
selected observed values are retained in the receipt. This is a board-subset
test, not official factory firmware certification or a CPU benchmark.

Reproduce using the commands in
[`tools/fixtures/stampfly-firmware/README.md`](../../../../tools/fixtures/stampfly-firmware/README.md).
The probe runs both products independently and uses isolated memory storage.

Both runs passed fresh GPIO0 press/release, three RMT RGB outputs, four
independent routed LEDC outputs (150093.808630394 Hz, including zero duty),
INA3221 identity/bus-voltage reads, wrong I2C address and wrong SPI CS,
BMI270 complete configuration upload and explicit virtual IMU inputs, pause,
resume and reset. The original module LED remains powered with G38 low;
v1.1 stays dark until G38 is high and another RMT frame arrives. Neither
product produces a framebuffer. IMU axes are explicit virtual axes; they do
not claim a calibrated 3D mounting transform.

The generated full receipt is retained outside Git at
`output/stampfly-runtime/fixture-full.json` in the integration checkout
(15784 bytes, SHA-256
`bedf432d43832f224bb351e0b9d29248c36a97d18c608ef8fc30eb1523252de8`).
The committed compact receipt replaces repeated protocol lines and duplicate
IMU samples with their numeric results. No personal data was present, and no
test outcome, firmware hash or measured value was changed. Re-running the
probe regenerates a full receipt; timestamps and selected observation times
may differ.

Generated binaries and the ESP-IDF build log remain outside Git. The firmware
source reuses the licensed Bosch configuration array already tracked under
`tools/fixtures/atoms3r-firmware/main/`. VL53L3CX is covered by its separate
driver harness; this fixture does not exercise it, BMP280, BMM150 or PMW3901.
Flight dynamics, physical power behavior, calibrated motor RPM/thrust, radio
control and physical sensor timing remain outside these results.
