# S31 SRAM UART fixture

This debugger-style SRAM program exercises the S31 UART0 FIFO/status register map,
SRAM store/load and shared single-precision F execution. It prints `S31 UART READY`,
waits for RX byte `A`, echoes it and prints `S31 SRAM FPU PASS`. Any other input
prints `S31 FAIL` and traps. It is neither an ESP-IDF image nor a substitute for
the official firmware; it does not configure pin mux, clocks or a board.

`irq-probe.S` separately exercises UART0 RX-full source 9, the S31 matrix's
machine-mode PASS_LEVEL, CLIC ID 17, standard `mintthresh`/`mintstatus`, a real guest
ISR and `mret`. It configures three level bits and waits at WFI for `A`; its handler
checks the current interrupt level, consumes and echoes the byte, then returns to
the main code, which checks that the level was restored and prints `S31 IRQ MRET PASS`.
The host test also masks and unmasks the threshold while the byte is pending.

```sh
RISCV_TOOLCHAIN_PREFIX=/path/to/riscv32-esp-elf- \
  bash firmware/s31-uart-probe/build.sh output/s31-uart-probe
S31_UART_ELF="$PWD/output/s31-uart-probe/probe.elf" \
S31_UART_IRQ_ELF="$PWD/output/s31-uart-probe/irq-probe.elf" \
  cargo test --release -p esp32s31 --test uart_fixture -- --include-ignored
cargo run --release -p esp32s31 --example bringup -- output/s31-uart-probe/probe.elf A
cargo run --release -p esp32s31 --example bringup -- output/s31-uart-probe/irq-probe.elf A
```

Generated ELF/disassembly stays in ignored output. Product catalogs and the deployed
website do not include this fixture. Register citations and bring-up limits are in
`docs/platform/s31-runtime.md`.
