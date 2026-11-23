# M5Stack StickC-Plus K016-P

K016-P uses the classic ESP32-PICO-D4 with 4 MiB Flash, the ST7789 135 × 240 LCD and an AXP192 PMIC. Board aliases are `stickc-plus` and `m5stack-k016-p`. The original factory v1.0 at its catalog-pinned URL boots from the complete classic ESP32 ROM and merged Flash image. The product remains partial and the firmware remains trial. [Product knowledge](../../web/platform/knowledge/m5stack-k016-p.json) · [Factory image and ROM digests](evidence/stick-k016-p/factory-inputs.json)

| Device | Modeled wiring and behavior |
| --- | --- |
| LCD | MOSI G15, SCLK G13, CS G5, D/C G23, reset G18. AXP192 LDO2 drives the backlight and LDO3 supplies the controller. Losing LDO3 erases modeled GRAM. |
| Buttons | Active-low A/G37 and B/G39. Power goes through the AXP192 PEK short-event latch. The 3D A/B press regions use photo-estimated positions and sizes. |
| Internal I²C | G21 SDA/G22 SCL, AXP192 at 0x34, BM8563 at 0x51 and MPU6886 at 0x68. Both hardware controllers and bit-banged I²C share the physical devices. |
| LED and buzzer | Active-low red LED on G10 and guest LEDC output on G2. The native board capture is 16 kHz PCM. |
| Microphone | SPM1423 connected on CLK G0/DATA G34. Native stimuli enter at the converted PCM boundary of the classic I²S PDM receiver and reach the guest through DMA. Browser microphone input stays disabled. |

The AXP192 fixture starts with a virtual 4.1 V battery. The browser scene can declare its initial voltage and VBUS state as `environment.power`, and running guests can receive a `power-environment` input. ADC latches use guest sample time. At 3.0 V the v1.0 guest rendered `WARNING22`. Replays preserve the configured and queued environment values and carry `bitExact: false`. The model does not emulate battery chemistry, charge current or full PEK shutdown behavior. [Datasheet and driver receipts](evidence/stick-k016-p/sources-20261122.json)

## Factory results

The 2026-11-22 headless trial reached the factory IMU page without input. With unit gravity `[0.5, 0.5, sqrt(0.5)]` and `[-0.5, 0.25, sqrt(0.6875)]`, the guest displayed about −30.00°/35.27° and 30.00°/16.78° respectively. Restoring `[0, 0, 1]` returned both values to zero. Keyboard and 3D presses reached the RTC and microphone pages. The RTC display changed after another guest second and the silent FFT pixel hash stayed black while guest frame numbers increased. Bounded holds and waits exercised the IR Send and BLE disconnect pages and returned to IMU. An earlier short input left the guest in the microphone page.

The native trial produced two distinct factory FFT displays with 1 kHz and 2 kHz PCM on the verified route. Silence and DATA G35 left the spectrum black while the guest continued drawing. The factory contains SDK `v3.2.3-14-gd3e562907` and used the virtual receiver rate of about 44.1 kHz. The tests cover the guest conversion/DMA path and menu navigation. Raw PDM filtering, microphone acoustics, optical IR, BLE wireless/GATT, the separate boot-time TestMode and hardware reproduction remain unverified. [Measured checkpoints and source snapshot](evidence/stick-k016-p/factory-verification-20261122.json)

## Reproduce

Compare the caller-supplied firmware and ROM with the pinned SHA-256 values before running the probe. Store captures under an ignored output directory. The native probe writes only framebuffer captures and compact observations. Compare the `fft` framebuffer hashes for `tone1k`, `tone2k`, `wrong-pin` and `silence`. The `low-battery` mode captures the startup warning separately.

```sh
cargo test -p esp32 --lib board::stickc_plus
cargo test -p esp32 --test stickc_plus_power_input --test stickc_plus_temperature_input
node --test tools/platform-stickc-plus-test.mjs
shasum -a 256 "$PLUS_FACTORY_BIN" "$CLASSIC_ROM_ELF"
cargo run --release -p esp32 --example stickc_plus_pages_probe -- \
  "$PLUS_FACTORY_BIN" "$CLASSIC_ROM_ELF" target/stickc-plus-native/tone1k tone1k
```

Build WASM and install the classic ROM as described in the platform setup guide. With a task-owned headless Playwright session on the workbench, run the browser probe for initial framebuffer, IMU, keyboard A/B, RTC progression and silent FFT refresh. The probe loads the catalog-pinned factory from its source. Close the task session afterward. [Platform setup](README.md#local-development) · [Browser probe](../../tools/probe-stickc-plus-browser.js)
