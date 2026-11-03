# StampFly native factory acceptance

Both complete, unchanged official factory images boot from the ESP32-S3 ROM to `Finish StampFly init!` / `Enjoy Flight!`. Both respond to a 150 ms G0 press at guest second 5 with `RTC_SW_CPU_RESET` at 6.231 seconds, then complete initialization again. [factory.json](factory.json) records exact inputs, hashes, instruction counts, observed markers, sensor statuses and output counters.

| Check | Original K138 / v1.1.0 firmware | K138-V11 / official v1.0.0 payload |
| --- | --- | --- |
| Five-second boot | Both ToF initializations, BMI270 config and ten initial ToF IRQ reads complete | Same |
| Five-second BMI SPI transactions | 8,237 | 8,237 |
| Five-second visible RGB updates | 6 | 3; Stamp-S3A module RGB is unpowered without G38 |
| G0 input | Press 5.000 s, release 5.150 s | Same |
| Guest response | Software reset 6.231 s, second successful initialization | Same |
| Twelve-second motor output | All four GPIOs 149812.734 Hz, 0% duty | Same |

The zero-duty motor outputs are meaningful disarmed firmware output, not evidence of flight. No transmitter was paired and no radio packets were injected. The no-target optical input produces ten initial zero ranges; IMU and battery inputs are the documented ideal defaults. The native run does not establish browser/WASM behavior, acoustic output, radio control or physical stability. The real core reset defect and its preserved negative results are documented by **EX248**, not worked around in these images.

## Reproduce

Build the CLI from the runtime plus the S3 coprocessor-reset fix. Set `SIM` to that binary, `ROM` to the official S3 revision-0 ROM ELF, and `FW_ORIGINAL` / `FW_V11` to local temporary copies of the two complete images pinned in the receipt. Verify their SHA-256 values before running. These files are not redistributed here.

```sh
cargo build --release -p esp32sim --bin esp32sim
"$SIM" --rom "$ROM" --board stampfly --flash-image "$FW_ORIGINAL" --flash-mb 8 --psram-mb 0 --boot rom --max-seconds 5 --console both --no-dump
"$SIM" --rom "$ROM" --board stampfly-v11 --flash-image "$FW_V11" --flash-mb 8 --psram-mb 0 --boot rom --max-seconds 5 --console both --no-dump
"$SIM" --rom "$ROM" --board stampfly --flash-image "$FW_ORIGINAL" --flash-mb 8 --psram-mb 0 --boot rom --max-seconds 12 --console both --no-dump --script docs/platform/evidence/stampfly-runtime/button.script --pwm 5 --pwm 42 --pwm 10 --pwm 41
"$SIM" --rom "$ROM" --board stampfly-v11 --flash-image "$FW_V11" --flash-mb 8 --psram-mb 0 --boot rom --max-seconds 12 --console both --no-dump --script docs/platform/evidence/stampfly-runtime/button.script --pwm 5 --pwm 42 --pwm 10 --pwm 41
```

The input receipt uses actual script-driven GPIO edges. Acceptance requires the following **guest outputs**, not just those input lines: successful sensor-init statuses, `Enjoy Flight!`, a later `RTC_SW_CPU_RESET`, and a second `Enjoy Flight!`. The PWM report must show the four expected physical GPIOs, a running timer and zero duty. The original and v1.1 products use distinct image hashes.

## Retention and limits

The receipt keeps curated observations rather than full console logs. Raw captures remain temporary local artifacts; raw and sanitized hashes are recorded. Sanitization replaces the personal home-directory label and emulator MAC label. Neither change affects the tested sensor, input, reset or PWM observations. Reproduction does not require either identifier. No firmware, ROM or full console capture is committed.

These are correctness checks. Wall-clock values printed by the CLI are retained as incidental measurements; compilation overlapped some runs, and they must not be presented as a CPU benchmark. Source-backed peripheral tests additionally passed: 45 I2C unit tests, four StampFly board tests, and native Clippy for `esp-periph` / `esp32s3` with warnings denied. The unchanged ST driver has its own [histogram-protocol evidence](../../stampfly-tof.md).
