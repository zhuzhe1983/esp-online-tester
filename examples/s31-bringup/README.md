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
  bash examples/s31-bringup/build.sh target/s31-fixtures
S31_UART_ELF="$PWD/target/s31-fixtures/probe.elf" \
S31_UART_IRQ_ELF="$PWD/target/s31-fixtures/irq-probe.elf" \
S31_TRNG_ELF="$PWD/target/s31-fixtures/trng-probe.elf" \
S31_WDT_ELF="$PWD/target/s31-fixtures/wdt-probe.elf" \
S31_MMU_ELF="$PWD/target/s31-fixtures/mmu-probe.elf" \
S31_FLASH_WORKER_BIN="$PWD/target/s31-fixtures/flash-worker.bin" \
S31_PADS_ELF="$PWD/target/s31-fixtures/pads-probe.elf" \
S31_EFUSE_ELF="$PWD/target/s31-fixtures/efuse-probe.elf" \
S31_CLOCKS_ELF="$PWD/target/s31-fixtures/clocks-probe.elf" \
S31_USB_ELF="$PWD/target/s31-fixtures/usb-probe.elf" \
S31_MONITOR_ELF="$PWD/target/s31-fixtures/monitor-probe.elf" \
S31_MSPI_ELF="$PWD/target/s31-fixtures/mspi-probe.elf" \
S31_CACHE_ELF="$PWD/target/s31-fixtures/cache-probe.elf" \
S31_ANALOG_ELF="$PWD/target/s31-fixtures/analog-probe.elf" \
S31_POWER_ELF="$PWD/target/s31-fixtures/power-probe.elf" \
S31_PMS_ELF="$PWD/target/s31-fixtures/pms-probe.elf" \
S31_CPLL_ELF="$PWD/target/s31-fixtures/cpll-probe.elf" \
S31_FLASH_PARAMS_ELF="$PWD/target/s31-fixtures/flash-params-probe.elf" \
S31_RESET_ELF="$PWD/target/s31-fixtures/reset-probe.elf" \
S31_MWDT_ELF="$PWD/target/s31-fixtures/mwdt-probe.elf" \
S31_RTC_TIMER_ELF="$PWD/target/s31-fixtures/rtc-timer-probe.elf" \
S31_NOR_WRITE_ELF="$PWD/target/s31-fixtures/nor-write-probe.elf" \
S31_GPIO_INPUT_ELF="$PWD/target/s31-fixtures/gpio-input-probe.elf" \
S31_BOOT_IMAGE="$PWD/target/s31-fixtures/spi0-observation.bin" \
S31_ROM_ELF="$PWD/target/s31-sources/roms/esp32s31_rev0_rom.elf" \
  cargo test --release -p esp32s31 --test uart_fixture -- --include-ignored
