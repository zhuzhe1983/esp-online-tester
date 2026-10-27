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
S31_BUILD_ROM_IMAGE=1 ESPTOOL_BIN=/path/to/esptool RISCV_TOOLCHAIN_PREFIX=/path/to/riscv32-esp-elf- \
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
S31_CACHE_ELF="$PWD/output/s31-uart-probe/cache-probe.elf" \
S31_ANALOG_ELF="$PWD/output/s31-uart-probe/analog-probe.elf" \
S31_POWER_ELF="$PWD/output/s31-uart-probe/power-probe.elf" \
S31_PMS_ELF="$PWD/output/s31-uart-probe/pms-probe.elf" \
S31_CPLL_ELF="$PWD/output/s31-uart-probe/cpll-probe.elf" \
S31_FLASH_PARAMS_ELF="$PWD/output/s31-uart-probe/flash-params-probe.elf" \
S31_RESET_ELF="$PWD/output/s31-uart-probe/reset-probe.elf" \
S31_MWDT_ELF="$PWD/output/s31-uart-probe/mwdt-probe.elf" \
S31_RTC_TIMER_ELF="$PWD/output/s31-uart-probe/rtc-timer-probe.elf" \
S31_NOR_WRITE_ELF="$PWD/output/s31-uart-probe/nor-write-probe.elf" \
S31_BOOT_IMAGE="$PWD/output/s31-uart-probe/spi0-observation.bin" \
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

`cache-probe.S` configures Flash MMU mapping, exercises explicit light-retention,
invalidates an actual selected cache set, freezes/releases D-cache and checks the
same independently supplied Flash word before and after. Changed bytes must take
its failure branch; no Flash array or closed cache clock must fault before PASS.
Set `S31_CACHE_ELF` to its source-built ELF when running the full fixture suite.
This checks the functional direct-memory contract, not cache lines or their timing.

`spi0-observation.S` is an **emulator-only** observation fixture for the documented
ROM/header discrepancy; do not flash it to hardware. Its reads and full-word writes
target SPI0 reserved offsets0x24/0x28. Rebuild with the script above, convert using
esptool5.4.0 `--chip esp32s31 elf2image --flash-mode dio --flash-size 16MB
--flash-freq 40m`, and put the result at0x2000 in an erased-byte merged input. Run
Espressif esp-emulator0.51.0 with the fixed public ROM, `--chip esp32s31 --rom
<rom> --firmware <generated-merged-input> --net user,restrict=yes --log-color never`.
Bound the subprocess from the host and close it after `SPI0 OBSERVATION DONE`;
the emulator's own `--timeout` does not promptly end this guest's WFI loop. Retain
only public UART values and relevant interception counts, not host-network logs.
Upstream compatibility is distinct from hardware verification and our strict map.

To exercise actual ROM SHA loading, build with `S31_BUILD_ROM_IMAGE=1` and
`ESPTOOL_BIN=/path/to/esptool` (esptool5.4.0). Set `S31_BOOT_IMAGE` to the generated
`spi0-observation.bin`, in addition to the existing source/ROM fixture variables.
The host test places only this source-built input at0x2000 in an explicit erased
64KiB development Flash array. ROM performs its own loading and SHA work. The
valid case reaches `SPI0 OBSERVATION DONE`; a one-bit appended-digest change emits
the ROM's SHA mismatch warning and still boots, while a one-bit payload change
emits the XOR-checksum assertion and never enters the source guest. The warning-only
SHA result is observed behavior without secure boot, not a security guarantee or
host-made decision. These negative controls do not modify any official product BIN.


`analog-probe.S` exercises the S31 internal analog-I²C digital transaction path
with caller-supplied BIAS register0/register1 bytes `17/29`, both master routes,
and digital-master reset. It checks real guest reads before changing the target
and completes the documented MPLL shutdown sequence. Run its external test with
an absolute `S31_ANALOG_ELF` path; generated ELF stays in ignored `output/`.
The private `bringup` example accepts `--analog-bias=hhhh` (register0 then
register1) as an explicit development input. Public headers do not establish
these analog reset values; no seed is installed by default, and no voltage,
settling, PLL-lock or product-hardware claim follows from byte readback.

