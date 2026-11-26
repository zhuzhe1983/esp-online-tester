# Public input path: VAMeter K136 and Xiaozhi Card Kit K146

This closes the JavaScript input transport check for the separate digital board
adapters described in `docs/platform/vameter-xiaozhi-card-runtime.md`. The test
uses `createPlatformRuntime`, the production Worker and an existing production
WASM build. Original LX7 SRAM programs configure real GPIO routes and SPI3; the
K146 guest also reads the FT6336U contact-count register through bit-banged I²C
on SDA2/SCL1. Guest instructions make every input decision and pixel write.

| SKU / public input | Guest-observable result |
| --- | --- |
| K136 `button: encoder-press` down/up | Guest GPIO2 changes first pixel red → green → red |
| K136 `encoder: encoder`, one positive step | Guest GPIO3/GPIO1 observes four quadrature transitions; a separate pixel changes blue → yellow and stays changed after the complete cycle |
| K146 `touch` down/up | Guest reads FT register 2, writes the bottom-right EPD RAM bit and activates refresh; pixel (175,263) changes white → black → white after BUSY48 clears |

`verification.json` records the tested WASM/ROM and authored program hashes,
actual public input acknowledgments, guest UART output and framebuffer hashes.
Initial and released frames must match exactly; the changed frame must contain
the expected guest-produced pixel. UART messages are written by guest stores to
the UART peripheral; they are synchronization evidence, not a substitute for
the framebuffer assertions. App-direct mode has no execution stubs.

Reproduce from the repository root with caller-supplied existing artifacts:

```sh
ESP32SIM_WASM=/path/to/esp32sim.wasm \
ESP32SIM_ROM_DIR=/path/to/rom-directory \
M5_TWO_INPUT_RUNTIME_RECEIPT=docs/platform/evidence/vameter-card-public-input-20261126/verification.json \
node --test tools/platform-vameter-card-input-runtime-test.mjs
```

Result: both product subtests and the enclosing test pass (3 passed, 0 failed).
An earlier 5-second capture wait expired before the release frame was observed.
The final fixture explicitly enables the BUSY48 input and allows 15 seconds of
host time for a frame; this is a test wait bound, not a physical timing claim.
This change only adds diagnostics and evidence. It does not compile Rust, build
WASM, copy firmware, change board behavior or validate a manufacturer image.
Physical hardware, browser pointer/raycast behavior, optical/timing calibration,
INA226 measurements, power, audio, storage, radio, modem and dock acceptance
remain outside this check. Existing native tests retain route/reset negative
coverage. This is an authored digital diagnostic, not factory-firmware acceptance.
