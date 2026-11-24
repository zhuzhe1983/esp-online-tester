# Original C5 LCD2.73 SRAM diagnostic

This project-authored freestanding RV32 program is independent of Waveshare's
factory image. The real board configuration is32 MiB flash/8 MiB PSRAM.

The guest initializes the sourced C5 Quad PSRAM/MMU, writes and reads all8 MiB,
and builds a320×320 RGB666 framebuffer in mapped PSRAM. Every pixel occupies
three wire bytes. SPI2 GDMA reads307,200 bytes using actual descriptors.
The 16-bit CH32V006 protocol resets the LCD and sets PWM. The QMI8658C
WHO_AM_I is read at6Bh. No touch, PMIC or codec is invented on this board.

PREV2 and NEXT3 each invert an88×304 region; BOOT28 inverts288×256 pixels.
All keys are active-low and polled by guest GPIO reads. Releases rebuild the
original frame. The host harness sends public GPIO-script events and checks
the actual subsequent output.

The SD workload saves LBA32, writes and reads a512-byte test pattern, restores
the original bytes and rereads them. The harness also compares the entire
initial and final card. It neither mounts FAT nor guarantees restoration if
execution is interrupted during the test.

Build with the preserved Espressif GCC14.2 and esptool5.1.0 toolchain:

```sh
RISCV_GCC=/path/to/riscv32-esp-elf-gcc ESPTOOL=/path/to/esptool sh build.sh
```

Run from the repository root:

```sh
CARGO_BUILD_JOBS=1 CARGO_PROFILE_RELEASE_LTO=false CARGO_TARGET_DIR=target-c5-rebuild \
  cargo run -p esp32c5 --release --example lcd273_probe -- \
  firmware/c5-lcd273-probe/probe.bin target/c5-lcd273-2026-11-23
```

The supplied4224-byte App SHA-256 is
`44cf80f57df4227ac568084955bb4a26aec49803e2e7390c9e845ab5fc444fcd`.
This App loads through the explicit SRAM path at app offset10000h. The
manufacturer merged image has its own application at20000h and remains a
separate ROM-boot reference. No factory, radio, USB, battery, physical color
calibration or filesystem acceptance follows from this diagnostic.