cargo run --release -p esp32s31 --example bringup -- target/s31-fixtures/probe.elf A
cargo run --release -p esp32s31 --example bringup -- target/s31-fixtures/irq-probe.elf A
cargo run --release -p esp32s31 --example bringup -- target/s31-fixtures/trng-probe.elf
```

Generated ELF/disassembly stays in ignored output. Product catalogs and the deployed
website do not include this fixture. Register citations and bring-up limits are in
`docs/esp32s31.md`.

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


`gpio-input-probe.S` uses GPIO36's documented function0 GPIO alias and reads
GPIO32's unresolved Flash-D pad in the same bank. The host supplies an explicit
already-resolved voltage sample, then changes it and sends UART `A`; the actual
guest branches on both bank reads and reports PASS. Missing samples and
contradictory external drivers fault; the wrong initial level takes the guest
FAIL branch. `set_unresolved_sample` does not implement a peripheral waveform,
change `Drive::Unsupported`, override known GPIO output, or disable input gating.
Set `S31_GPIO_INPUT_ELF` to the generated ELF. The bringup example's repeated
`--peripheral-pad=32:0` option is a development input, off by default; it does not
identify the real INK4 Plus pin voltage or manufacture a product profile.

`pmu-policy-probe.S` checks separate active/sleep policy banks, write-trigger
readback, documented read-only bias fields and release of forced domain controls.
It emits `S31 PMU POLICY PASS` only after guest readback succeeds. Three negative
modes request active CPU powerdown, unsupported partial APB gating and forced TOP
reset; each traps at the actual store. Bias initialization counts a request, not
analog calibration completion. Set `S31_PMU_POLICY_ELF` to its generated ELF.

`mmu-probe.S` now also enables `PMU_PSRAM_XPD` before accessing the explicitly
supplied PSRAM backing. The bus checks this PHY power gate as well as the PSRAM
clock for reads, writes and instruction fetches. Enabling power does not create
or identify a memory chip. PHY gating preserves the caller's backing bytes;
external RAM rail loss, decay and retention are not modeled.

`mpll-probe.S` enables the real MPLL power sequence and analog master, configures
400 MHz, polls functional calibration and routes MPLL/2 to the PSRAM core. It
writes executable instructions into caller-supplied PSRAM, fetches them through
the real MMU and checks the returned42 before UART PASS. Negative modes omit the
analog clock or PHY power, gate the PLL path, change the divider after calibration,
or attempt to write RO completion with calibration stopped. Each fails before
retiring the PSRAM entry. Set `S31_MPLL_ELF` to its generated ELF. The calibration
uses sixteen private runner quanta; it is not a physical settling-time model and
does not identify a RAM die or replace the original product firmware.

`mspi-pads-probe.S` reads dedicated PSRAM pad reset values, changes drive/delay
configuration while HOLD is set, then releases it and writes/executes code from
caller-supplied memory. Real data readback precedes execution. Three negative
modes keep CS, DQS or a data pin held and fault respectively on an external store,
instruction fetch or data read, with no retired PSRAM entry. Set
`S31_MSPI_PADS_ELF` to the generated ELF. The conservative HOLD subset preserves
RW configuration but refuses transfers: it does not invent a frozen physical
waveform or simultaneous-write sampling order. DQS power is a requested field;
the generic backing-memory path does not claim a DDR serial protocol or RAM die.

`psram-controller-probe.S` reads the documented AP mode-register pairs,modifies
MR8 while preserving MR0 despite an extra bus byte,writes instructions through
MSPI3,reads actual data back,then configures MSPI2/MMU and executes from that same
backing array. Set `S31_PSRAM_CONTROLLER_ELF` to an absolute ELF path. The ten modes
include a PASS/return42 and nine negatives: missing device,DQS,PHY/core clock,
closed AXI,DQS removed before fetch,invalid MR/command and absent backing.
The test supplies reference MR0,1,2,3,4,8 bytes explicitly; this does not identify a
physical Plus die. DDR transactions are functional,with no analog timing margin.

`apm-probe.elf` exercises real SRAM reads, writes and instruction fetch through an
explicit testbench HP_MEM_APM path2 connection. Set `S31_APM_ELF` to its absolute
path. The fixture covers independent R/W/X denial, unchanged storage on denied
write, no execution on denied fetch, filter disable/restoration, missing route,
and unsupported quiet denial. This declared connection is **not** identification
of the physical S31 master/path topology. No fixture binary is published.

`multicore-probe.S` uses the real S31 ROM to start hart1, rather than setting its
PC from the host. Set `S31_MULTICORE_ELF` and `S31_ROM_ELF`; the test first runs
hart0's actual ROM initializer loop, then loads this SRAM-linked test program.
The guest writes its secondary entry with the ROM API, enables clocks/releases
reset, and verifies independent CLIC state, 400 simultaneous atomic increments,
SC conflict retries, software stall/resume, and a second ROM boot to a changed
entry. Shared result words come only from executed guest stores. The fixture does
not validate an INK4 Plus board or physical dual-core timing. All generated ELFs,
BINs and objdump listings stay in the ignored output directory.

`cache-error-probe.S` starts hart1 through the same real ROM path, then closes
the shared D-cache bus and executes a real load from mapped Flash. Set
`S31_CACHE_ERROR_ELF` and `S31_ROM_ELF`. The runner first reports the load fault;
the test resumes at the guest exception vector without changing guest registers.
Hart0's real source69 CLIC handler observes the latched status, proves direct RAW
writes cannot clear it, clears it through the documented WT register and reopens
the bus. The guest writes its own completion word and prints PASS. This exercises
functional tracing and interrupt delivery, not cache-line timing or physical
requester-ID capture.

`rtc-calibration-probe.S` performs actual TIMG0 register polling, tests a 3-cycle
XTAL/10 measurement (30 XTAL ticks), a gated-target timeout, then a 17-cycle
nominal RC_SLOW measurement (5000 XTAL ticks at136kHz). Set
`S31_RTC_CALIBRATION_ELF`. The first20000 guest instructions run with no clock
input and must stay in the polling loop; only enabling the explicit nominal CPU
clock conversion allows the same guest to continue and print PASS. No host writes
the completion flags or mailbox. The one-instruction-per-cycle relationship is a
development approximation; it is not physical clock calibration.

`modem-clocks-probe.S` configures BLE/COEX/Wi-Fi-power LP clock requests and
UART1/2/3 clock gates/dividers, then performs eight real internal I²C reads/writes
and resets the digital master while preserving its explicit BIAS target data.
Set `S31_MODEM_CLOCKS_ELF`. The guest writes its completion word and UART PASS;
the harness checks 1MHz/34kHz/2MHz LP rates and independent UART APB/core gating.
Negative cases disable the master, gate its parent, or omit BIAS input, and must
fault without a transaction or PASS. FORCE_ON does not replace software EN.
The UART3 disabled-core case clears CLK_EN bit7 while retaining SYS/APB bits0/1;
clearing SYS is a different gate. This verifies configuration and existing I²C
behavior, not radio, network, auxiliary UART traffic or sleep/wakeup behavior.

`startup-clocks-probe.S` executes the documented unused-clock shutdown sequence
for the supported cold configuration words. Set `S31_STARTUP_CLOCKS_ELF`.
It preserves USB OTG reset bits, checks TEE automatic clock policy, and computes
a real SHA256 block through the existing SHA device before gating/restoring its
clock and reading the unchanged digest. Six negative modes access gated SHA,
unimplemented ETM data, unsupported LCDCAM activation, or a PMS-denied TEE,
LP_PERICLKRST or CNNT word. No mode replaces a denied read with a fake value.
These tests do not implement the data planes behind the new clock requests.

`systimer-probe.S` verifies real snapshot polling, a future one-shot target,
source33/CLIC delivery, interrupt acknowledgement and `mret`. Set
`S31_SYSTIMER_ELF`. The first20000 instructions have no timer clock input and
cannot leave the snapshot loop. Explicit nominal CPU/source timing lets the
guest reach WFI;512 explicitly supplied timer ticks then wake it through its
actual interrupt handler. The handler checks RAW cannot be cleared by writing
status and clears it through INT_CLR before writing its mailbox and UART PASS.
No automatic idle-time advance or complete product readiness is implied.

`psram-irq-probe.S` executes real loads and stores through an invalid PSRAM MMU
entry, with an explicit 64KiB development backing array. Set `S31_PSRAM_IRQ_ELF`.
The harness stops at each actual access exception and resumes without changing
guest registers or PC. The guest exception handler advances its own `mepc`; its
source112 CLIC ISR checks and clears the real read/write error bits. Finally the
guest creates a valid mapping and verifies a successful data write/read, prints
PASS and writes its mailbox. This tests functional address errors and the two
handlers; it does not identify a physical RAM die or claim ECC/FIFO engines.

`brownout-probe.S` configures the real internal BOD threshold byte with a guest
read-modify-write, then programs mode0 and reaches WFI. Set `S31_BROWNOUT_ELF`.
No implicit voltage or clock input is supplied. Two explicit detector edges with
an already-qualified active comparator sample trigger source143; the guest ISR
clears HP/LP raw banks independently and returns with `mret` to print PASS. A
second mode produces a diagnostic digital-system reset request after four
edges. Negative modes omit the required analog byte or request an unimplemented
RF shutdown on a real event; neither fabricates successful completion. The test
does not measure physical voltage, comparator latency, or Flash/RF power behavior.

`waiti-probe.S` configures the documented automatic Flash idle controls, then
executes real erase/program/read and calls instructions from the modified Flash.
Set `S31_WAITI_ELF`. Enabled wait samples the completed NOR status three times;
disabled wait performs no automatic status transaction. Wrong command/address/
width and actual suspend requests are negative modes that stop before modifying
Flash. This is the existing synchronous functional NOR,not physical BUSY latency
or analog timing tuning.

`lazy-fpu-probe.S` leaves FS Off,executes FLW,and enters its own exception handler.
Set `S31_LAZY_FPU_ELF`. The first diagnostic stop preserves the architectural
trap. Resuming calls the real handler,which enables FS and returns with `mret`;
the same FLW retires and real FADD/FSW produce binary32 `0x40400000`. UART `L`
leaves FS Off for a repeated-exception test;UART `X` causes an unmapped load.
The native `bringup --guest-exceptions=N` option resumes at most N architectural
exceptions without bus faults;the default0 stops at the first exception. It
never changes PC/registers or skips guest instructions. The reusable remote
probe exposes the same bounded option and records its value. This fixture does
not prove that the private Plus firmware handles its own FPU exception correctly.


`fpu-reason-probe.S` checks EXT_ILL for six actual FS-Off full/compressed floating
loads and stores. Set `S31_FPU_REASON_ELF`. The strict default reaches the guest's
FAIL because the memory reason remains zero; the explicit private S31 upstream
compatibility policy produces six real traps, handler `mret` returns, and PASS
with binary32 1.5 stored by actual FSW. No host FS/PC repair is used. The reference
Espressif emulator0.51.0 also passes this source image after real ROM boot:

```sh
esptool --chip esp32s31 elf2image --flash-mode dio --flash-size 16MB \
  --flash-freq 40m --output target/s31-fixtures/fpu-reason-probe.bin \
  target/s31-fixtures/fpu-reason-probe.elf
