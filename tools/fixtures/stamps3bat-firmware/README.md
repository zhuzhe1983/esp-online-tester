# Stamp-S3Bat internal protocol fixture

This source-built ESP-IDF 5.5.1 application is independent diagnostic firmware, not an official factory image. Keep build artifacts outside Git and the public website.

With IDF 5.5.1 activated, configure an external build and SDK configuration, then limit the compile jobs:

```sh
idf.py -C tools/fixtures/stamps3bat-firmware -B /tmp/stamps3bat-build -D SDKCONFIG=/tmp/stamps3bat-sdkconfig reconfigure
ninja -C /tmp/stamps3bat-build -j3
node tools/probe-stamps3bat.mjs --build /tmp/stamps3bat-build --product m5stack-s015
```

The fixture boots through the real S3 ROM and initializes/tests 8 MB Octal PSRAM. It uses IDF I2C APIs on SDA48/SCL47 to observe PM1 identity and default power, probes an absent address and an incorrect SDA3/SCL4 route, and writes the manufacturer's GPIO0/NeoPixel protocol. Staged observations distinguish data-buffer writes from refresh commands and separate LED power loss from a fresh frame after power restoration. Real RMT21 traffic must have no visible effect.

Before polling PWR, it explicitly disables PM1 automatic single-click reset and double-click shutdown. Fresh physical-button status and the read-clear pressed flag then drive guest-selected red/green/blue/black output. The probe checks release, pause, reset and absence of a framebuffer. PMIC autonomous events, analog power, charging, wireless and real-hardware validation are outside this fixture.

Protocol source: [M5PM1 pinned source](https://github.com/m5stack/M5PM1/tree/315d2899c863ae34b59197392b9f4c59490751ef) and the [official S3Bat RGB tutorial](https://docs.m5stack.com/en/arduino/m5stamp_s3bat/rgb_led). Exact source receipts are in `docs/platform/evidence/stamps3bat-runtime/sources.json`.
