# C5 independent SRAM protocol fixture

`probe.S` is original RV32IMAC assembly linked at `0x40800000`. It is a local
test, not M5Stack factory firmware or an ESP-IDF application. Its UART0 handler
uses C5 source 47, standard CLIC CSRs and the C5 GPIO matrix. Sending `1` lights
the Stamp-C5 GPIO28 blue LED; `0` extinguishes it. `W` drives GPIO27 low without
changing GPIO28 (a wrong-pin control). `R` transmits two RMT symbols
on explicit GPIO5, signal64, and polls TX_END. No RGB exists on Stamp-C5 itself.

Rebuild with Espressif GCC `esp-14.2.0_20241119` (GCC14.2.0):

```sh
RISCV_GCC=/path/to/riscv32-esp-elf-gcc sh firmware/c5-uart-probe/build.sh
cargo test -p esp32c5
cargo run -p esp32c5 --example app_image -- stamp-c5 firmware/c5-uart-probe/probe.bin 10W
```

The build script also runs Espressif esptool5.4.0 `elf2image --chip esp32c5`
with revision100..199 and4MiB flash declaration. `probe.bin` is a standard ESP
app image with chipID0x17, valid XOR checksum and appended SHA-256; it has no
IDF app descriptor. Load it as **C5 SRAM app-direct**, not merged flash/ROM boot.
Every segment and the entry point are inside SRAM. The linker word-aligns the
sections so esptool's padding cannot overlap a following segment.

`probe.elf` and `probe.bin` are committed so ordinary CI tests execute the real guest without a
cross compiler. No debug metadata or machine paths are embedded. The single
RWX SRAM segment is intentional for this minimal assembler/linker fixture.
Register provenance and exact hashes are in
`docs/evidence/c5-bringup/sources.json`, `results.json` (checkpoint) and
`machine-results.json` (app-image follow-up).
