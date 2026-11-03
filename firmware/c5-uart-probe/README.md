# C5 independent SRAM protocol fixture

`probe.S` is original RV32IMAC assembly linked at `0x40800000`. It is a local
test, not M5Stack factory firmware or an ESP-IDF application. Its UART0 handler
uses C5 source 47, standard CLIC CSRs and the C5 GPIO matrix. Sending `1` lights
the Stamp-C5 GPIO28 blue LED; `0` extinguishes it. `R` transmits two RMT symbols
on explicit GPIO5, signal64, and polls TX_END. No RGB exists on Stamp-C5 itself.

Rebuild with Espressif GCC `esp-14.2.0_20241119` (GCC14.2.0):

```sh
RISCV_GCC=/path/to/riscv32-esp-elf-gcc sh firmware/c5-uart-probe/build.sh
cargo test -p esp32c5
```

`probe.elf` is committed so ordinary CI tests execute the real guest without a
cross compiler. No debug metadata or machine paths are embedded. The single
RWX SRAM segment is intentional for this minimal assembler/linker fixture.
Register provenance and exact hashes are in
`docs/evidence/c5-bringup/sources.json` and `results.json`.
