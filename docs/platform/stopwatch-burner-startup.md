# StopWatch Burner firmware startup

## M5Guruguru-Starter v4

The original public v4 image now reaches its QR-code setup screen in the native
interpreter and browser WASM. The old StopWatch board connected internal devices
only to I2C controller 0. M5GFX probes through GPIO SDA47/SCL48 before using a
hardware controller; this firmware also initializes controller 1 on those pins.
The missing route caused repeated `LGFX ... ack wait` failures and no display.

The board now shares the same PMIC, expander, touch and IMU devices between GPIO
I2C and both pin-gated hardware controllers. RX8130 at `0x32` supplies register
RAM for detection and settings. It does **not** advance time, raise alarms or
retain a battery-domain clock. Addresses without physical modeled devices still
NACK, including `0x50`; a failed optional probe is not itself a boot failure.

Sources checked 2026-10-27:

- [StopWatch pin map](https://docs.m5stack.com/en/core/StopWatch), internal SDA47/SCL48 and device addresses.
- [M5GFX detector at 65c816a](https://github.com/m5stack/M5GFX/blob/65c816a38386062d2c3dc47d41237a9083acbc75/src/M5GFX.cpp), StopWatch GPIO probe and controller handoff. This is a checked source reference, not a claim that the downloaded binary embeds this exact dependency revision.
- [M5Guruguru-Starter source](https://github.com/nananauno/M5Guruguru-Starter), Arduino setup, AP provisioning and avatar upload workflow.

The firmware remains the unmodified remote image; no replacement firmware or
guest patch is used. Its log reaches `AP Started!` and `After Setup End` and the
real framebuffer shows the setup QR code. The browser still has no SoftAP HTTP
bridge, so uploading avatars through that AP and the complete application
workflow are **not** verified. No physical device was flashed or tested.

Reproduce with the firmware URL and SHA-256 in the [receipt](evidence/stopwatch-burner-startup.json),
selecting StopWatch in the platform. Native reproduction after downloading the
artifact to a temporary path:

```sh
target/release/esp32sim --board stopwatch --flash-mb 16 --psram-mb 8 \
  --rom web/wasm/fw/esp32s3_rev0_rom.elf --boot rom --max-seconds 3 \
  --no-jit --no-dump --console both --flash-image "$FIRMWARE_BIN" \
  --tft-png "$FRAME_PNG"
```

The board regression checks GPIO ACK/NACK, physical pin gating, shared writes
across both controllers and reset defaults. Official StopWatch UserDemo V0.5
also still reaches its real guide screen in browser WASM.

## Kaleidoscope v0.0.2: separate failure, unresolved

The Burner image downloads successfully but faults before producing a frame.
With the I2C fix above, native and browser execution still report a coprocessor
exception at `0x420137c5`. A native instruction breakpoint identifies `FloatS`
with `CPENABLE=0`, `PS=0x60120`; its firmware application name is
`StopWatch-UserDemo`, version `0.0.2-kaleidoscope`.

This establishes the first observed failure, not whether real hardware behaves
the same or which startup component is responsible. No forced coprocessor
enable, instruction skipping or synthetic success has been applied. The
downloaded binary and its provenance are recorded separately in the receipt.