`power-probe.S` reads caller PERIF register10 seed `f7`, clears detector bits to
`f2`, resets the target through the PMU control and checks restored `f7`. Its BOD
mode1 enable comes from the host mailbox at `0x2f070000`. After the guest reaches
WFI, a qualified comparator event produces a typed Brownout reset request only
when enabled; no voltage or physical glitch model is implied. Set the absolute
`S31_POWER_ELF` path for this test. The private example accepts
`--analog-perif-glitch=hh` as an explicit target input; missing seed, closed power,
asserted reset and incorrect route fail. Product firmware is never altered by
these source-built positive/negative controls.

`pms-probe.S` sends `A`, installs and locks UART permissions from a host mailbox,
attempts to overwrite the locked control, reads real UART status, and sends `B`.
Set absolute `S31_PMS_ELF` when running the source fixture suite. The positive
case emits `AB`; denied reads/writes leave only `A` and fault at the actual device
access. Restrictions require the explicit bus-error response; quiet-denial read
semantics, security-domain transitions and PMS IRQ/record registers are not yet
implemented. This validates the private HP-CPU TEE subset, not security isolation
for an entire product.

`cpll-probe.S` uses the real S31 ROM frequency reader after configuring CPLL80,
updates DIG through real internal-I²C transactions and removes the RTC_FAST
oscillator gate. Set absolute `S31_CPLL_ELF` and `S31_ROM_ELF` paths. Missing
power, invalid divider and no-start controls take the guest timeout path; wrong
DIG data takes its failure branch; missing data and removal of an active CPU
clock trap. Calibration takes16 functional model quanta, not measured physical
settling time. The explicit `--analog-dig=hhhhhhhh` seed orders registers5,7,13,14;
headers do not establish their analog reset bytes. The fixture does not identify
product hardware or validate a physical PLL, RTC timing or voltage.

`flash-params-probe.S` reads caller-supplied SFDP bytes at two actual SPI1
addresses, selects an explicit stable BBPLL480 MHz input divided by six, then
sends an idle Winbond Resume command. Set `S31_FLASH_PARAMS_ELF`. The valid guest
prints `S31 SFDP FLASH80 IDLE PASS`; wrong bytes take its own failure branch.
Absent SFDP, absent BBPLL, removal of the actual ALIVE clock gate and another
manufacturer fail at the corresponding guest access. This does not fabricate a
complete SFDP table, program/erase completion, oscillator lock or product BOM.
The private runner accepts `--nor-sfdp=hex-offset:hex-bytes` and
`--bbpll-hz=480000000` as explicit development inputs; both are absent by default.


`reset-probe.S` exercises each HP core's RTC watchdog receiver gate, actual
software-reset stores, and lockup gates with an explicit qualified host event.
It emits `S31 RESET READY`; an ordinary guest exception is not classified as
lockup. Reset requests stop the private runner; they do not reconstruct full
reset domains or run the second hart.

`mwdt-probe.S` independently uses TIMG0/1 watchdogs, with write protection,
configuration-update, prescaling and real CLIC interrupt/mret. Eight explicit
XTAL ticks cause the guest ISR to emit `S31 MWDT IRQ PASS`; six further ticks
request a system reset. A gated WDT clock does not advance the counter, and
clearing the first group's interrupt does not clear the other group's state.
Automatic Flashboot timing and GP timers are not implemented.

`rtc-timer-probe.S` reads actual latched 48-bit RTC counter snapshots, checks
update-done/clear and digital reset. One run supplies slow-clock ticks explicitly;
a second opts into the interpreter-cycle clock approximation. A missing clock
input takes the guest's failure branch; a denied PMS permission faults at the
actual read. The optional profile converts interpreter cycles using the active
CPU rate and nominal 136 kHz RC_SLOW rate. The interpreter accounts one cycle per
instruction; this is not silicon timing. It advances RTC_TIMER only, not the
watchdogs or a complete sleep clock scheduler. Alarm/IRQ enable, wakeup and
REGDMA requests remain explicit unsupported operations.


`nor-write-probe.S` erases an independently supplied Flash sector, programs two
instructions through SPI1 USER02, reads them through the actual Flash MMU and
executes them. The programmed function must return42 before the guest emits
`S31 NOR ERASE PROGRAM EXECUTE PASS`. A second FF write cannot restore cleared
bits. Omitting erase or program WREN takes the guest's own failure branch;
another NOR manufacturer traps before a mutation. This fixture also checks the
optional PC retirement observer: it records the programmed entry once only for
the successful run. No fixture binary is published or substituted for a product
firmware image.
