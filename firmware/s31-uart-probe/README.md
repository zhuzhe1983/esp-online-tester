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

`trng-probe.S` checks the CRC random-data MMIO path: no sample before enable,
distinct samples after enable, frozen data with each clock/sampling gate disabled,
and cleared data after peripheral reset. `S31 TRNG GATES PASS` is functional
simulation evidence, not a test of physical entropy or cryptographic quality.

`wdt-probe.S` arms RTC watchdog stage 0 for an interrupt and stage 1 for system
reset. The host explicitly advances the RTC slow-clock domain: seven ticks produce
no ISR, the eighth prints `S31 WDT IRQ PASS` through a real guest handler, and three
more produce a typed reset request. This does not prove a CPU-to-RTC clock tree or
complete reset-domain execution; the development runner stops at that request.

`mmu-probe.S` writes the S31 Flash/PSRAM MMU index and content ports, then calls
`flash-worker.S` at a mapped Flash virtual address. That code writes PSRAM and
returns a value; the SRAM caller remaps the same virtual address to a different
physical page, checks it is initially empty, restores the mapping and checks the
first page's data. The host supplies this source-built worker and 16 KiB of model
PSRAM explicitly. This is not a product memory-capacity claim or a flash boot test.

`pads-probe.S` configures GPIO10 as a low output and GPIO2 as an input, holds both
PMU domains, then changes output/input-enable registers. The host confirms GPIO10
remains low, changes GPIO2's external input to high, and sends UART `A`. The guest
checks that input is still live, releases HP and LP holds separately and verifies
the newly configured output/input states before printing `S31 PAD HOLD INPUT PASS`.
Without the external high input it takes its explicit failure branch.

`efuse-probe.S` transfers an explicitly supplied synthetic decoded eFuse read image,
checks read-only shadow data and the read-complete interrupt, and clears the interrupt.
It does not program OTP, synthesize a product identity, or validate ECC/security behavior.
The same guest faults at READ_CMD when no input image is supplied.

`clocks-probe.S` calls the fixed 20260528 S31 ROM's `ets_clk_get_cpu_freq`,
checks 40 MHz, commits a divide-by-two XTAL root and checks 20 MHz. It waits for
host UART data, pulses core reset and verifies the RX FIFO emptied while its
configuration remains; an APB module reset then restores the modeled register defaults.
Host checks verify computed clock/baud values and retain already-transmitted output.
This does not measure physical UART/clock timing or prove PLL calibration.

`usb-probe.S` fills the real EP1 FIFO with bytes 0..63 and waits for an explicitly
attached functional host to consume it. The guest then submits a ZLP, configures
USB receive source 2 through the matrix/CLIC, and waits at WFI. Host packet `A`
triggers a guest ISR that validates and echoes the byte, clears RX interrupt and
returns with `mret`. No host means no progress; a masked CLIC threshold prevents
the ISR. This is a packet boundary model, not USB enumeration or physical timing.

The separate fixed-ROM tests reconstruct ROM-side initializer bytes from the ELF,
then verify the guest's own unpack loop installs the exact interface-table pointer.
Explicit straps 0/0x12 and an attached packet peer produce USB/SPI Flash boot banners
before the documented first unsupported USB clock or SPI0 reserved-register access. They do not establish
a real product strap or OTP profile, or a successful flash boot.

```sh
RISCV_TOOLCHAIN_PREFIX=/path/to/riscv32-esp-elf- \
  bash firmware/s31-uart-probe/build.sh output/s31-uart-probe
S31_UART_ELF="$PWD/output/s31-uart-probe/probe.elf" \
S31_UART_IRQ_ELF="$PWD/output/s31-uart-probe/irq-probe.elf" \
S31_TRNG_ELF="$PWD/output/s31-uart-probe/trng-probe.elf" \
S31_WDT_ELF="$PWD/output/s31-uart-probe/wdt-probe.elf" \
S31_MMU_ELF="$PWD/output/s31-uart-probe/mmu-probe.elf" \
S31_FLASH_WORKER_BIN="$PWD/output/s31-uart-probe/flash-worker.bin" \
S31_PADS_ELF="$PWD/output/s31-uart-probe/pads-probe.elf" \
S31_EFUSE_ELF="$PWD/output/s31-uart-probe/efuse-probe.elf" \
S31_CLOCKS_ELF="$PWD/output/s31-uart-probe/clocks-probe.elf" \
S31_USB_ELF="$PWD/output/s31-uart-probe/usb-probe.elf" \
S31_MONITOR_ELF="$PWD/output/s31-uart-probe/monitor-probe.elf" \
S31_MSPI_ELF="$PWD/output/s31-uart-probe/mspi-probe.elf" \
S31_ROM_ELF="$PWD/output/s31-sources/roms/esp32s31_rev0_rom.elf" \
  cargo test --release -p esp32s31 --test uart_fixture -- --include-ignored
cargo run --release -p esp32s31 --example bringup -- output/s31-uart-probe/probe.elf A
cargo run --release -p esp32s31 --example bringup -- output/s31-uart-probe/irq-probe.elf A
cargo run --release -p esp32s31 --example bringup -- output/s31-uart-probe/trng-probe.elf
```

Generated ELF/disassembly stays in ignored output. Product catalogs and the deployed
website do not include this fixture. Register citations and bring-up limits are in
`docs/platform/s31-runtime.md`.

`monitor-probe.S` reads its actual PC/SP capture, generates a stack-underflow IRQ
through source119/CLIC17, and creates genuine load/store access exceptions. The
host resumes only at the architectural trap vector; the guest handler and checks
verify both exception records. `mspi-probe.S` checks an explicitly supplied test
NOR ID/status and two Flash reads. Its negative controls change the data, omit
identity/array, or close the clock; none may produce PASS. These inputs do not
identify INK4 Plus hardware. The MMU fixture explicitly opens/enables SPI0 AXI.
