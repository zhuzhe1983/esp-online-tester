# StamPLC internal protocol fixture

This is independently built ESP-IDF5.5.1 diagnostic firmware, not an official factory image. It uses the real ESP-IDF I2C, SPI, RMT and GPIO APIs and boots through the S3 ROM. Build artifacts belong outside Git and the public site.

With IDF5.5.1 activated:

```sh
idf.py -C tools/fixtures/stamplc-firmware -B /tmp/stamplc-build -D SDKCONFIG=/tmp/stamplc-sdkconfig reconfigure
ninja -C /tmp/stamplc-build -j3
node tools/probe-stamplc.mjs --build /tmp/stamplc-build
```

The probe uses the platform's real worker and locally built WASM/ROM assets. It checks shared G3 reset, PI4IO0x43/AW9523B0x59 over SDA13/SCL15, absent address0x44, wrong SDA2/SCL1 and SPI CS10, backlight gating and every pixel in a four-quadrant display. The fourth relay driver is read back before interactive input. Three fresh A/B/C cycles then produce distinct guest-selected RGB, framebuffer and relay GPIO states, with IRQ14 assertion/read-clear. Real G21 RMT data is gated by G38 supply; supply restoration requires a fresh frame. Pause and platform reset are also checked.

Physical relay contacts/loads, external optocoupler stimuli, LCD reads, analog brightness/delays, sensors, RTC, SD, industrial buses, sound, wireless and physical power behavior are outside the fixture. The protocol source and retained evidence are described in [the board documentation](../../../docs/platform/stamplc-runtime.md).

An additional bounded observer accepts a caller-supplied official merged image and validates its hash before running it:

```sh
node tools/probe-stamplc-factory.mjs <merged-firmware-file> <sha256> <official-https-source-url>
```

It records actual boot, frames, fresh A/B/C input and guest serial without assigning whole-application compatibility. No third-party firmware binary is stored in the repository.