python3 - <<'PYTHON'
from pathlib import Path
p = Path('target/s31-fixtures')
(p / 'fpu-reason-rom-image.bin').write_bytes(bytes([255]) * 8192 + (p / 'fpu-reason-probe.bin').read_bytes())
PYTHON
<reference-esp-emu> --chip esp32s31 \
  --firmware target/s31-fixtures/fpu-reason-rom-image.bin \
  --net user,restrict=yes --timeout 1s --exit-on 'S31 FPU MEMORY' --log-color never
```

Require `S31 FPU MEMORY REASONS PASS`; exit0 alone is not success. Generated
source ELF/BIN stays in ignored output. This reference behavior conflicts with
the public IDF S31 load/store reason-bug capability, so the private runner's
`--upstream-fpu-memory-reason` remains explicit and off by default. See
`docs/esp32s31.md` for evidence and limitations.


`crypto-clock-probe.S` verifies actual SHA-256 of `abc` through S31 XTAL, RC_FAST
and explicit BBPLL/ref240 roots. Set `S31_CRYPTO_CLOCK_ELF`. The same guest first
checks real key-manager reset, lock and no-key state. Clock-negative modes leave
SHA untouched; reserved selector fails at its write. Modes6/7 attempt actual
key-manager START/CONTINUE and must stop without completion, success or valid
keys. This bounded initialization model has no key-generation/deployment engine
and does not claim secure boot or Flash encryption.
