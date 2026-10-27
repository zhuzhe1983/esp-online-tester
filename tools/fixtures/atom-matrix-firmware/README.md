# Atom-Matrix internal GPIO/RMT/I2C fixture

Source-built ESP-IDF 5.5.1 protocol test, not manufacturer firmware. Generated binaries stay in ignored output and must not be published to the website or catalog.

With ESP-IDF 5.5.1 and target esp32 activated:

```sh
idf.py -C tools/fixtures/atom-matrix-firmware -B "$PWD/output/atom-matrix-fixture-build" build
node tools/probe-atom-matrix.mjs --build output/atom-matrix-fixture-build
```

C008-B official pin map / M5Atom ebbf7363: G39 active-low button, G27 GRB chain, MPU6886 at 0x68 on SDA25/SCL21. Fixture starts LED0 red/LED24 blue, pressed G39 turns LED0 green, release red. It reads actual MPU identity and acceleration via I2C; the probe rotates the scene quaternion and requires changed guest register values. This driver fixture does not prove the distributed factory binary's Wi-Fi scan or complete application works. No real hardware verification.
