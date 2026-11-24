# Original C5 8 MiB Quad memory diagnostic

This freestanding RV32IMAC program is an original diagnostic, not manufacturer
firmware. It starts from SRAM through the ordinary C5 app-direct loader and
configures the C5 physical MSPI pins, serial/QPI commands and MMU itself. No ROM
function is skipped or patched and no unknown analog register is host-seeded.

The guest first verifies SPI/QPI read/write commands. It opens the default-closed
AXI path, maps 128 pages, writes and checks all 2,097,152 32-bit words of an 8 MiB
die, checks 8/16-bit accesses at the high end and executes two instructions from
PSRAM. A failure prints `C5 PSRAM8 FAIL`; only completed checks produce
`C5 PSRAM8 PASS` on UART0.

```sh
RISCV_GCC=/path/to/riscv32-esp-elf-gcc ESPTOOL=/path/to/esptool \
  sh firmware/c5-psram-probe/build.sh
cargo run --release -p esp32c5 --example psram_probe -- \
  firmware/c5-psram-probe/probe.bin
```

The committed ELF/bin are built with Espressif GCC 14.2.0
(`esp-14.2.0_20260121`) and esptool 5.1.0. The image header declares C5
revision 100..199 and 4 MiB flash for this SRAM fixture; host configuration adds
the explicit 8 MiB Quad die. It is not a 32 MiB board factory image.

The single RWX SRAM segment is intentional. Nominal timing uses a 48 MHz
crystal, 12 MHz serial initialization and 48 MHz cache-memory clock. Power-up
delay loops use the emulator's 240 MHz CPU cycle counter; analog stabilization,
PLL tuning and cache-line latency are not simulated. See
[the memory model](../../docs/platform/c5-memory-runtime.md) and
[execution receipt](../../docs/evidence/c5-memory-2026-11-23/results.json).
