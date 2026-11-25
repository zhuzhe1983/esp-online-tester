# Cardputer-Adv battery voltage, 2026-11-25

`6b600f8ecb5a1fe7d7cea599dcf26ad3059030b5` adds a cold board default through the existing `BoardModel::default_analog_inputs` hook: an explicitly virtual3.7V battery supplies1.85V to GPIO10. The existing host `AnalogInputs` voltage/raw/stream source takes precedence, including zero, through device reattachment and MCU reboot. No register, per-tick path, trait method or public ABI is added.

## Hardware and SOC boundary

The [official pin map](https://docs.m5stack.com/en/core/Cardputer-Adv) assigns battery sensing to GPIO10. [M5Unified96576a14 Power_Class.inl](https://github.com/m5stack/M5Unified/blob/96576a14247ddc935149145aa6b51943464120f1/src/utility/Power_Class.inl#L982) selects ADC1, GPIO10 and a battery/pad ratio of2. Its ADC SOC estimate is the integer-clamped `(battery_mV -3300)*100/800`. A separate fuel gauge's SOC need not match this estimate. [M5's battery documentation](https://docs.m5stack.com/en/arduino/m5cardputer/battery) states that Cardputer/Adv cannot read battery charging status or current.

Hosts inject pad volts, not battery volts or percentage. For example, a4,027,000µV battery measurement becomes2.0135V on GPIO10:

```rust
bus.analog_set(10, esp_periph::AnalogSource::Const(battery_microvolts as f32 / 2_000_000.0));
```

The existing S3 ADC1 curve targets integer2013mV; a nearest-code tie selects raw2175, calibrated2012mV and a doubled4024mV reading. This small quantization is retained. It is not corrected by changing the real input voltage to force a desired SOC. No discharge, charger, current, brownout, analog supply or physical calibration model is claimed.

## Actual firmware and controls

The unchanged Wordle merged Flash and S3 ROM hashes are in `verification.json`. At2guest seconds, real guest ADC transactions complete once:

| Input | ADC raw | Actual firmware display | Per-core instructions |
| --- | ---: | --- | --- |
| Virtual nominal3.7V |1997|50%, yellow|8,189,700 /424,129,942|
| Explicit4.027V host battery |2175|90%, green|8,189,784 /424,129,942|

[Nominal pixels](nominal.png) and [host-voltage pixels](host-voltage.png) are the emulator's actual PNGs, inspected visually. Both retain the same guest console hash. Different input voltages intentionally change firmware work and pixels; they are not pooled into a performance comparison. At0.125guest seconds the firmware has not sampled GPIO10, so that earlier endpoint cannot establish battery behavior.

A frozen baseline helper executable, production-equivalent to5492cc4c, with the same explicit2.0135V input has exactly the candidate's per-core work,480,000,000cycles, exceptions, interrupts, ADC result, guest console and real PNG. The new default therefore does not overwrite host input. Full hashes and binary provenance are retained in the JSON. Firmware decides when to sample and refresh; a host update alone is not proof that the displayed icon has refreshed.

Reproduce after building both revisions with the same stable toolchain; supply the exact input files:

```sh
printf '0 adc 10 2.0135\n' > output/battery.script
$BIN --boot rom --rom "$ROM" --board cardputer-adv --flash-mb 8 --psram-mb 0 \
  --flash-image "$FLASH" --net none --max-seconds 2 --no-dump --adc 10 \
  --script output/battery.script --tft-png output/battery.png
```

## Checks and limitations

Four tests in `esp32s3/tests/cardputer_adv_battery.rs` exercise SENS ADC1_CH9, completed-conversion generations, pre-attachment voltage overrides, raw values including zero, shared streams and reboot. A guest program writes attenuation, pulses START and reads DATA/DONE; on AArch64 its JIT variant asserts actual compilation. Register bits cite [ESP-IDFv5.4.2 sens_reg.h](https://github.com/espressif/esp-idf/blob/v5.4.2/components/soc/esp32s3/register/soc/sens_reg.h).

Full release workspace tests pass with ignored tests included and only `external_` skipped:2,914 main passes plus1 isolated cache-child pass, not2,915 unique tests. Both required Clippy commands pass with warnings denied. The actual production WASM passes the shared-memory handoff,13 firmware manifests and four required platform test files. No golden was regenerated. The first workspace attempt correctly failed because ignored P4 hosted artifacts were absent; after fetching the documented assets the entire command passed.

Native clocks, quantum64, VQ1 and BB1 remain unchanged. These are correctness checks, not CPU measurements. No device was accessed. Firmware SOC estimates, host fuel-gauge SOC and charging/current must retain their separate meanings.
