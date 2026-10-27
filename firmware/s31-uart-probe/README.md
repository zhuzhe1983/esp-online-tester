# S31 SRAM UART fixture

This debugger-style SRAM program exercises the S31 UART0 FIFO/status register map,
SRAM store/load and shared single-precision F execution. It prints `S31 UART READY`,
waits for RX byte `A`, echoes it and prints `S31 SRAM FPU PASS`. Any other input
prints `S31 FAIL` and traps. It is neither an ESP-IDF image nor a substitute for
the official firmware; it does not configure pin mux, clocks, CLIC or a board.

```sh
RISCV_TOOLCHAIN_PREFIX=/path/to/riscv32-esp-elf- \
  bash firmware/s31-uart-probe/build.sh output/s31-uart-probe
S31_UART_ELF="$PWD/output/s31-uart-probe/probe.elf" \
  cargo test --release -p esp32s31 --test uart_fixture -- --include-ignored
cargo run --release -p esp32s31 --example bringup -- output/s31-uart-probe/probe.elf A
```

Generated ELF/disassembly stays in ignored output. Product catalogs and the deployed
website do not include this fixture. Register citations and bring-up limits are in
`docs/platform/s31-runtime.md`.
