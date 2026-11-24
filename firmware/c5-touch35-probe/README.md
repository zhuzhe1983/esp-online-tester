# Original C5 Touch LCD3.5 SRAM diagnostic

This is project-authored freestanding RV32 code, **not Waveshare factory
firmware**. It uses real C5 MMIO, manufacturer-sourced board wiring and the
public App/touch/BOOT interfaces. The board profile has32MiB flash/8MiB PSRAM.

The guest initializes the sourced C5 Quad PSRAM transport and MMU, writes and
reads all8MiB, then places a320×480 RGB565 framebuffer there. SPI2 GDMA reads
that framebuffer through the mapped PSRAM. AXP2101 DCDC1 is explicitly set
to3300mV; CH32V006 receives full16-bit reset/direction values and its actual
PWM command. FT6336 count/coordinate reads trigger a visible32×32 patch.
BOOT28 inverts a288×416 region. Releases restore the original scene.

The SD diagnostic saves LBA32, writes a512-byte test pattern, reads it back,
restores the saved bytes and reads those back. The harness additionally
compares the complete initial and final card image. This does not mount a
filesystem and does not guarantee restoration if execution is interrupted.

Build using the fixed Espressif GCC14.2 toolchain and esptool5.1.0 documented
in the shared C5 source receipts:

```sh
RISCV_GCC=/path/to/riscv32-esp-elf-gcc ESPTOOL=/path/to/esptool sh build.sh
```

Run from the repository root:

```sh
CARGO_BUILD_JOBS=1 CARGO_PROFILE_RELEASE_LTO=false CARGO_TARGET_DIR=target-c5-rebuild \
  cargo run -p esp32c5 --release --example touch35_probe -- \
  firmware/c5-touch35-probe/probe.bin target/c5-touch35-2026-11-23
```

This selects the explicit SRAM App path. It does not establish flash boot,
manufacturer desktop execution, physical PSRAM manufacturer identity,
physical display calibration, Wi-Fi/BLE, audio, camera or battery support.
The supplied App image is4688bytes, SHA-256
`9395b2b72f506fabdcbe238f1a8fe39ce7825ecf78ab535c572d054f45f3b1e3`.
