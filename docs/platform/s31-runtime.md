# ESP32-S31 bring-up

Status on 2026-10-28: Metalio INK4 Plus remains **unavailable** in this platform.
The private S31 core now executes the original ROM, bootloader and official
application entry with explicitly declared development inputs. It currently stops
after Flash initialization at a real FS-Off floating-point exception in strict
mode. An explicit upstream-emulator compatibility option now lets the real
guest handler return and retire that instruction; startup then stops at the
unsupported crypto-domain clock control. No product framebuffer or complete
board support has been established. The earlier separate Espressif emulator
baseline below is retained for comparison, not presented as this core's result.

## Official emulator baseline

Inputs are pinned in [source receipts](evidence/s31-bringup/sources.json) and
[results](evidence/s31-bringup/official-baseline.json): esp-emulator v0.51.0,
repository `61d71cef2b1f13a936173c6d282a52c093137d6a`, and CloudZao Plus 2.0.68
SHA-256 `265c51b07252dc2864e71439fefc8e9448d182fcd14a6bccfa6952fe58859b80`.
The firmware's image chip ID is 32; its bootloader reports ESP-IDF `v6.1-dirty`.
The source comparison uses CloudZao revision
`ad8edf4150a688737c0d7c00cae4413079b262ef`; this is not a claim that its source
rebuilds the distributed binary bit for bit.

Unmodified release WASM was tested under Node and headless Chromium. Both reach
the same software-reset request after 62,350,000 requested batch instructions and
52,962,986 reported cycles. UART hashes match exactly. Native macOS arm64 reaches
the same first failure. Requested batch sizes are not retired-instruction counts.
No performance claim is made. Browser sessions and the temporary server were closed.

Observed sequence:

1. Embedded S31 ROM, bootloader, dual-core startup and PSRAM memory test succeed.
2. The upstream emulator supplies a simulated 16 MB PSRAM die. This does **not**
   establish the physical product's RAM capacity.
3. TCA9555 is absent; EPD power-on is skipped, TPS is not ready and EPD init fails.
4. Native reports `hart0 read32 unmapped 0x00000000`. The faulting instruction at
   `0x4001b9c0` is `lw a5,0(s2)` (`0x00092783`), with `s2 = 0` in the dump.
5. The UART heading says Core 1 / Cache error, but the dump identifies Core 0 /
   `MHARTID = 0`, `MCAUSE = 0x19`. Do not infer a CPU1 cache defect from that heading.

In the pinned source, `MetalioEInk4PlusBoard` returns early when EPD init fails,
leaving `display_ = nullptr`; `Application::Start` subsequently calls
`display->SetChatMessage`. This matches the null vtable load and the UART ordering.
Without the exact released ELF it remains a source-correlated inference, not a
symbolicated attribution. No invalid-MMIO response or firmware patch was added to
hide the error.

The official native runtime itself reports 22 ROM interception patches. Therefore
“unmodified release” means no changes by this probe; it does not mean interception-free
execution. Browser networking has no bridge; native used `user,restrict=yes`.
Firmware was fetched into RAM, hash-checked and never saved as a binary in the repo
or deployed website. Compact evidence omits board identifiers and full stack dumps;
raw hashes remain available to check the retained local diagnostic captures.

## Integration and licensing boundary

The public distribution and WASM package declare Apache-2.0. Its repository provides
release binaries, documentation and tooling; the full emulator core was not found
there at the pinned revision. Retain the upstream license if distributing its package;
do not describe this evaluation as a source port into our MIT core.

The JS wrapper exposes boot/run, registers and limited memory inspection, GPIO input,
UART and network frame APIs. It exposes no public SPI/I²C peripheral-attachment
callback or display-framebuffer API. Native `--i2c-dev` is a generic register-file
device; acknowledging addresses with it would not validate TCA9555 power gating,
TPS65185 sequencing, touch or the parallel EPD. It was not used to manufacture a
successful board probe. Adding it as an alternate browser backend would still need
a supported board-I/O contract and verified EPD output.

## Reproduce the baseline

Extract the official v0.51.0 WASM archive into an ignored output directory, verify
its SHA-256 from the source receipt, and set `S31_PKG` to its `www/pkg` directory.
Supply the firmware URL and expected SHA from the product receipt:

```sh
node tools/s31-esp-emulator-probe.mjs "$S31_PKG" "$S31_FIRMWARE_URL" \
  "$S31_FIRMWARE_SHA256" output/s31-probe 10
```

The tool stops at the first requested reboot or its bounded work/time limit and
writes diagnostic JSON/UART, not firmware bytes. The browser run used the same
published `WasmEmulator` API and SHA-checked `fetch(..., {cache: 'no-store'})`, with
50,000-instruction batches and the same first-reboot stop. The native command used
`--chip esp32s31 --firmware /dev/stdin --net user,restrict=yes --timeout 5s
--exit-on MCAUSE --log-color never`, passing the SHA-checked download through stdin.

## Own-core implementation stages

ESP-IDF v6.1 is fixed at `fff9895c82d744c7237be8847347bdd1b07c6643` in the source
receipts. Its S31 map differs materially from P4: HP SRAM `0x2f000000..0x2f080000`,
ROM `0x2f800000..0x2f850000`, LP SRAM `0x2e000000..0x2e008000`, flash
`0x40000000..0x44000000`, PSRAM `0x50000000..0x54000000`, UART0 `0x2038a000`.
S31 is not a P4 board alias or an S3 variant.

Reuse the already implemented RV32 integer/F execution (EX240), counter and interrupt
work (EX241) only after checking S31 headers; P4's SoC implementation (EX242) is a
reference for shared IP, not an address-map template. Current upstream PRs were
checked: #310 touches shared RV32 execution; no second S31/CLIC implementation was
found. Avoid mixing that optimization with this hardware work.

The next acceptance boundaries are a real SRAM-linked UART fixture, exact unsupported
ROM/MMIO faults, S31 reset/clock/CLIC/flash/PSRAM bring-up, and finally the product's
TCA9555/TPS65185/parallel EPD/touch/SD path. A serial fixture alone must not enable
the catalog's product runtime. Preserve the default official firmware and its remote
source while these stages remain incomplete.

### Initial in-tree subset (5850cb30)

`esp32s31::Bringup` is a separate, interpreter-only development entry point. It has
one HP hart, the S31 SRAM/LP SRAM/ROM windows, and polled UART0 using the common
FIFO implementation with fields checked against S31 `uart_reg.h`. The shared CPU
is configured for the RV32IMAFC subset and counter reads. Existing P4/S3/C3/C6
constructors and execution code are unchanged; no product board or platform chip
selector is enabled by adding this crate.

`firmware/s31-uart-probe` contains source for a SRAM-linked fixture that waits for
UART RX, echoes it and checks SRAM plus an FPU operation before reporting success.
Generated ELF/BIN files belong only in ignored output. The fixture assumes an
already usable UART clock; it does not prove clock/pin-mux setup or physical timing.
The ROM inspection loader accepts the S31 reset entry and preserves ROM instructions;
it omits the ELF's address-zero `.eh_frame` debugger metadata. ROM-side initializer
table reconstruction is still outstanding, so it is not a full ROM boot loader.

S31 v6.1 `interrupt_reg.h` sets `INTTHRESH_STANDARD = 1`: `mintstatus = 0xfb1` and
`mintthresh = 0x347` through `csr_clic.h`. The existing P4 rev < 3 path uses
`mintstatus = 0x346` and a mapped threshold register. Consequently the initial S31
subset deliberately leaves interrupt delivery unimplemented; enabling the P4 flag
unchanged would give S31 the wrong CSR contract. The same discipline applies to
MSPI/cache and clock/reset integration in the next stage.

Validation: the source-built fixture emits `S31 UART READY`, waits for host input,
then echoes `A` and emits `S31 SRAM FPU PASS`; a fresh run with `B` reaches its
failure/ebreak branch. The positive run retires 206 steps before WFI. The independent
public S31 ROM ELF from release 20260528 (SHA-256
`a52ad7513deb656a910a5740125f1cce2c7941f11ce57213b7b43aea93d5ab87`)
runs seven instructions before the first unmapped read: PC `0x2f801724`, address
`0x20701030`, `LP_AONCLKRST_HPCORE0_RESET_CAUSE_REG`. This ELF is not claimed
byte-identical to the official emulator's embedded ROM. These are starting points
for reset/CLIC bring-up, not evidence of full ROM or application startup.

Both Clippy targets passed, including denied warnings; 1,366 Rust tests passed with
the CI ignored-test policy, the separate source fixture passed, and the four required
platform test files and eight existing WASM workloads passed. The new crate also
checks for `wasm32-unknown-unknown`, although it is not exported through the platform
runtime. [Detailed receipt](evidence/s31-bringup/uart-subset.json).

The CPU guard compared the new workspace with main-at-branch
`20e75f722d41a8f95308ea18abc4457bc2dde345`, five alternating pairs after warmups.
Median child user CPU changed by S3 hello +0.49%, C3 +0.08%, C6 −1.79%; instruction,
cycle and exception/interrupt statistics and UART hashes were identical. These
small changes are not attributed to S31: the existing executable dependency graphs
are unchanged and the new crate is unused by them. No S3-core code changed, so
Pocket Tank was not required by CONTRIBUTING's S3-specific condition. A rejected
C3 command in the first harness attempt is retained separately; the corrected run
covered all three workloads. [Samples and commands](evidence/s31-bringup/uart-perf.json).

### POR state and standard CLIC

The next stage adds the two HP reset-cause registers, with the documented power-on
cause and write-triggered clear bits. This is a power-on state, not a watchdog,
software-reset or power-domain model. The old P4 `ClicCore` register/arbitration
implementation has moved unchanged into `esp-periph::clic`; P4 retains its existing
CSR addresses, threshold mapping and tests. S31 has a separate CPU constructor,
standard `mintstatus`/`mintthresh` CSRs, its own CLIC address windows and its own
interrupt matrix. S31's matrix requires a nonzero PASS_LEVEL: values 2/3 route to
machine mode; simply copying P4's ID-only mapping would leave routing disabled.

The modeled source is UART0 (source 9). The executing hart is still hart 0; hart 1's
control registers can be addressed but there is no second running CPU. The acceptance
fixture explicitly configures three interrupt level bits. Other priority partition
modes, supervisor-mode delivery and the rest of the source fabric are not validated.
The S31 bus does not invent P4's threshold-register alias at CLIC offset 8.

The real SRAM-linked IRQ fixture waits at WFI, receives `A`, takes hardware-vectored
CLIC ID 17, checks `mintstatus` inside its handler, drains and echoes UART RX, then
returns via `mret`. Main code checks that the interrupt level was restored before
printing `S31 IRQ MRET PASS`. A separate host-controlled threshold test leaves RX
pending while masked, observes no handler output, and then unmasks the same guest.
With RX preloaded, the complete positive run takes 232 steps. Generated fixture
ELFs remain local ignored test artifacts.

The same unmodified public S31 ROM now executes 6,978 steps before faulting at
`ets_stack_guard_setup`: PC `0x2f80cfea`, read32 `0x208140fc` (TRNG base
`0x20814000` plus `0xfc`). The exact ROM instruction is `lw a5,0(a4)`; it is followed
by a read-modify-write setting bit 28. IDF v6.1 `trng_reg.h` identifies this as
`TRNG_DATE_REG` and bit 28 as `TRNG_CLK_EN`. The next stage must implement the
documented TRNG and reset/clock sequence rather than return arbitrary values.
This advances the previous reset-cause fault; it does not establish full ROM boot,
flash execution, entropy quality, peripheral readiness or INK4 Plus compatibility.

Both Clippy targets pass; 1,371 Rust tests pass under CI's ignored-test policy,
along with both source fixtures, the four required platform test files and nine
existing WASM workloads, including P4 hello. Existing goldens are unchanged. The
first Clippy attempt found a test-only identity operation; the first full test run
lacked public P4 co-simulation fixture inputs. Both failures and their corrections
are retained in the [stage receipt](evidence/s31-bringup/reset-clic.json).

Five alternating CPU pairs against main `3c1737de` gave median user-time changes
of S3 hello −0.79%, C3 −2.68%, C6 +0.80%, with identical instruction/cycle/trap
statistics and UART hashes. These observations are not a speedup claim; code
layout and host load are not controlled beyond isolated timing and alternating
order. No S3-core code changed, so CONTRIBUTING's Pocket Tank condition does not
apply. [CPU samples](evidence/s31-bringup/reset-clic-perf.json).

### CRC random-data path

The following stage models the S31 TRNG CRC output path used by the public ROM
and the fixed IDF v6.1 `rng_ll.h`/`trng_ll.h`. Output requires the LP peripheral
bus clock (`0x2071003c`, bit 30), deasserted reset (bit 31), module clock
(`TRNG_DATE`, bit 28), sampling and noise-CRC enables. ROM reads `CRC_DATA`
at offset `0x44`; the IDF LL uses `CRC_SYNC_DATA` at `0x48`. Both are functional
sampling points in this subset, without synchronizer timing. Disabled gates hold
the previous sample; reset clears it and disables sampling. The register clock
control is retained across the sampling reset, matching the ROM sequence that
enables it before pulsing reset. This reset-domain behavior is inferred from that
public ROM path, not measured on silicon.

The noise source is the existing deterministic `esp-periph::rng::Rng` stream.
It provides reproducible functional input, not physical entropy, a modeled CRC
polynomial or cryptographic guarantees. Health tests, conditioning keys, request/FIFO
output and arbitration remain unmapped: code requiring those paths gets an explicit
unsupported-access fault. No test-result/ready bit is manufactured to bypass them.

The SRAM fixture executes real MMIO loads/stores and prints `S31 TRNG GATES PASS`
after checking gates, changing samples and reset, reaching WFI in 147 steps. The
same unpatched public ROM advances from the prior 6,978-step TRNG fault to 7,238
steps. It now faults in `clear_super_wdt_reset_flag`, PC `0x2f805a12`, read32
`0x2080101c`. IDF v6.1 identifies this as `RTC_WDT_SWD_CONFIG_REG`; the next ROM
write sets `SWD_RST_FLAG_CLR` bit 19. RTC watchdog timing and reset behavior remain
to be implemented. This remains an early-ROM probe, without application or display
startup, and does not change the product catalog's unavailable status.

Both Clippy targets, 1,373 Rust tests, all three source fixtures, the four platform
files and nine existing WASM workloads pass; goldens remain unchanged. The
production WASM SHA-256 and the native S3/C3/C6 `__TEXT,__text` sections are identical
to `804aa937`, as expected for a change confined to the unused development crate.
[Functional receipt](evidence/s31-bringup/trng.json).

The required five-pair CPU guard against main `3c1737de` observes S3 +0.35%, C3
−0.64%, C6 +2.20%, with exact guest-stat and UART matches. It reuses the prior
main binaries after checking their hashes and rebuilds the candidate binaries.
These timings include the preceding CLIC stage and host noise; they are not an
attributed TRNG slowdown or speedup. [Samples](evidence/s31-bringup/trng-perf.json).

### RTC watchdog counter and reset requests

The next subset adds RTC WDT configuration, write protection, feed, stage timeout,
interrupt status/clear, and typed CPU/system/RTC reset requests. IDF v6.1 `rwdt_ll.h`
requires stage 0's hold value to be multiplied by `2^(1 + WDT_DELAY_SEL)`; the model
accepts that eFuse field as an explicit input. Its initial zero represents an
unprogrammed model, not a measured product eFuse. Flashboot mode can count with
`WDT_EN` clear, but boot-mode logic must explicitly enter flashboot. Sleep/debug
pause inputs are separate. Guest-controlled values are bounded, including zero
timeouts and reserved stage actions.

`advance_rtc_ticks` supplies the clock domain explicitly. The runner does not convert
CPU instructions or host time into RTC time while the clock tree is missing. It
stops with a typed reset request; it does not yet execute the whole chip's reset
domains. The counter covers one finite four-stage sequence, restarted by feed or
enable, without claiming recurring-stage behavior. Super-WDT POR latch/clear and
configuration storage are modeled; missing-clock detection and super-WDT expiration
are not. The ROM clears the super reset flag before unlocking configuration, so
that flag-clear path is separate from protected configuration writes.

The real SRAM fixture arms stage 0 for an interrupt and stage 1 for system reset.
Seven slow-clock ticks leave the guest waiting; the eighth routes source 136 through
CLIC ID 18, executes the guest's handler and prints `S31 WDT IRQ PASS`. Two further
ticks do not reset; the third produces `Reset(System)`. This verifies the MMIO,
counter and guest ISR path under explicit clock input, not autonomous boot timing.

The unpatched public ROM reaches 7,245 steps, past `clear_super_wdt_reset_flag`,
before `Cache_Mask_All` reads `0x2c000000` at PC `0x2f80207a`. This is the S31 cache
controller base, not RAM. Cache controls/MMU and the later flash path remain
unsupported; no ROM hook or arbitrary MMIO return bypasses that fault.

Both Clippy targets, 1,377 Rust tests, four source fixtures, four platform test files
and nine existing WASM workloads pass. Goldens remain unchanged. The production
WASM and native S3/C3/C6 code sections match `d4310c9c` byte for byte. This stage
therefore cites that stage's existing CPU samples for the unchanged production
paths; it does not claim a new timing run or a performance result for the new S31
counter. [Functional evidence and code hashes](evidence/s31-bringup/rtc-wdt.json).

### Cache bus gates and external-memory MMU

The next standalone subset implements hart 0's instruction/data cache bus gates
and the separate Flash/PSRAM MMU index/content ports at MSPI0. There are 1,024
entries per table. The fixed v6.1 `mmu_ll.h` selects Flash pages of 256/128/64/32
KiB and PSRAM pages of 64/32/16/8 KiB. Valid, target and physical-page fields follow
`ext_mem_defs.h`; the two entry formats are different. Invalid entries, backing
memory bounds, unsupported sensitive/encrypted mappings and invalid table indices
produce explicit faults. Mapped Flash writes are prohibited while no SPI program
command engine exists. Reads and writes can cross noncontiguous mapped pages;
writes validate the full access before changing memory.

The host explicitly supplies development Flash bytes and PSRAM capacity. The
256 MiB/64 MiB allocation ceilings are software limits, not measured INK4 Plus
capacities. MMU table SRAM starts deterministically invalid; the register header's
CONTENT-port reset value of 892 does not establish the power-on contents of all
entries. This models coherent functional access, not cache lines, stalls, encryption,
cache-error interrupts or maintenance/lock operations. Debug/ECO control storage
does not emulate the corresponding tests. Only hart 0's external buses execute;
ROM-cache aliases, the second hart and DMA remain outside this stage. The direct
TCM ROM and SRAM windows do not pass through these external-memory gates.

The source-built SRAM fixture programs the MMU ports and executes a separate
Flash-mapped worker, which writes PSRAM and returns. The caller remaps the virtual
page, checks the other physical page is empty, restores the mapping and reads the
original value. It prints `S31 MMU FLASH PSRAM PASS` after 175 instructions.
Running the same program without Flash backing faults on instruction fetch;
insufficient PSRAM faults on the worker's store. Additional tests cover all page
sizes, closed instruction/data gates, encryption rejection, invalid indices,
discontiguous page boundaries and incomplete 32-bit instructions at a mapping edge.

The unpatched public ROM now reaches 7,255 steps. The next first fault is
`pmu_enable_unhold_pads`, PC `0x2f805a5c`, reading `PMU_IMM_PAD_HOLD_ALL`
at `0x2070410c`. PMU pad control remains unimplemented in this stage; this advance
does not establish Flash boot, execution of the Plus image, or display readiness.

Both Clippy targets, 1,381 Rust tests, five source fixtures, four platform test files
and nine existing WASM workloads pass. Goldens are unchanged. The five-pair CPU
guard observes S3 −1.47%, C3 −1.91%, C6 −0.24%, with exact guest statistics and UART
hashes. Existing production WASM and CLI code remain byte-identical to the prior
stage, so those timings are not an attributed speedup. The baseline binaries retain
their prior hashes; main's intervening changes contain no native CLI/core changes.
[Functional receipt](evidence/s31-bringup/cache-mmu.json),
[CPU samples](evidence/s31-bringup/cache-mmu-perf.json).

### Software GPIO and PMU global pad hold

The standalone S31 pad model adds the software GPIO output/enable banks, input
sampling, IO_MUX function/input/pull configuration, output inversion/open drain,
and the PMU's separate HP/LP hold/unhold strobes. The fixed v6.1 generated GPIO
header describes some banks up to GPIO66, but `soc_caps.h` and the driver support
GPIO0–61 and exclude unbonded GPIO29/41. This model follows that explicit capability
mask; it does not turn template register bits into extra accessible package pins.
GPIO0–7 form the LP pad group and GPIO8–61 the HP group.

Hold captures the output drive and input/pull configuration. Register writes while
held remain visible in the registers but do not change the held electrical state.
External input still changes while input is enabled, as documented by IDF's GPIO
driver API. Releasing one PMU group applies its current configuration without
releasing the other group. Contradictory simultaneous hold/unhold commands have no
documented priority and fault explicitly. Individual-pad LP_SYSTEM hold registers,
sleep/wakeup transitions, power domains and complete reset retention are not modeled.

Peripheral output routing is reported as unsupported instead of generating a fake
signal. GPIO interrupts/wakeup configuration is rejected in this subset; analog
behavior, drive strength and filtering are absent. Push-pull contention and
unresolved opposite pulls produce explicit faults. A disconnected floating input
resolves low as a deterministic model convention. The initial IO_MUX state is an
unconfigured development state, not a claim about the package's reset pulls or
functions. Sampled strap bits are a caller-supplied input, initially zero; they are
not changed to force a desired ROM boot mode. UART fixture output still uses the
existing UART development interface, without claiming routed TX pad timing.

The real source fixture verifies held GPIO10 output and held GPIO2 input-enable,
then accepts a host input change and UART `A`. Guest reads confirm live input and
the separately released HP/LP groups before `S31 PAD HOLD INPUT PASS`. Omitting
the input change reaches the guest's `S31 PAD FAIL`/`ebreak` branch. The initial
negative-test expectation incorrectly used `Exception(3)` rather than the
interpreter's explicit `Ebreak` stop; only that host assertion was corrected.

The unpatched ROM advances to 7,267 instructions. Its first unsupported access is
now `ets_efuse_config_wr_tim`, PC `0x2f80818a`, reading eFuse-controller
`EFUSE_WR_TIM_CONF2` at `0x207151f8`. No eFuse values, ROM instructions or official firmware bytes were
changed to pass the pad stage. Plus boot and display remain unavailable.

Both Clippy targets, 1,384 Rust tests, six source fixtures, four platform test files
and nine existing WASM workloads pass. Goldens stay unchanged. Five-pair CPU guards
observe S3 +0.65%, C3 −1.85%, C6 −0.81%, with exact guest statistics and UART hashes.
Existing production WASM/CLI code is byte-identical to the preceding stage; these
are guard measurements, not an attributed speed change for the new pad model.
[Functional receipt](evidence/s31-bringup/pmu-gpio.json),
[CPU samples](evidence/s31-bringup/pmu-gpio-perf.json).


## eFuse input, XTAL domain and ROM console initialization

The next standalone stage adds the actual eFuse read command path at `0x20715000`.
A decoded input image must be supplied explicitly: no MAC, key, revision, calibration
or product OTP is invented. Correct READ opcode `0x5aa5` transfers supplied words
and error metadata to read-only shadows, raises READ_DONE source152, and updates
the RTC watchdog delay selector from its documented one-bit field. Absent input,
programming, read-protected images and unsupported ECC/security operations fail
explicitly. Read completion is a zero-latency functional transfer, not OTP timing.
The fixed header's WDT_DELAY_SEL prose lists four legacy values while its actual
bitfield is one bit at19; this model follows the bitfield and records the conflict.

The HP clock root at `0x20587000` now models XTAL40 MHz, CPU integer/fractional
division and MEM/SYS/APB integer division. Configuration is staged until the real
UPDATE strobe; domain rates are derived from the active configuration. The v6.1 LL
identifies mux1 as CPLL and mux3 as PLL_F240M, superseding the generated register
header's older PLL_F300M prose. PLL/RC power/calibration are still absent, so those
sources are rejected rather than reported locked at a fabricated frequency.
No CPU-instruction-to-RTC or serial-bit timing relationship is claimed.

UART0 clock/reset `0x20587088` and memory-power control `0x2058628c` now affect
functional input, output and IRQ eligibility. Disabled bus/core clocks cannot pass
FIFO traffic. Core reset clears receive state; the APB module reset restores its
modeled configuration and FIFO. Already-transmitted bytes remain in the host capture.
The `uart_input` method respects these controls; direct access to `uart0` remains a
low-level debugger interface. Forced UART memory low-power entry, FORCE_NORST and
unmodeled UART clock sources are explicit unsupported operations. Serial framing,
flow control and physical pin transmission remain partial.

The ROM also releases individual GPIO58/59 holds. LP_SYSTEM's two HP hold words
(`0x20700270/274`) now map GPIO40..62 and GPIO8..39 respectively, intersected with
the package capability mask. Individual and global holds keep the original snapshot
until both are released; setting a second hold cannot accidentally resample a changed
output. LP per-pin hold, sleep retention and reset-domain behavior remain absent.

Source-built `efuse-probe.S` checks data transfer, RO behavior and READ_DONE clearing;
without an explicit image the same code faults on the command. `clocks-probe.S`
calls the real fixed ROM frequency routine, observes 40→20 MHz, then exercises actual
UART reset state and output. The real ROM boot probe reaches `usb_serial_device_tx_one_char`
and emits UART `E` before its next unsupported read at USB Serial/JTAG EP1_CONF,
`0x20391004`, PC `0x2f811cfa`. ROM instructions are unchanged and no eFuse image is
supplied by this default probe. This is a console-initialization checkpoint, not a
Plus firmware boot: Flash commands, remaining ROM RAM initialization/SoC blocks,
S31 custom ISA/second core and the Plus board peripherals still need work.

A separately retained official-emulator observation reads strap `0x12` and shows
its own ROM naming that input SPI_FAST_FLASH_BOOT. Its public read_memory API returns
zero for unmapped memory, and all sampled eFuse controls—including documented
nonzero reset registers—returned zero. Those observations do not identify real
Plus eFuse contents and are not copied into this model as a hardware profile.
See `evidence/s31-bringup/efuse-clocks.json` for source hashes, exact outcomes and limits.

## USB console packets and actual ROM initializer execution

The standalone USB Serial/JTAG EP1 subset now models the 64-byte TX FIFO, explicit
WR_DONE/automatic full-packet commit, RX packet consumption, and receive/empty/IN-token
raw interrupts. Source2 routes through the existing S31 matrix and standard CLIC.
The APB clock gates guest accesses and host traffic. Host packets are explicit inputs;
without a connected functional peer a full TX packet remains pending. There is no
fabricated periodic host polling, USB enumeration, PHY/JTAG/SOF behavior, reset or
48-MHz timing. Unsupported interrupt enables and register windows fault explicitly.
The example's `--usb-host` flag opts into a functional peer polled every 1000 requested
instructions; this quantum is not a USB frame or timing guarantee.

This exposed a separate defect in the initial ROM inspection loader. The public
20260528 ELF carries initialized RAM sections separately from its ROM segments. Its
actual `_init` copy table uses 12-byte `(RAM start,RAM end,ROM source)` records. Missing
ROM-side source bytes caused the guest copy loop to overwrite its own interface-table
pointer with zero, later trapping in the USB timeout path. The loader now reconstructs
only the 19 explicitly matched payloads (1232 bytes) from this same ELF. It checks all
bounds, alignment, table coverage and overlapping data; missing or conflicting data
fails before changing guest memory. No ROM instruction or pointer value is invented,
and no host code performs the guest's RAM initialization. A fixed-ROM test observes
zero at RAM `0x2f07fff4` before execution and the ELF's `0x2f84882c` value after the
real copy/clear loops complete in 6303 instructions at `0x2f801852`.

LP_SYSTEM's 16 scratch words at `0x2070002c..0x20700068` now retain actual guest writes,
with the header's POR-zero defaults. IDF `rtc.h` names STORE4 bit0 as ROM log control.
Sleep/reset-domain retention is still absent; the words persist only for the lifetime
of the current development bus. No default strap or product eFuse data was changed.

The source USB fixture sends bytes 0..63, waits for a real host drain, submits a ZLP,
then receives `A` through a guest ISR and emits an actual one-byte echo packet. With
no host the same guest remains in its full-FIFO poll; with CLIC threshold masked the
pending receive cannot execute the handler. Its completed ISR path prints
`S31 USB IRQ ECHO PASS`. The fixture and generated binaries are test-only, not
catalog firmware.

The unmodified fixed ROM now emits complete banners through both UART and three USB
packets. With explicit development strap 0 it reports USB_BOOT (85 bytes, 39232 steps);
with explicit strap 0x12 it reports SPI_FAST_FLASH_BOOT (97 bytes, 42580 steps). The
latter value is a caller-selected experiment, corroborated only by the earlier
upstream emulator observation, not a measured Plus strap profile. Both stop on the
same exact unimplemented read: BUS_MONITOR_CLOCK_GATE `0x2d002114`, PC `0x2f806c3a`,
inside `ets_clk_assist_debug_clock_enable`. This is a supported startup-console
boundary, not execution of the Plus flash bootloader or application.

Bus-monitor capture/reset semantics, the SPI flash command engine, clock/PLL/PSRAM
bring-up, the S31 ISA/second hart and Plus board devices remain outstanding. Product
status remains unavailable. The next stage follows the actual monitor and Flash
path; it must not replace unsupported reads with arbitrary success values.

Validation: both stable Clippy targets pass; all1396 Rust tests (131 groups),32 S31
unit tests,11 source/ROM tests, four platform Node files and nine existing WASM
workloads pass, with unchanged goldens. Production WASM is byte-identical to the
preceding stage. Five alternating hello pairs against freshly built main `a9375446`
show S3 −1.62%, C3 +1.12%, C6 −0.33%, with exact instruction/cycle statistics and UART
hashes. Main includes independent Guru/INK4 changes, so these are regression guards,
not an attributed S31 speed result. No S3 core changed in this stage.
[Functional evidence](evidence/s31-bringup/usb-rom.json),
[CPU samples](evidence/s31-bringup/usb-rom-perf.json).

## Real bus-monitor capture and bounded SPI1 Flash commands

The next standalone stage implements the bus monitor from fixed S31 v6.1 headers:
actual hart0 PC/SP sampling, stack minimum/maximum interrupts (matrix source119),
clock/reset gates and two-slot real exception metadata. The second hart remains
inactive and receives no fabricated samples. The source fixture creates a stack
underflow, takes the CLIC ISR, then performs actual unmapped load/store instructions;
its own trap handler returns with `mret` and checks the recorded causes and PCs.
The diagnostic runner stops at each exception; the test resumes at the architectural
vector without setting guest PC or capture data. Region watchpoints, memory-stall
monitoring, a debugger, recursive-handler lockup detection and reset-domain behavior
remain absent. Exception trace storage is not a claim that automatic lockup/reset
has been implemented.

SPI1 now has a bounded synchronous NOR command subset: RDID, RDSR, WREN/WRDI,
24-bit ordinary/fast/dual/quad reads into the 64-byte receive buffer. It requires
an explicit caller-supplied NOR ID/status and Flash array. No identity or QE bit is
inferred from the INK4 Plus firmware, and no default Flash die is installed. APB/core
clock and reset gates, CS0 selection, transfer bounds and supported phase lengths
are checked. Unsupported commands, write/erase, SFDP, OPI, security, power-down,
suspend/resume and unsupported clock sources fail explicitly. This models completed
functional transfers, not SPI wire timing or contention. Existing `SpiMem` array
operations are reused behind these checks; its permissive unknown-command and PSRAM
fallbacks are not exposed. RDSR captures the external status only on that command.

SPI0 retains documented configuration and exposes actual empty FIFO state because
this subset does not queue AXI requests. External Flash access requires the AXI
interface enabled/open and its clocks/reset released. The existing MMU source
fixture now explicitly opens that interface before fetching its source-built Flash
worker. This does not initialize a real product or replace ROM boot.

`mspi-probe.S` checks its explicitly supplied test ID, WREN/RDSR/WRDI, an ordinary
03h read and dedicated read against independent source data. Changed data takes the
guest's failure branch; missing ID/array or a disabled clock faults before any
command completes. These fixture inputs are unrelated to the physical Plus BOM.

### INK4 Plus official image, through the real ROM

The exact official 2.0.68 image was fetched again into host RAM and passed through
stdin to the standalone S31 ROM runner. SHA-256 matches the catalog; its bootloader
at0x2000 and application at0x80000 both have chip ID32, valid segment XOR checksums
and valid appended SHA-256 digests. The partition table at0x8000 was parsed. The
firmware was not written to disk, Git or a server. No ROM/BIN opcode or boot entry
was changed, and no application-direct execution was used.

With the explicit experimental strap0x12 and USB packet peer, the ROM prints its
97-byte SPI_FAST_FLASH_BOOT banner and first fails after43162 steps at
`SPI_init`, PC`0x2f80ebf6`, read32`0x20500028`. No Flash command has completed;
the bootloader, application and display have not run. This same image must be
retested at subsequent stages. [Image and guest receipt](evidence/s31-bringup/monitor-plus-2.0.68.json).

The next barrier is a concrete ROM/header discrepancy: the fixed public ROM reads
and updates the low10 bits at SPI0 offsets0x28 and0x24, while the fixed v6.1
`spi_mem_c_struct.h` names those two words `reserved_024[2]`; only SPI1's matching
offsets are documented as MISO/MOSI data lengths. These accesses are deliberately
still unsupported. Returning zeros merely to pass initialization would not establish
the real SPI0 behavior. The USB_BOOT branch separately reaches the still unsupported
HP_SYS USB control at`0x205870ac`, PC`0x2f8152d4`, after46351 steps/85 output bytes.
Neither branch establishes the physical product's strap or OTP contents.

Reproduce the official-image probe with caller-selected public source/hash:

```sh
cargo build --release -p esp32s31 --example bringup
python3 tools/s31-native-firmware-probe.py \
  --firmware-url "$S31_FIRMWARE_URL" --sha256 "$S31_FIRMWARE_SHA256" \
  --rom output/s31-sources/roms/esp32s31_rev0_rom.elf \
  --output output/s31-plus-current.json
```

This is development instrumentation, not a website loader or a successful product
runtime. Plus remains unavailable; PLL/PSRAM initialization, custom ISA/second hart,
real Flash boot and the TCA9555/TPS65185/parallel EPD/touch/SD board path remain open.

Validation: both stable Clippy targets, all 1399 workspace Rust tests (131 groups),
36 S31 unit tests, 13 source/ROM tests, four platform Node files, nine existing WASM
workloads, own S31 wasm32 compile and native binaries pass. Goldens are unchanged.
Production WASM and the three native code sections match the previous stage byte
for byte. Five alternating hello pairs versus freshly built main `84e67571` give
S3 +0.142%, C3 +0.023%, C6 −0.603%, with exact guest statistics and UART hashes.
Main contains independent changes; these are regression guards, not S31 speed claims.
[Functional receipt](evidence/s31-bringup/monitor-mspi.json),
[CPU samples](evidence/s31-bringup/monitor-mspi-perf.json).

## Flash/cache ROM bring-up (explicit development inputs)

This stage keeps the strict default failure above. `--rom-spi0-shadow` explicitly
allows two stateful full-word shadows at SPI0+0x24/+0x28. The public ROM uses their
low ten bits; both the initial and current public IDF SPI0 struct mark them reserved.
A source-built guest on Espressif emulator0.51.0, with the same public ROM ELF,
reads `0xff` after ROM startup and round-trips `0xa55a03ff`. Reset-time GDB reads
return zero for these words **and for the documented nonzero SPI0 CTRL reset**.
That is upstream compatibility evidence, not silicon reset evidence. There are
22 upstream ROM interception patches. Our opt-in adds no opcode patch, ready flag,
transfer data or automatic product identity. With it off, these addresses still fault.

The documented v6.1 subset now handles cache-domain clock/reset gates, synchronous
invalidate/clean/writeback over the existing direct-memory model, mode0 freeze,
Flash/PSRAM MMU software light-retention controls and SPI0 read-mode configuration.
There are no retained dirty cache lines or speculative fills: maintenance finishes
only for valid selected domains, freeze blocks further memory misses, and disabling
it restores translation. Preload, autoload and lock remain at their documented
idle reset states; attempts to enable these unsupported operations fault. Freeze
mode1 (forcing a cache hit), deep/shutdown retention, cache-error interrupt recovery
and physical cache timing are not claimed. For MMU low-power mode the header field
reset is0 while its prose calls2 the default; this stage follows the field declaration
and does not infer a physical power-down sequence from that conflicting annotation.

SPI1 now has functional NOR deep-power-down/release state. APB reset preserves the
external die's state; a sleeping die cannot supply reads or cached Flash access.
The controller RD_STATUS latch is independently writable and RDSR replaces only
its status field; writing the latch does not modify external NOR status. Electronic
ID through RESANDRES, status writes, erase/program and wire/wakeup latency remain
unsupported. `--nor-jedec=ef4018` is an explicit development input matching the
existing source fixture, with status0. It is **not an identified INK4 Plus NOR** and
is never installed by the product catalog or normal runtime.

Using both explicit options with the unchanged official Plus2.0.68 full image,
the real ROM initializes its Flash MMU table, performs6 MSPI commands and reads the
boot header. Its actual output now includes `SPI mode:DIO, clock div:1`.
It next fails at `ets_sha_enable`, PC`0x2f80c288`, read32`0x20587058` after58978
instructions. This is the unimplemented crypto clock/reset block before bootloader
SHA verification; neither bootloader entry, app entry nor display is reached.
The same complete image remains in RAM only. The next stage must implement actual
SHA register/block computation and clock/reset behavior, not bypass image verification.

```sh
python3 tools/s31-native-firmware-probe.py \
  --firmware-url "$S31_FIRMWARE_URL" --sha256 "$S31_FIRMWARE_SHA256" \
  --rom output/s31-sources/roms/esp32s31_rev0_rom.elf \
  --rom-spi0-shadow --nor-jedec ef4018 \
  --output output/s31-plus-flash-cache.json
```

[Full-image result](evidence/s31-bringup/flash-cache-plus-2.0.68.json),
[functional/source evidence](evidence/s31-bringup/flash-cache.json).

Flash/cache validation:1402 workspace Rust tests (131 groups),39 S31 unit tests,
15 source/ROM tests, both Clippy targets, four platform Node files and nine WASM
workloads pass. Production WASM/native code sections match7d7c229f byte for byte.
[Five-pair CPU samples](evidence/s31-bringup/flash-cache-perf.json) versus fixed
main84e67571 show S3+1.4504%, C3+0.2474%, C6+0.7295%, with identical UART/statistics;
these are regression guards, not attributed S31 runtime overhead or speed claims.

## SHA block engine and first official bootloader entry

The standalone S31 bus now reuses the shared SHA compression routines behind the
S31 register map at0x20509000 and CRYPTO_CTRL0 at0x20587058. Fixed v6.1 `sha_reg.h`
and `esp_hal_security/esp32s31/include/hal/sha_ll.h` define the message/digest word
ordering and the parent/DS/HMAC/ECDSA resets that hold SHA in reset. This subset
supports synchronous register-driven SHA1/SHA224/SHA256/SHA384/SHA512 blocks,
with clock/reset gates and S31's16-bit DMA count field. Unsupported algorithm
modes, DMA transfers/interrupt enable, SHA3/SHAKE/SM3 and alternate clock roots
fault instead of falling into older shared-model defaults. There is no SHA timing
or DMA model. The enclosing security clock/reset gate follows the register grouping;
it has no independent silicon validation. Other crypto blocks remain absent.

The unchanged official INK4 Plus2.0.68 merged image now loads all three bootloader
segments through the real ROM. The guest computes347 SHA blocks, emits
`entry 0x2f06a2ba`, and executes bootloader code. The first missing access is then
PC`0x2f06a476`, read32`0x20587040` (HP_SYS MODEM clock gate), after199409 instructions.
This is the first actual bootloader entry in this core; it is not application entry,
board initialization or a working display. It still requires the explicit SPI0
shadow/test NOR/strap/USB development inputs above. Plus remains unavailable.

SHA mismatch behavior is deliberately not strengthened by the host. A source-built
ROM boot fixture prints its expected output and reaches WFI with five real SHA
blocks. Flipping only one appended-digest bit makes the ROM print its independently
calculated digest, the changed expected digest and `Attempting to boot anyway...`,
then execute that fixture. This is the public ROM's unsecured-boot behavior, **not**
a host bypass or an assertion that a mismatched image is rejected. Flipping a payload
byte instead produces the real XOR-checksum error and ROM assertion; no guest entry
or fixture output occurs. The official image has neither warning nor mismatch.
No third-party BIN, ROM opcode or requested boot entry was altered to obtain success.

[Official full-image result](evidence/s31-bringup/sha-plus-2.0.68.json),
[SHA vectors, source ROM controls and checks](evidence/s31-bringup/sha-boot.json).

SHA-stage validation:1404 workspace Rust tests (131 groups),41 S31 unit tests,
16 source/ROM fixtures, both Clippy targets, four platform Node files, nine WASM
workloads and both native/WASM builds pass. All three production native binaries
and the production WASM match the just-timed Flash/cache build byte for byte;
that hash-linked CPU comparison applies to the identical binaries. No golden or
platform UI changed, and no separate browser or hardware success is claimed.


### Bootloader analog-I²C and MPLL shutdown subset

The unchanged official INK4 Plus 2.0.68 merged image now executes its real
`bootloader_hardware_init` path after ROM entry. This stage adds the documented
MODEM bus gate, analog-I²C master enable/force/source controls, two independently
routed byte-transaction masters, and the documented BIAS register0/register1
storage. It also adds PSRAM XTAL clock/reset gating for actual mapped reads,
writes and fetches, plus the narrow MPLL shutdown sequence used before changing
bias. These controls are private to the S31 crate; no production board or product
availability is changed.

The digital register addresses, fields, route polarity and reset values come from
fixed ESP-IDF v6.1 `fff9895c` headers and `regi2c_impl.c`. The BIAS target's analog
reset values are **not public in those headers**. Default execution therefore
rejects an unseeded target instead of assuming zero. The example's explicit
`--analog-bias=0000` input supplies register0/register1 test bytes; it is not an
identified Plus silicon profile. Other slave/register addresses, burst transfers,
analog settling, voltage calibration, RF operation and PLL power-up/lock remain
unsupported. The master returns an idle state only after a real modeled byte read
or write completes. A digital-master reset clears controller state but preserves
the separately supplied analog target bytes; this target/controller separation is
a functional modeling assumption, not a hardware measurement.

With no BIAS seed, the same image faults on its actual first target read request:
PC `0x2f0702a0`, write32 `0x2010f804`, after 199,507 instructions and zero completed
analog transactions. With explicit `0000`, the firmware performs four actual
read/modify/write transactions, leaves BIAS bytes `a0/0a`, and completes its MPLL
shutdown sequence. The next access faults at PC `0x2f06a512`, read32 `0x2070201c`
(`LP_ANA_FIB_ENABLE_REG`) after 199,633 instructions. Application and display are
still not entered; the platform continues to mark INK4 Plus unavailable.

The source-built `analog-probe.S` tests independent host bytes `17/29`, guest
reads and writes, both master routes, controller reset preserving target data,
and non-latching MPLL shutdown command bits. It prints
`S31 ANALOG ROUTE RESET MPLL PASS` and reaches WFI in 252 instructions with eight
completed byte transactions. The identical guest fails on wrong data (111 steps),
missing target data (19), and a disabled parent bus clock (7). Unit tests also
verify that force-on cannot substitute for master enable, missing/disabled routes
do not mutate the target, and PSRAM clock/reset gates block real memory access
without erasing external storage.

See the [stage receipt](evidence/s31-bringup/bootloader-analog.json) and
[official full-image trace](evidence/s31-bringup/bootloader-analog-plus-2.0.68.json).


Analog-stage validation: both warnings-denied Clippy targets, 1,409 workspace
Rust tests under CI's ignored-test policy, 45 S31 unit tests, all 17 external
source/ROM fixtures, the four platform Node files, nine existing WASM workloads,
production native/WASM builds and the private S31 wasm32 check pass. Goldens and
UI are unchanged. No browser or hardware compatibility result is inferred from
these native/source checks. The initial relative fixture-path failure and test-
module ordering lint are retained in the stage receipt with their corrections.

The official image was also run with BIAS seed `ffff`: its masked writes leave
`af/fa`, preserving the unrelated nibbles and reaching the same BOD/FIB access.
The two CI test names containing `external_` (one new PSRAM case and an older MSPI
case) were corrected before delivery, and both now run under CI's skip filter.
Five alternating CPU pairs against fixed main `84e67571` (production source paths
unchanged through main `f14d5634`) gave S3 −0.842%, C3 +0.474%, C6 +2.062%, with
identical guest instruction/cycle statistics and UART hashes. This is a regression
guard, not an attributed S31 speed result; see [samples](evidence/s31-bringup/bootloader-analog-perf.json).

### Bootloader BOD and PERIF power/reset subset

The same unmodified Plus 2.0.68 full image now executes its BOD mode1 enable,
internal PERIF detector configuration and super-watchdog auto-feed, then stops at
PC `0x2f06a6f8` writing `0x20706000` (`LP_PERI_PMS_LP_SYSREG_CTRL_REG`). This is
permission-controller initialization, not an application or display entry. INK4
Plus remains unavailable in the product platform.

This stage adds the fixed v6.1 LP_ANA FIB, mode1 reset-enable and glitch-reset
configuration words. BOD mode1 accepts an explicit **already-qualified comparator
event**, requires software control (FIB bit1 clear), and emits a typed Brownout
reset request only when enabled. The runner stops before executing another
instruction; it does not invent a physical voltage, comparator threshold, reset
pulse, reset-domain retention or automatic reboot. Unknown eFuse/FIB-controlled
policy is rejected. Mode0 interrupts and physical glitch detection remain absent;
retaining their configuration fields is not evidence of analog behavior.

PERIF/SARADC slave `0x69` register10 is separately routed through the real internal
I²C byte-transaction path. Access requires PMU XPD30 and reset release31 plus the
existing master/parent clocks and slave route. The explicit
`--analog-perif-glitch=hh` input supplies its unknown analog initial byte. Power
loss or reset returns this byte to that caller-supplied profile; a master-only
reset preserves it. This is a functional reset-domain assumption, not measured
silicon retention. Other SARADC registers, ADC conversion and detector calibration
remain unsupported.

Without a PERIF seed, the official image stops at its first target access after
199,720 instructions, four BIAS transactions and no PERIF transaction. With `ff`,
the real driver clears register10 bits0,2,3 and preserves the other bits, producing
`f2`; with `00`, the same writes leave `00`. Both perform ten total analog
transactions and reach the same permission-controller fault after 199,980
instructions. All earlier explicit NOR/BIAS/SPI0 development inputs remain
required and do not identify the actual Plus hardware profile.

The source-built `power-probe.S` independently checks PERIF `f7 → f2 → reset → f7`
through guest commands, configures BOD, prints `S31 PERIF RESET BOD ARMED`, and
waits after 193 instructions. A qualified host event then produces Brownout only
for the enabled case; disabled/no-event controls stay waiting. Wrong target data
executes the guest's failure branch; a missing seed traps before any transaction.
See [stage receipt](evidence/s31-bringup/bootloader-power.json) and
[official probes](evidence/s31-bringup/bootloader-power-plus-2.0.68.json).

Power-stage validation passes both warnings-denied Clippy targets, 1,411 workspace
Rust tests under CI's ignored-test policy, 47 S31 unit tests, 18 actual source/ROM
fixtures, the four platform Node files, nine existing WASM scenarios, production
native/WASM builds and the private S31 wasm32 check. No golden, UI, product model
or availability changed. Five alternating CPU pairs versus fixed main84e67571
(production paths unchanged through f14d5634) gave S3 +0.588%, C3 −0.050%, C6
+1.633%, with identical UART hashes and instruction/cycle statistics. This is a
production regression guard; no S31 speed or silicon claim is inferred.

### Bootloader peripheral-permission initialization

The unchanged official Plus 2.0.68 image now executes all 99 LP/HP peripheral
permission writes from fixed v6.1 `bootloader_mem.c` (the upstream temporary
all-security-modes bring-up policy, IDF-14620). It then reaches clock setup and
faults at PC `0x2f06fd88`, read32 `0x20701000`
(`LP_AONCLKRST_ROOT_CLK_CONF_REG`), after 200,488 instructions. This does not enter
the application or display; Plus remains unavailable.

The private S31 bus now has the three documented PMS control arrays, their
reset TEE read/write permissions, write locks, date, automatic-clock policy and
bus-error configuration. Permissions actually guard the connected UART, USB,
SHA, cache/MSPI, clock, PMU, LP scratch, eFuse, TRNG, watchdog, debug-monitor and
PMS-register accesses. A denied operation cannot mutate the target FIFO/storage.
The single modeled HP CPU is in its reset TEE security domain; REE/master-mode
transitions, DMA security and APM memory-region filters are not represented.
Control words for unimplemented peripherals are retained without creating those
peripherals. The separate MODEM/CLIC and GPIO permission ownership is unconfirmed
and is not silently assigned to a neighboring controller.

Restricted TEE profiles require the documented bus-error response in this
subset. The public header says the reset setting suppresses that error response,
but does not establish blocked-read data or stall semantics; attempting to
install a restricted profile with quiet responses is therefore explicitly
unsupported and leaves the control word unchanged. PMS interrupt enable and
exception-record registers remain unsupported. A locked control ignores later
writes, including an attempted unlock. No complete TEE/security certification is
claimed.

The source-built `pms-probe.S` sends `A`, installs and locks caller-supplied UART
permissions, attempts to reopen the locked word, reads actual UART status and
then sends `B`. The allow case emits `AB` and waits in 21 instructions; write-
denied and read-denied cases stop at the actual UART store/load in 20/18 steps
and leave output at `A`. Unsupported quiet-denial configuration stops in 13 steps
before installing the restriction. Bus tests additionally prove denied LP scratch
and SHA accesses do not mutate the target, and PMS self-protection has no bypass.
See [stage receipt](evidence/s31-bringup/bootloader-pms.json) and
[official trace](evidence/s31-bringup/bootloader-pms-plus-2.0.68.json).

PMS-stage checks pass both warnings-denied Clippy targets, 1,413 workspace Rust
tests, 49 S31 unit tests, 19 real source/ROM fixtures, the four platform Node files,
nine WASM scenarios and production/private-target builds. Five alternating CPU
pairs against the same fixed main84e67571 baseline gave S3 +0.077%, C3 −0.403%,
C6 +2.198%, with identical instruction/cycle statistics and UART hashes. This is
a production regression guard, not an S31 speed claim. No golden, UI or product
availability changed; no browser or hardware verification is claimed.

### Bootloader CPLL and low-power clock initialization

The same unmodified Plus 2.0.68 complete image now reaches its real second-stage
bootloader log: `ESP-IDF v6.1-dirty 2nd stage bootloader`, build timestamp and
`Multicore bootloader`. The private core has executed the guest's CPLL power,
320 MHz feedback/reference configuration, calibration start/poll/stop, and clock
latch. The resulting CPU/MEM/SYS rates are 80 MHz and APB is 40 MHz. It then stops
at PC `0x2f06d1a8`, write32 `0x20501000` (Flash SPI1 command, prohibited), after
283,521 instructions. The application and display have not started; Plus remains
unavailable. This log is a later checkpoint, not complete firmware compatibility.

The subset follows the pinned v6.1 clock LL and register headers. CPLL output is
computed from XTAL40 MHz × feedback/reference. Completion requires actual power,
a start command, the supported 320 MHz divider and 16 functional instruction
quanta. That delay is a deterministic model scheduling choice, **not measured
silicon settling time**. No analog PLL waveform, RF clock, peripheral PLL or
sleep transition is inferred. RO calibration-END writes cannot inject completion;
STOP cancels a pending calibration but retains an already completed result.
Invalid dividers and missing power/start never produce ready. Changing the active
CPU's divider/calibration or removing its clock is explicitly rejected before
mutation because suspended-clock execution is not modeled.

LP clock reset selectors and the RO 40 MHz XTAL field are separate from the
existing reset-reason registers. RTC_FAST can select XTAL or the functional
RC_FAST gate; removing FOSC power makes the latter's rate zero. The nominal
17.5 MHz / 136 kHz RC_FAST/RC_SLOW values come from `clk_tree_defs.h`; some older
LL identifiers/comments call RC_FAST 20 MHz. These are not measured clocks,
calibration or automatic CPU-to-RTC conversion. XTAL32K, force-on overrides and
sleep power transitions remain unsupported. Reset-disabled PMU sleep and BOD
mode0 interrupt cleanup is accepted; enabling those missing event sources is
rejected. BOD mode1's existing qualified comparator input remains separate.

DIG slave `0x6d` registers5/7/13/14 now use the existing real internal-I²C data and
routing path. Public headers provide fields but no analog reset bytes, so
`--analog-dig=hhhhhhhh` is an explicit development input in register order. With
`ffffffff`, the unchanged guest writes leave `ff/ff/f3/80`, preserving unrelated
bits. No seed still fails at the first DIG access. Earlier explicit NOR identity,
BIAS/PERIF bytes and SPI0 legacy compatibility inputs retain their documented
limits; none establishes the physical Plus hardware profile.

`cpll-probe.S` executes the documented sequence and calls the fixed real ROM
frequency reader. It prints `S31 ROM CPLL80 DIG PASS` in 244 instructions, reads
80 MHz and updates DIG through three real transactions. Missing power, invalid
divider and no-start controls execute its own timeout path and remain at XTAL40.
Wrong DIG input executes the failure branch; absent DIG data and active CPU-clock
removal trap at their real stores. Unit tests cover atomic clock latching,
calibration cancellation/power loss, target routing, RO fields and disabled IRQ
cleanup. See [stage receipt](evidence/s31-bringup/bootloader-clocks.json),
[official probes](evidence/s31-bringup/bootloader-clocks-plus-2.0.68.json) and
[production CPU guard](evidence/s31-bringup/bootloader-clocks-perf.json).

Clock-stage validation passes both warnings-denied Clippy targets, 1,418 workspace
Rust tests, 54 S31 unit tests, 20 real source/ROM fixtures, four platform Node
files, nine existing WASM scenarios and production/private-target builds. Five
alternating CPU pairs against fixed main84e67571 gave S3 +0.026%, C3 +1.042%,
C6 −1.805%, with identical output hashes and instruction/cycle statistics. This
is a regression guard, not an S31 speed claim. No golden, UI or product
availability changed; no browser or physical-hardware verification is claimed.

### Flash parameter read, clock selection and idle Resume

The same complete Plus 2.0.68 image now prints its actual bootloader Flash
configuration: 80 MHz, DIO and 16 MB. It stops after 362,095 instructions at
PC `0x2f06a378`, read32 `0x2070103c`, the unimplemented HPCORE0 reset-control
register. The app and display have not started; Plus remains unavailable.

The previous SPI1 stop is the bootloader's SFDP read at address `0x10`, used by
the pinned IDF driver to select its XMC startup path. The private model accepts
the real command8/address24/dummy8 read and returns only a caller-supplied byte
interval. Missing data, reads outside that interval and malformed transfers
fault without changing the receive buffer. SFDP is independent of NOR array
contents and JEDEC identity, and survives a digital-controller reset. No
complete manufacturer table or Plus BOM is inferred. The development probe
supplies `--nor-sfdp=10:ef`; this is an explicit non-XMC test input, not a physical
measurement of the product.

The guest then selects the actual Flash clock mux and divider. The pinned S31
MSPI LL assumes SPLL480 MHz and divides it by six for the supported 80 MHz
core. `--bbpll-hz=480000000` supplies that stable source explicitly; default
construction has no BBPLL input. The MSPI PLL enable, core enable, APB/reset
controls and HP_ALIVE BBPLL output gate determine access. Removing the source
or gate blocks commands and mapped Flash access. This digital clock model does
not establish analog PLL startup, settling or hardware timing. The functional
XTAL path applies the documented divider field, but its resulting rate has not
been compared with physical S31 hardware; no timing claim uses it.

The explicit Winbond `ef4018` descriptor accepts Resume `0x7a` only for the
modeled idle, unsuspended state. Winbond-authored W25Q128JV RevF/RevI section
8.2.20 specifies that idle Resume is ignored. It leaves array bytes, receive
buffer, WEL and QE unchanged; it does not manufacture completion of an erase or
program. Unsupported suspend states, another manufacturer, malformed phases or
a missing clock remain failures. The manufacturer-hosted latest PDF returned
403; the fixed older manufacturer-authored PDFs were fetched from public
distributor mirrors, with that provenance retained in `sources.json`.

`flash-params-probe.S` validates two distinct SFDP reads, Flash80 selection and
idle Resume through actual guest instructions. It emits `S31 SFDP FLASH80 IDLE
PASS`; wrong bytes execute its own failure branch. Missing SFDP, missing BBPLL,
ALIVE gate removal and an unsupported manufacturer trap at their guest
operations. This fixture is source-only in Git; generated binaries remain in
ignored output. It is not substituted for the official image.

The final `SFDP[0x10]=0x20` negative control enters the guest's XMC-specific startup
and stops on its unsupported `0xb9` command after 296,610 instructions. The
standard input reaches the reset-register stop above; both outcomes are
retained in the [official-image receipt](evidence/s31-bringup/flash-parameters-plus-2.0.68.json).
See the [stage receipt](evidence/s31-bringup/flash-parameters.json) and
[CPU guard](evidence/s31-bringup/flash-parameters-perf.json).

Validation passes 1,420 workspace Rust tests, 56 S31 unit tests and 21 source/ROM
fixtures, both warnings-denied Clippy targets, and the local CI commands for
WASM/JIT, platform/recording, VQ, worker/timing/BLE and receipt contracts. This was
an arm64 macOS host, not the Linux CI runner. Five alternating pairs against
fixed main84e67571 yielded S3 −0.126%, C3 −0.293%, C6 −0.979%, with identical
UART hashes and instruction/cycle statistics. These small differences are not
a speedup claim. No UI, golden or product-availability change is included, and
no browser or physical-hardware validation is claimed for S31.


### HP reset receivers, timer-group watchdogs and RTC snapshots

The unmodified **INK4 Plus 2.0.68** full image now reads the real partition table,
selects OTA0, verifies/loads all six application segments and logs
`Loaded app from partition at offset 0x80000`. It stops after **30,015,263
instructions** in ROM at **`mepc=0x2f80e4be`**, writing SPI1 CMD
**`0x20501000`** with **`0x01020000`**. The strict Flash subset rejects this
unsupported command. **The guest has not entered its application, produced a
framebuffer or made INK4 Plus usable.** The clocked probe uses the same fixed
full-image SHA-256; its firmware bytes exist only in host/guest RAM and a pipe.
No app-direct loader, patched instruction or substitute firmware is used.

The preceding blockers were functional hardware gaps. Each HP-core reset
control now implements the LP-WDT receiver gate, write-triggered software reset
and lockup gate. A lockup input must be explicitly qualified by a caller; the
RV32 core does not yet classify nested traps as hardware lockup, and ordinary
exceptions never trigger it. Disabled CPU watchdog receivers allow a later
system-reset stage to proceed. Reset requests stop the private runner and name
the affected cores/cause; they do not claim reconstructed reset domains, pulse
width, second-hart execution or an observed hardware reboot. The IDF header's
STAT_VECTOR prose contains P4 addresses, which are not used as S31 mappings.
Alternate vectors and stall/debug controls remain unsupported.

Both TIMG watchdogs have their actual S31 register/clock windows, protected
configuration, atomic update latch, prescaler, feed, four-stage actions and
separate IRQ status. Sources27/30 reach CLIC and a real guest ISR/mret in the
source fixture. APB gates register access; the independent WDT gate pauses its
counter. Explicit XTAL input determines counter boundaries. No automatic
Flashboot timeout, GP timer or calibration-ready placeholder is installed;
requests to advance the unsupported Flashboot mode fail explicitly. A
four-stage run stops after its final stage or a typed reset request; repeating
hardware reset sequencing is outside this subset.

RTC_TIMER now has a 48-bit counter, paired latched snapshots, write-triggered
update-done events, independent HP/LP raw-status clearing, overflow and digital
reset. A read alone does not create a snapshot or return a fabricated ready
flag. Alarm enable, interrupt enable, wakeup, REGDMA and sleep/stall features
remain unsupported rather than pretending to fire. Its MMIO and the two TIMG
windows pass through the real PMS permissions and require aligned word access.

The development option **`--rtc-timer-from-cpu`** advances RTC_TIMER using the
interpreter's cycle counter, active CPU clock rate and nominal 136 kHz RC_SLOW.
It is off by default. The RV32 interpreter counts one cycle per instruction;
this is an explicit functional timing approximation, not measured S31 latency.
Fractional ticks are retained across runs and clock changes (rounding at a
clock change is less than one divided by the new CPU frequency, in RTC ticks).
The option does not silently advance watchdogs or implement a complete clock
scheduler. Manual source fixtures retain explicit `advance_rtc_ticks` and
`advance_mwdt_xtal_ticks` inputs. WFI does not fast-forward wall time.

Reproduce the full-image result by adding `--rtc-timer-from-cpu --steps 100000000`
to the prior fixed-image probe command. The bounded runner now accepts up to
100 million instructions so the guest can finish its real multi-megabyte image
verification; it does not skip SHA work. The 2-million and 10-million negative
budget observations remain retained separately. See the
[official-image receipt](evidence/s31-bringup/bootloader-reset-plus-2.0.68.json),
[functional/source checks](evidence/s31-bringup/bootloader-reset.json) and
[production CPU guard](evidence/s31-bringup/bootloader-reset-perf.json).


Performance evidence qualification: the long-lived S31 branch includes an older
set of product implementations than fixed main84/f14d. Earlier main comparison
ratios therefore include unrelated branch differences; they cannot isolate S31
causal overhead. Original samples and hashes are retained unchanged in the
[qualification record](evidence/s31-bringup/baseline-qualification.json). This
stage additionally compares against an independently rebuilt fixed59a91211
with the same production sources. Integration on main still requires its own
before/after check.


Final validation: 1,427 workspace Rust tests (ignored included), 63 private S31
unit tests and all24 source/ROM fixtures pass, as do both warnings-denied
Clippy targets and all local CI command groups. No golden or UI changes and no
silicon verification. The same-production-source five-pair CPU guard measured
S3 -0.850%, C3 +0.084%, C6 +1.270%, with identical UART/instruction/cycle checks;
these small noisy differences are not a speedup claim. Its
[raw numeric samples](evidence/s31-bringup/bootloader-reset-same-source-perf.json)
are separate from the qualified cross-branch main comparison.


### NOR writes and the first official application instructions

The same unmodified **INK4 Plus 2.0.68** full image now completes its OTA metadata
write, logs `Set actual ota_seq=1 in otadata[0]`, and **executes the application's
entry at `0x2f0009b6`**. An optional diagnostic observer records successful
retirement at that PC, once, with initial cycle **30,065,117**. It observes
execution without rewriting or redirecting it. This is stronger evidence than
an image-loaded log, but **the application still stops during early startup and
has not produced a display**; the Plus catalog remains unavailable.

The next failure is at ROM `rom_gpio_get_input_level`, PC **`0x2f807750`**, a
32-bit read of **`GPIO_IN1=0x20583068`**, after **30,099,235 instructions**.
The call requests GPIO36, while the same bank also contains enabled input pads
32 and59 with non-GPIO functions. The current pad model rejects unresolved
peripheral drive, so it cannot form that input word. At the stop, enabled pads
are27/SPIQ and32/SPID (mux `0x700`) and59/UART0_RXD (mux `0x300`), all with
function0. No inferred pad level has been supplied to bypass this gap. These
pin-function names come from the fixed S31 IO_MUX header; this is not a claim
that board wiring or electrical idle levels have been verified.

This stage implements only the explicit Winbond `ef4018` unprotected profile's
4KiB Sector Erase and USER Page Program02, based on manufacturer-authored
W25Q128JV RevF sections7.1.2,8.2.13 and8.2.15. WREN must set WEL. Without it,
commands leave the array unchanged. Erase sets exactly the addressed sector to
FF; page program ANDs only clocked data bytes into the array and wraps within
the256-byte page. A completed accepted write clears WEL. Byte alignment,
24-bit addresses, command phases, the64-byte controller buffer, complete
backing-range bounds and the explicit manufacturer are validated before any
mutation. Wrong profiles, malformed transfers or unsupported operations do not
fall through to the shared SPI model's permissive writes.

The fixed S31 ROM emits `FLASH_SE|FLASH_PE` at `0x2f80e4be`; PE's header prose
mentions USER only. This narrow combination is supported as **observed ROM
behavior**, with the discrepancy retained. The subsequent ROM store at
`0x2f80e86a` uses the documented `USER|FLASH_PE` and command02. Other direct
program/block/chip erase, status protection, suspend and encryption paths are
still unsupported. No NOR contents or OTA record is generated by the host:
the actual guest supplies the address and bytes. Operations finish
synchronously after real array mutation; program/erase latency, wear and power
failure behavior are not claimed. The functional cache has no retained lines,
so mapped reads and interpreter instruction fetch see the same updated array.
There is no new S31 browser persistence or production integration here.

The source-only fixture independently proves erase, program, mapped readback,
execution of the programmed instructions, WEL failure branches and one-to-zero
programming. The official image uses the same source/hash and explicit
hardware inputs as before. See the
[official application-entry receipt](evidence/s31-bringup/nor-write-plus-2.0.68.json)
and [functional checks](evidence/s31-bringup/nor-write.json). No hardware or
browser display validation is implied.


Final NOR-stage validation: 1,429 workspace Rust tests (ignored included),
65 private S31 unit tests and all 25 source/ROM fixtures pass, as do both
warnings-denied Clippy targets and all local CI command groups. No golden or UI
change, no browser display test and no silicon verification are claimed. The
same-production-source five-pair CPU guard measured S3 -1.599%, C3 +1.190% and
C6 +2.000%, with identical UART/instruction/cycle checks. These small noisy
differences do not establish a speedup or isolated overhead. See the
[same-source numeric samples](evidence/s31-bringup/nor-write-same-source-perf.json);
the [fixed-main cross-branch guard](evidence/s31-bringup/nor-write-perf.json)
remains separately qualified by the earlier baseline record.


### GPIO function aliases and explicit unresolved input samples

Fixed IDF6.1 IO_MUX definitions assign ordinary GPIO to function0 on selected
pins, including GPIO36; this is now honored alongside function1. Function0
Flash/SPI2/JTAG/UART pins are kept distinct. GPIO29/41 remain excluded by the
S31 capability mask. No default physical pin profile has been inferred.

The private bringup runner accepts repeated `--peripheral-pad=32:0` flags for
caller-supplied, already-resolved voltage samples at otherwise unresolved pads.
These are optional diagnostic inputs, not modeled Flash/UART waveforms or
measured Plus wiring. Missing samples still fault. They apply only to
`Drive::Unsupported`; known GPIO output/contention and disabled inputs keep their
normal behavior. A contradictory external driver also faults. Held pad
configuration retains a live sample, and removing it restores strict behavior.

The unmodified Plus2.0.68 full image still faults at GPIO_IN1 by default. With
explicit samples32:0/59:1, it reaches application Flash PC **0x400059ce** and
stops at **0x20704000**, the first write to PMU_HP_ACTIVE_DIG_POWER, after
**30,099,332 instructions**. Inverting both samples reaches the same PMU access.
The relevant IDF PMU_instance code reads GPIO36 for the Flash-LDO voltage policy,
not the unrelated32/59bits. GPIO36 input is still disabled in the retained
model's deterministic initial state; its physical reset/strap voltage has not
been established. This does not establish a real board voltage configuration.
The Plus remains unavailable and has no framebuffer.

The source-only fixture exercises real guest bank reads before and after a
sample change, with UART PASS and missing/wrong/conflicting input negatives.
See the [fixed source and functional receipt](evidence/s31-bringup/gpio-input.json)
and [three official-image observations](evidence/s31-bringup/gpio-input-plus-2.0.68.json).
Reproduce with the prior fixed-image probe arguments and repeated
`--peripheral-pad 32:0 --peripheral-pad 59:1`; omit these flags for strict mode.

Final GPIO-stage validation: 1,431 workspace Rust tests (ignored included),
67 private S31 unit tests and all 26 source/ROM fixtures pass, along with both
warnings-denied Clippy targets and every local CI command group. The fixture
suite was repeated single-threaded only because the successful initial run's
diagnostic lines interleaved; the initial log hash is retained. No goldens, UI
or product support status changed. The five-pair same-production-source CPU
guard measured S3 +0.193%, C3 +0.053%, C6 +0.732%, with identical UART and
instruction/cycle checks. These small noisy differences are not an S31 speed
claim. [Same-source samples](evidence/s31-bringup/gpio-input-same-source-perf.json)
and [qualified cross-branch main guard](evidence/s31-bringup/gpio-input-perf.json)
remain separate.

### PMU policy banks and PSRAM PHY power

The private core stores the documented active/modem/sleep policy banks separately,
with header-defined writable fields and reset values. Immediate clock updates
apply only the implemented active all-on policy; partial peripheral gating,
active CPU powerdown, forced reset/isolation and sleep transitions remain
explicitly unsupported. Releasing force-on/no-reset/no-isolate controls is
supported. Requested external-LDO controls have register readback; they do not
establish an analog rail voltage or a completed calibration.

The fixed IDF header calls the active regulator's two bias-voltage fields
read-only, while the same revision's LL writes them. This implementation retains
the documented RO reset values and records those attempted writes. DBIAS_INIT
records a request and reads zero as a write-trigger; there is no invented
calibration-done result. The discrepancy remains in the evidence, rather than
being silently resolved in favor of the tested firmware.

`PMU_PSRAM_XPD` is identified by the fixed `ldo_ll_enable(0)` implementation as
MPLL PHY power enable. It now gates actual PSRAM reads, writes and instruction
fetches together with the existing clock control. Power alone does not supply a
chip identity, backing memory or MMU mapping. The source MMU fixture explicitly
enables it; power-off/clock-off negatives reject real memory access and preserve
backing bytes. This models PHY access gating, not external RAM power-loss decay.

The same unmodified full Plus2.0.68 image, with the previously declared development
inputs, passes the mode-policy/force-release/LDO initialization and reaches the
next unsupported immediate MPLL control at **0x207040f4**. It stops after
**30,183,046 instructions**, application SRAM PC **0x2f001ef2**. The real application
entry is still observed once at cycle30,065,117. This has not reached product
initialization or a framebuffer; INK4 Plus remains unavailable. No firmware/ROM
patch, direct application launch or substituted fixture is used. See the
[PMU semantics and checks](evidence/s31-bringup/pmu-init.json) and
[retained official-image progression](evidence/s31-bringup/pmu-init-plus-2.0.68.json).

PMU-stage validation passes 1,434 workspace Rust tests, all 70 S31 unit tests and
27 source/ROM fixtures, both strict Clippy targets and all local CI command groups.
The two performance receipts preserve exact UART/instruction/cycle equality.
The same-production-source five-pair medians changed S3 -0.464%, C3 +1.208%, C6
+0.609%; these small noisy deltas do not establish S31 execution speed. All 372
tracked Rust/manifest/lock files in the production workspace members match the
independently built fixed-source reference. The separate main84 comparison remains
a qualified cross-branch guard. No UI, product status or goldens changed.

### MPLL and PSRAM source clocks

The fixed IDF clock LL supplies the MPLL tie-high/low sequence, the active-policy
and alive-root gates, MSPI divider and calibration fields. The private model now
tracks these independently of CPLL. The functional process requires an explicit
start, enabled power and analog-master clock. It completes after sixteen private
runner quanta; this is a software scheduling contract, not measured analog lock
time. Writing the read-only completion bit cannot manufacture completion. Stop
cancels pending work while retaining an already completed result; power loss or
a changed divider invalidates that result.

The supported REF1 divider follows the LL formula `40 MHz * (FB + 1) / 2`.
Charge-pump/filter tuning and other reference divisors are explicitly unsupported.
PSRAM source selectors0/1/2 route the actual XTAL/MPLL/CPLL rate and integer divider.
Selecting an unavailable source is permitted as register configuration, but real
reads/writes/instruction fetches are denied until both the clock and PHY power
are available. No chip identity, memory size or MMU map is inferred.

With the same full official Plus2.0.68 image and declared development inputs,
the original app programs divider0x99 and selects a 400 MHz PSRAM core clock.
It then faults after **30,183,925 instructions** on the unimplemented dedicated
MSPI pad-register read **0x2058401c**, app PC **0x2f00fa2a**. The source identifies
this as IOMUX_MSPI_PIN_PSRAM_D_PIN0. This still precedes a working PSRAM device,
complete product initialization or framebuffer. INK4 Plus remains unavailable.
See [functional/source evidence](evidence/s31-bringup/mpll.json) and
[two retained original-firmware observations](evidence/s31-bringup/mpll-plus-2.0.68.json).

The source-only fixture actually writes and executes code in caller-supplied
PSRAM, returns42 and emits PASS. Missing analog clock takes its own timeout/FAIL;
missing PHY power, a gated PLL path, a changed divider and written RO completion
all fail before retiring the memory entry. These are functional hardware-model
checks, not claims of physical PSRAM timing or product compatibility.

MPLL-stage validation passes 1,437 workspace Rust tests, 73 private S31 unit tests,
28 source/ROM fixtures, both Clippy targets and all local CI command groups.
The five-pair same-production-source CPU guard preserves exact UART/instruction/
cycle results; median changes are S3 -1.405%, C3 +1.541%, C6 -0.249%.
All 372 production Rust/manifest/lock files match that fixed reference. These
small noisy differences are not an S31 speed claim; the main84 cross-branch
guard remains separately qualified. No UI, product status or goldens changed.

### Dedicated MSPI pad configuration

The S31 PSRAM pins have their own IO_MUX block at0x20584000, separate from GPIO.
Documented reset values and writable masks are retained for the eleven pad
registers, including the distinct DQS layout. The register auto-clock-gating bit
is stored as a policy; disabling automatic gating is not treated as turning the
register clock off. Drive, pull, hysteresis, phase and delay values are retained
configuration, not verified analog waveforms or a signal-integrity model.

HOLD uses a conservative functional subset: configuration is still readable and
writable, but held pads cannot perform dynamic external memory reads, writes or
instruction fetches. No frozen physical pin value or simultaneous-write sampling
order is inferred. Releasing HOLD applies the current configuration and permits
the ordinary clock/PHY/MMU/backing checks. DQS power is a requested configuration
field; this stage does not implement a DDR serial protocol or attach a memory die.

The source-only fixture verifies real pad reads and writes, then writes, reads and
executes code in caller-supplied PSRAM. CS/DQS/data-pin HOLD negatives fail on the
actual store/fetch/load before the PSRAM entry is retired. The same unmodified
Plus2.0.68 full image proceeds through pad setup to the still unimplemented
SPI_SMEM_S_AC register **0x205021a0**, app PC **0x2f00faca**, after **30,183,992
instructions**. There is still no physical PSRAM identity, complete product startup
or framebuffer. Plus remains unavailable. See [source/functional evidence](evidence/s31-bringup/mspi-pads.json)
and [official-image probe](evidence/s31-bringup/mspi-pads-plus-2.0.68.json).

MSPI-pad validation passes 1,439 workspace Rust tests,75 private S31 unit tests,
29 source/ROM fixtures,both Clippy targets and all local CI command groups.
The final source was rerun after removing an unproven HOLD snapshot; the earlier
probe and initial validation hashes are retained. Five-pair same-production-source
CPU medians change S3 +0.144%,C3 +0.611%,C6 +1.205%,with exact UART,instruction and
cycle results. All372 production Rust/manifest/lock files match the fixed reference.
These small differences are not an S31 speed claim; the main84 cross-branch guard
is separately qualified. No product status,UI or golden output changed.

### PSRAM serial controller and explicit test device

S31 MSPI2/3 is modeled separately from the Flash controller. The private subset
retains documented controller reset values and configuration masks, checks APB,
core clock, PHY power and DQS before a serial operation, and applies real command,
address, chip-select, FIFO-length and DDR-phase checks. Unsupported protocols and
unknown registers fail explicitly. Interrupt enable/routing, ECC, sleep/reset
commands and physical signal timing remain unsupported.

The optional `--psram-mr=<12hex>` argument supplies six AP-compatible mode bytes in
MR0,MR1,MR2,MR3,MR4,MR8 order; `--psram-bytes=N` separately allocates backing memory.
Both are required together. There is no default die,identity,density,good-die flag
or memory allocation. Controller reset preserves external mode state and contents.
The older isolated raw-MMU fixtures retain their explicit backing-memory path;
attaching a serial die requires configured MSPI2 AXI and read/write commands before
CPU memory access. Serial reads/writes and CPU accesses share the same byte array.

AP Memory's APS256XXN-OBRx Rev1.00 datasheet section6.7 supplies the non-linear mode
read pairs: address4 returns MR4/MR8,address8 returns MR8/MR0,and reads repeat that
pair. A mode write changes only the addressed byte,not the following read-only
register. Array transactions implement x8/x16 addressing,wrap/hybrid/linear burst
selection and bounded row crossing. Wrong phases,missing devices,invalid addresses
or out-of-bounds writes cannot create success/data. Completed peripheral commands
raise real raw end status; this is synchronous functional behavior,not a serial
waveform or contention/timing guarantee.

With the same unmodified full Plus2.0.68 image and the explicitly supplied reference
test-device responses `0c0ddf802005` plus33,554,432 backing bytes,the firmware performs
6,222 PSRAM commands and prints `Found 32MB PSRAM`, `Speed: 200MHz` and `Multicore app`.
These identity/capacity messages reflect the caller's reference test device; they
are **not identification of the Plus physical die**. Timing-tuning reads return
actual previously written data at every represented delay setting; analog signal
margin,thermal variation and real calibration success are not modeled.

The next observed fault is the original application's write to **LP_APM_FUNC_CTRL
0x20706cbc**, PC **0x401e8666**, after **38,340,198 instructions**. Full multicore
startup,product peripherals and the framebuffer are not ready; Plus stays unavailable.
The source-only fixture independently writes code through real SPI transactions,
reads it back,then fetches it through the configured MMU/AXI path and returns42.
Its negative cases cover absent die,DQS,PHY and core clocks; invalid MR/commands;
missing backing; and closed AXI or removed DQS before instruction fetch.

See [source and fixture evidence](evidence/s31-bringup/psram-controller.json) and
[original-image controls](evidence/s31-bringup/psram-controller-plus-2.0.68.json).
Final validation passes 1,443 workspace Rust tests,79 private S31 unit tests,
30 source/ROM fixtures,both Clippy targets,the private WASM compile and all local
CI command groups. Final-source checks repeat the two narrowed unsupported
boundaries; initial logs and the failed relative-fixture-path invocation are
retained separately. No product status,UI or golden outputs change.

Five-pair CPU guards preserve exact UART, instruction and cycle results. The
same-production-source medians change S3 -0.980%, C3 -0.140%, C6 -1.153%; all 372
production Rust/manifest/lock files match the fixed reference. These small noisy
changes are not an S31 speed claim. The separate main84 cross-branch guard remains
qualified by [baseline provenance](evidence/s31-bringup/baseline-qualification.json).

### APM controls and explicit permission-path fixture

The four separate S31 APM banks now expose their documented reset configuration,
field masks, region locks, per-path function enables, bus-error selection and
automatic clock gate. LP_APM has a different register layout from HP_APM,
HP_MEM_APM and CPU_APM; the HP bank's final path also has a distinct offset.
The existing peripheral permission controller protects these configuration banks.
The actual application disables LP, HP and HP_MEM filtering itself; CPU filtering
remains at reset. No guest opcode or firmware byte is changed.

This is a bounded **reset-TEE, region0** subset. The published S31 TRM
pre-release0.1 still omits the bus/PMS chapters, and the fixed public IDF source
has no S31 `apm_ll.h`/`apm_defs.h`. Therefore the model does not borrow P4/C6 master
numbers. Without an explicit test route it accepts only the unrestricted reset
TEE region configuration and guest function-enable changes. Restrictive TEE
configuration requires `set_test_route` and an enabled bus-error response.
There is no default product route or CLI option that pretends this topology is
known. Region0 retains its documented full range; alternative enabled regions,
REE transitions, silent-denial responses, interrupt delivery and transparent
HP-AXI exception traffic are unsupported and rejected.

A source-only guest fixture declares a test connection to a 128-byte SRAM page,
then independently verifies write, read and execute denial. A denied write leaves
the previous data intact, and a denied instruction fetch never retires the target
entry. Disabling the declared path allows the same guest code to execute and
return42; the guest then restores permissions. Status bits and W1C work, but
exception INFO0/INFO1 reads remain unsupported because transaction-ID/region
encoding is not established. Host diagnostics are separate from those registers.

With the same full original Plus2.0.68 image and unchanged explicit development
inputs, the next first fault is **PMU_CPU_STALL_SW0x207041ec** read from application
PC **0x2f00c34e**, after **38,340,579 instructions**. It has not completed multicore
startup or rendered the product framebuffer. Plus remains unavailable.

Final-source validation passes 1,446 workspace Rust tests (including82 S31 unit
checks),31 source/ROM fixtures,both warnings-denied Clippy targets,the private
WASM compile and every local CI command group. The initial full-suite run lacked
19 nested ignored fixture files in the new worktree; the original failure is
retained,and the complete suite was repeated after copying and hash-checking them.
No UI,browser product status,production-core code or golden output changes.

Two five-pair CPU guards preserve exact UART, instruction and cycle outcomes.
Against the fixed identical-production-source reference,the medians change S3
-0.269%,C3+0.885%,C6+1.306%; all372 non-S31 production source/manifest/lock files
match. This is a regression guard,not an S31 speed claim. The main84 cross-branch
comparison retains the existing baseline-provenance qualification.
See [source/fixture checks](evidence/s31-bringup/apm.json),
[original-image final result](evidence/s31-bringup/apm-plus-2.0.68-final.json),and
[CPU guard](evidence/s31-bringup/apm-same-source-perf.json).

## Two functional HP harts (2026-10-28)

The private runner now executes both HP harts. The S31 v6.1 clock/reset words
`2058701c/20`, PMU stall commands `207041ec`, actual stalled-state word
`20586064`, and LP boot-address word `20700164` control execution. CPU1 begins
at the real ROM reset vector `2f800000` with `mhartid=1`; the ROM waits for the
address written by `ets_set_appcpu_boot_addr` and performs its own jump. Writing
the boot-address register does not itself start or redirect a CPU. The CLIC
window `10800000` belongs to the executing hart; the previous unsubstantiated
`+10000` remote alias now faults explicitly. Each hart has its own interrupt
matrix/threshold, instruction-cache gate and monitor capture. The common data
cache/MMU and SRAM/PSRAM are shared. Successful stores invalidate a conservative
whole-address-space LR/SC reservation set, including writes through aliases;
this permits extra SC failures without allowing a false successful reservation.

The scheduler alternates one interpreter instruction per eligible hart. A round
advances one optional nominal RTC quantum; it does not sum both cores' cycles
or claim silicon arbitration/timing. Gated or stalled cores do not execute, and
one core's WFI does not stop the other. Global-reset assertion resets that hart
and its CLIC, preserves shared RAM and the LP boot-address word, and causes a
fresh ROM execution when released. `FORCE_NORST`, unknown stall command values,
LP-triggered reset-domain reconstruction, full PMU sleep and DMA remain explicit
gaps. This stage does not turn on a product runtime.

The source fixture runs the real CPU1 ROM twice, checks each hart's independent
CLIC word, obtains 400 shared atomic increments with 200 observed SC retries,
then stalls/resumes CPU1 and changes its guest boot address before resetting it.
Both guest routines finish in WFI; no host writes the result words or CPU1 PC.
The complete official Plus 2.0.68 image also executes its second-hart application
code. Its next failure is a read of `CACHE_TRACE_ENA` (`2c0003d8`) by hart1 at
`400022ee`; the old PMU stall-register fault is gone. Cache-error/trace setup,
remaining application peripherals and the parallel EPD display are still pending.
**INK4 Plus remains unavailable, with no framebuffer output.**

The [stage receipt](evidence/s31-bringup/multicore.json) records final source/input
hashes, failures and regression measurements. Final validation passes 1,454 workspace
Rust tests, 90 private S31 unit tests, 32 source/ROM fixtures, both warnings-denied
Clippy checks and every local CI group. The [official image receipt](evidence/s31-bringup/multicore-plus-2.0.68-final.json)
records CPU1 entry `2f000882` retiring once at its own cycle1,008. Earlier single-hart statements above describe
their historical checkpoints, not the scope of this new private runner.

## Cache failure tracing and first PSRAM self-test (2026-10-28)

The private S31 runner now models the L1 fail-trace gate and its enable, raw,
status and clear words. An actual request through a closed, powered and clocked
cache bus latches the corresponding I0/I1/shared-D error; source69 is asserted
only while a latched bit is enabled. Disabling trace does not clear an old event.
The S31 TRM access glossary distinguishes the raw register's **WTC** field from a
direct write-one-to-clear register: writing the paired **WT** clear word clears
the latch; writing RAW or the read-only masked status cannot clear or forge it.
Frozen/unclocked cache domains, missing host backing and unmodeled MMU failures
still produce strict diagnostic faults rather than invented cache-error causes.
Physical requester-ID/attribute capture and complete fail-propagation behavior
remain unsupported; no failed access is replaced with fabricated data.

The source fixture causes the fault on the second hart and runs the real first
hart CLIC handler after the runner is resumed normally. It verifies a pending
error, failed direct RAW clearing, successful WT clearing and restored Flash
access. All result words and UART PASS text are written by executed instructions.

The same unmodified full INK4 Plus 2.0.68 image passes its own PSRAM memory test,
printing `SPI SRAM memory test OK`. Its next unsupported access is TIMG0
`RTCCALICFG` at `20580068`, read by hart0 at `2f00df70`. The next stage must
implement clock-calibration counting; this result is not a board/display test.
**INK4 Plus remains unavailable and has no framebuffer output.**

The [cache stage receipt](evidence/s31-bringup/cache-errors.json) retains source
versions, functional fixtures, checks and performance guards. The [final official
image observation](evidence/s31-bringup/cache-errors-plus-2.0.68-final.json) fixes
the original image hash and explicit development inputs.

## TIMG0 RTC clock measurement (2026-10-28)

The private runner counts XTAL ticks over a configured target-clock window,
including the selected divider, one-shot completion, periodic data-valid and
missing/gated-target timeout. The counter advances only with explicit clock
input. Register reads and writes to read-only results cannot complete a pending
measurement. TIMG0 register reset clears its measurement state while preserving
the separate clock mux; APB gating blocks access, and reset holds the counter.
The supported calibration mux inputs are existing functional MPLL/CPLL,
SYS/APB/MEM clocks, nominal136kHz RC_SLOW and40MHz XTAL. Unconnected BBPLL/APLL,
RC_FAST digital routing, external32K/pad and per-core gate paths reject explicitly.
Zero-cycle requests are outside the fixed IDF LL contract. The timeout reset-pulse
width and physical edge phase/jitter are not modeled; periodic windows start with
zero phase and therefore report the rounded-up ideal interval.

A separate `--rtc-calibration-from-cpu` development option advances XTAL ticks
from one nominal CPU quantum per dual-hart scheduler round. It does not change
`--rtc-timer-from-cpu` semantics or silently enable watchdog timing. Fractional
clock conversion is preserved across CPU frequency changes. The source fixture
first remains in real guest polling for20000 instructions without a clock input;
with the option enabled it verifies30 and5000 XTAL tick results, a gated-clock
timeout, a restart and its own UART PASS output.

The complete unmodified Plus2.0.68 image now finishes RTC calibration and reaches
`MODEM_LPCON_WIFI_LP_CLK_CONF` at`2010f00c` (hart0 PC`401ea4e8`). This address is
confirmed by the separate modem base header: MODEM_PWR_BASE`2010d000` plus`2000`,
then register offset`c`; it is not in the general peripheral base table. Modem
clock/power behavior is the next stage. **INK4 Plus is still unavailable and has
no framebuffer output.** See the [calibration receipt](evidence/s31-bringup/rtc-calibration.json)
and [final official-image result](evidence/s31-bringup/rtc-calibration-plus-2.0.68-final.json).

## MODEM low-frequency requests and startup UART clocks (2026-10-28)

MODEM_LPCON now owns its clock words once, including the internal analog-I²C
master gate and reset. The BLE timer, COEX and Wi-Fi-power requests select the
existing XTAL or nominal RC_SLOW input with divider+1; a missing request or
parent clock yields zero rate. FORCE_ON preserves policy but cannot substitute
for EN. Unknown clock sources and unimplemented reset targets fail explicitly.
PMU state maps and pad sleep requests are retained configuration only; neither
creates a sleep transition, RF calibration result, connection or received packet.

The source fixture executes eight real internal I²C transactions while the other
LP clock bits are populated, resets the master and checks preserved target data.
Disabling the master/parent or omitting explicit BIAS data stops the same guest
before any transaction. This caught and removed an early duplicate-owner bug
that had disconnected the new LPCON gate from the existing analog sequencer.
UART1/2/3 expose the documented XTAL divider and independent APB/core gates, but
their FIFOs, interrupts and pin routes remain unimplemented. UART0's existing
core-clock predicate now stays independent of APB gating. Four UART sleep-pad
clock request bits retain their documented reset values without implying wakeup.

The unchanged complete Plus2.0.68 image passes these initialization words and
stops at ETM clock control `205870a8` (hart0 PC`4000204c`) after71,120,058 total
instructions. The fixed IDF6.1 `esp_perip_clk_init` calls its clock-gate helper to
disable unused modules after reset; ETM is the next word in that sequence. The
model still has no parallel EPD controller/framebuffer and **INK4 Plus remains
unavailable**. This is a private SoC stage, not evidence of product usability.

See [clock/fixture receipt](evidence/s31-bringup/modem-clocks.json) and
[official image result](evidence/s31-bringup/modem-clocks-plus-2.0.68-final.json).

## Unused peripheral clock cleanup and application metadata (2026-10-28)

The fixed IDF6.1 startup helper shuts down unused clock requests before scheduler
initialization. The private S31 model now retains21 precisely addressed words
for that sequence, using documented reset values and field masks. This includes
ETM/trace, DMA and GPSPI requests, PARLIO/I²S pad clocks, key-manager and HP/LP
I²C clock requests, the untouched USB OTG reset bits, and TEE/LP GPIO automatic
clock policy. Existing SHA, TIMG, UART, Flash, PSRAM and modem clocks keep their
original owners; no duplicate storage shadows those functional devices. New
TEE, LP_PERICLKRST and CNNT accesses use their documented peripheral permission
slots. Clock changes cannot bypass an existing permission denial.

This is deliberately a cold configuration subset. Unsupported reset/force/source
changes fail, and peripheral data registers remain unmapped. The LCDCAM source
and integer divisor may be configured while disabled, matching the startup
helper; activating its unimplemented engine fails explicitly. Automatic clock
gating does not mean the permission filter is disabled. No invented completion,
DMA transaction or RF packet is generated by these words.

The executed source fixture computes SHA256 of `abc`, gates the existing engine,
restores it and checks the preserved digest. It also verifies that gated data,
unimplemented ETM/LCDCAM and three explicit PMS denials fault without PASS.
The complete unmodified Plus2.0.68 image now prints its own project name
`Metalio-E-Ink4-Plus`, version and heap initialization ranges. The next unsupported
access is SYSTIMER clock control `20587120`, hart0 PC`402bd8e2`, after71,948,628
instructions. Scheduler/timer startup and the actual board/display path remain
unfinished; **INK4 Plus is still unavailable, with no framebuffer**.

See the [startup clock receipt](evidence/s31-bringup/peripheral-clocks.json) and
[official image observation](evidence/s31-bringup/peripheral-clocks-plus-2.0.68-final.json).

## SYSTIMER counters, snapshots and guest alarm interrupt (2026-10-28)

The private S31 timer uses its own documented `20399000` register window and
`20587120` clock/reset word. It has two52-bit counters, three independently
selected comparators, source33/34/35 interrupt routes, and the fixed divide-by2.5
clock relationship. APB access gating is separate from counter-clock gating;
the local `CLK_EN` bit selects automatic/free-running register-clock policy and
does not stop the counter when clear. XTAL uses40MHz; the optional RC_FAST path
uses the existing power-gated nominal17.5MHz source, not a measured oscillator.

Snapshot validity begins clear and only becomes valid after an actual update
request crosses an explicit timer edge. Reading status or writing its SS/WTC
bits does not complete work. Counter and comparator loads use their WT bits;
zero does nothing. Alarms retain active targets separately from pending register
values, honor unit/work/stall gates, and latch raw interrupts independently of
interrupt enables. Only the paired clear register clears raw status. Periodic
comparators handle52-bit wrap and large elapsed inputs without per-event loops
or guest-triggered arithmetic overflow. The permissions route uses the real
HP_PERI1_PMS control at offset60. ETM enable configuration is retained, but no
ETM event consumer or sleep transition is claimed.

`--systimer-from-cpu` is a separate, explicit development timing option. It
converts one nominal CPU quantum per dual-hart round into source ticks, retaining
fractional progress across CPU frequency changes. It does not silently enable
RTC/watchdog time or claim instruction-accurate timing. A completely waiting
machine still returns `Waiting`; callers can supply explicit timer edges with
`advance_systimer_ticks`. There is no automatic idle-time jump in this stage.
Synchronization on the next timer edge is a functional model, not measured CDC
latency. FORCE_NORST precedence and zero-period alarms remain unsupported.
The retained September2026 public TRM still marks its System Timer chapter
“to be added later” (27%); register semantics and clock ratio here come from
the fixed public IDF headers/HAL. Wrap and batching tests establish the declared
functional model, not silicon validation of edge ordering or simultaneous events.

The source fixture first executes20000 instructions without a timer input and
stays in the real snapshot polling loop. Enabling the nominal clock conversion
lets the same guest program its alarm and reach WFI after44 instructions. With
no further edges it remains asleep and produces no interrupt. Supplying512
timer edges wakes the guest through the actual matrix/CLIC route; its own ISR
checks and clears status, returns with `mret`, and prints
`S31 SYSTIMER CLOCK SNAPSHOT IRQ MRET PASS` after241 further instructions. The
host neither writes completion flags nor changes the PC to obtain that result.

With the same original Plus2.0.68 complete image and the previously declared
development electrical/clock inputs, execution now passes SYSTIMER initialization
and prints `Adding pool of 32768K of PSRAM memory to heap allocator`. The next
actual fault is a store to the PSRAM controller's interrupt-enable register
`205020c0`, CPU0 PC`4000bb2c`, after72,090,520 total instructions. There are no
timer alarm events before that stop. This is not a scheduler or display pass:
**INK4 Plus remains unavailable, with no framebuffer**.
The same complete image without `--systimer-from-cpu` exhausts100,000,000
instructions at PC`2f808cb6`, with no access fault, zero timer ticks/counters and
no fabricated snapshot acknowledgement. Its earlier RTC clock inputs remain
unchanged, so this control isolates the missing SYSTIMER input.

See the [timer receipt](evidence/s31-bringup/systimer.json) and
[official image observation](evidence/s31-bringup/systimer-plus-2.0.68-final.json).

## PSRAM address errors and guest interrupt handlers (2026-10-28)

System MSPI2 now implements its documented interrupt bank at
`205020c0/c4/c8/cc`. An actual rejected PSRAM MMU translation raises the read
address-error bit7 or write address-error bit9 while retaining the original
synchronous access exception. Instruction fetch is a read. Raw events latch
independently of enables, become masked status, and clear only through the paired
WT clear register. Controller reset clears both banks without erasing the
explicitly supplied external RAM data or mode registers.

The fixed public S31 `mspi_periph.c` assigns source112 to system MSPI2 and
`irq=-1` to peripheral MSPI3. Only real enabled system events reach the per-hart
matrix/CLIC. The existing MSPI3 command-completion raw bits remain independent;
this stage does not invent a peripheral interrupt route. Documented enables for
ECC, FIFO, local PMS and XTS events can be retained as configuration, but their
unimplemented engines do not fabricate successful checks or pending events.

The error path distinguishes invalid MMU entries from missing development
backing, unsupported sensitive/alternate targets, and power/cache/pad gating.
Those other failures remain explicit diagnostics and do not falsely become
PSRAM address-error interrupts. Cross-page operations preserve first-failure
order; a rejected multi-byte write leaves earlier bytes unchanged.

The source fixture executes a real unmapped load, receives exception5 after37
instructions, and resumes its own exception handler without host register/PC
repair. That handler advances `mepc`; the real source112 ISR checks and clears
the latched read error before `mret`. A subsequent store independently produces
exception7 after32 more instructions and takes the same two-handler path. The
guest then establishes a valid mapping and verifies an actual data write/read,
writes completion word2 and prints `S31 PSRAM REAL FAULT IRQ MRET PASS` after225
more instructions. The supplied64KiB backing is a declared development input,
not physical RAM identification. The first fixture attempt failed because it
compared the whole CLIC `mcause`, including MPIE/MPP state, with the exception
number; correcting the guest's extraction fixed that test without changing the
emulator's exception behavior.

With the same complete, unmodified Plus2.0.68 image and prior explicit electrical
and clock inputs, the actual application passes PSRAM error-handler registration
and reaches its brownout initialization. Its next fault is a read from
`LP_ANA_BOD_MODE0_CNTL` at`20702000`, CPU0 PC`401ea9e4`, after72,090,656 total
instructions. This stage does not bypass brownout control, replace the image or
claim a working scheduler/display: **INK4 Plus remains unavailable and has no
framebuffer**. All production chip paths remain unchanged.

See the [PSRAM interrupt receipt](evidence/s31-bringup/psram-irqs.json) and
[original image observation](evidence/s31-bringup/psram-irqs-plus-2.0.68-final.json).

## Brownout mode0 configuration and explicit detector input (2026-10-28)

The private S31 power model now separates mode0's digital counter/interrupt
path from the earlier mode1 analog reset input. Mode0 uses its own documented
control reset value, wait fields, RW counter-clear level, and independent HP/LP
RAW/ENA/ST/CLR banks. HP events route through source143 to the real matrix/CLIC.
The LP bank retains its register behavior; no LP processor delivery is claimed.
Writing RAW/status cannot fabricate or acknowledge an event. Reset requests stop
the diagnostic runner with the existing `PowerReset(Brownout)` result; they do
not silently restart a host process or invent reset sequencing.

`brownout_mode0_ticks(tripped,ticks)` accepts an explicitly qualified comparator
sample and detector-domain edges. It does not derive voltage from the threshold
byte, assume a physical detector clock, or advance while the guest reads a
register. Zero edges do nothing. Disabled detection, held counter-clear and
inactive samples do not accumulate qualification time. Active samples reach the
configured interrupt/reset waits; clearing raw while the condition persists can
raise it again on a later edge. This is a declared functional counter model,
not silicon validation of detector latency or simultaneous-edge ordering. The
source-selection register is retained as configuration; callers qualify input
for that selection externally.

Unknown FIB policy or an enabled higher-priority mode1 path cannot be silently
bypassed. Chip-wide analog reset and Flash/RF shutdown on a real comparator
input remain unsupported: their configuration can be inspected, but such an
input returns an explicit error instead of claiming successful power behavior.
The bounded supported path delivers the digital interrupt and digital-system
reset request with those unsupported actions disabled. Ordinary startup with
no comparator event does not manufacture a healthy-voltage measurement.

The internal threshold register is the actual target`0x61`, register5, on the
existing digital analog-I²C controller. Its source header documents bits2:0 but
not the full reset byte, so `--analog-bod=hh` is an explicit optional seed. Reads
without that byte fail. Real guest read-modify-write preserves the other bits,
uses the selected master and respects clock/slave gates; it does not bypass the
bus or convert the byte into a voltage.

The source fixture programs that byte through three guest transactions and
reaches WFI after201 instructions. With no input it remains asleep; one detector
edge is insufficient. After the second edge, its actual ISR checks HP and LP
raw status, clears each bank independently and returns with `mret`;192 further
instructions produce `S31 BROWNOUT REAL IRQ MRET PASS`. A separate mode requests
a digital-system reset on the fourth edge. Missing analog input and requested
RF shutdown are negative cases, with no fabricated completion.

For the same original Plus2.0.68 full image, leaving the new analog seed absent
stops at the real internal-I²C transaction:72,091,514 instructions, store to
`2010f804`, PC`2f00c06c`. Supplying the explicitly declared reference byte`ff`
lets the guest finish its brownout setup without a comparator event. The next
actual stop is a read from `FLASH_SPI1_FLASH_WAITI_CTRL` at`20501098`,
PC`2f00bc7c`, after72,135,486 instructions. Neither result indicates board/display
completion: **INK4 Plus remains unavailable, with no framebuffer**.

See the [brownout receipt](evidence/s31-bringup/brownout.json),
[strict missing-input observation](evidence/s31-bringup/brownout-plus-2.0.68-no-seed.json)
and [explicit-input observation](evidence/s31-bringup/brownout-plus-2.0.68-final.json).


## Flash automatic idle control and bounded guest handlers (2026-10-28)

The private SPI1 model now retains `FLASH_WAITI_CTRL` with its documented reset
value and mask. PER/PES wait-delay bits are cold configuration;actual suspend/
resume triggers and automatic engines remain unsupported. Supported sector
erase and page program still change the backing array under the real WEL rules.
When automatic wait is enabled,an eight-bit command05 with no address samples
the resulting NOR status after the synchronous operation. Invalid command or
framing fails before array/WEL mutation. Disabled wait sends no automatic
status command. Dummy timing fields are retained,without a claim of physical
sampling calibration or busy-time simulation. Host diagnostic counters report
these internal samples;the public RD_STATUS latch is not invented as an alias.

The source fixture erases,programs,reads and executes the programmed Flash code.
It passes with automatic waiting (three status samples) and with waiting off
(no samples). Wrong command,address phase,16-bit command and actual suspend
trigger each stop before any Flash write. The existing NOR fixtures and tests
remain unchanged.

The original complete Plus2.0.68 image now passes Flash initialization. With the
same declared electrical/clock inputs,it first stops after72,245,916 instructions
at an architectural illegal-instruction exception for `flw fa5,1340(a5)`
(`53c7a787`,mepc`4006ba84`);the CPU has already entered its exception vector at
`2f000000`. FS is Off and no bus fault has occurred. This is a diagnostic stop,
not evidence of a missing FLW decoder:the shared RV32F implementation already
raises that exception when floating-point state is disabled.

An optional bounded `--guest-exceptions=N` diagnostic policy now permits the
guest's architectural handler to execute. Default0 preserves the first-stop
behavior. It never changes guest PC/registers,skips instructions,or continues
through a recorded unsupported bus access. A separate source fixture proves
that a real handler enables FS,returns with `mret`,retries the same FLW and
computes/stores binary32 3.0. A repeated-exception case checks the finite limit.

For the actual Plus image,continuing one such exception reaches a second,real
failure after72,246,976 instructions:store to TIMG1 watchdog write protection
at`20581064`,PC`2f80927c`,denied by the existing clock gate. The exception
handler has not been shown to return successfully. Public IDF6.1 `soc_caps.h`
marks the FPU EXT_ILL load/store limitation,but its `vectors.S` opcode fallback
also has a P4-only revision condition;the image's private dirty SDK revision is
unknown. This potential mismatch and the actual handler path need further
investigation;no FPU state or watchdog gate is forced merely to pass startup.
**INK4 Plus remains unavailable,with no framebuffer or board-level UI claim.**

See the [Flash wait receipt](evidence/s31-bringup/waiti.json),
[first-exception observation](evidence/s31-bringup/waiti-plus-2.0.68-first.json)
and [guest-handler observation](evidence/s31-bringup/waiti-plus-2.0.68-handlers.json).


## Explicit upstream FP memory reason compatibility (2026-10-28)

The first FLW exception was compared with the fixed official Espressif emulator
v0.51.0 using the same complete Plus2.0.68 image. Its actual guest executes
`csrrw a0,0x7f0,zero` at`2f00017e` and reads EXT_ILL=1. The handler returns to
`4006ba84`, retries the original `53c7a787` FLW, advances to`4006ba88`, and loads
binary32 1.0 into F15. Temporary breakpoints are removed before each step; no
host register/FS/PC edit or firmware patch is used. The public reference reports
22 ROM intercepts, so this is evidence of that emulator's behavior, not measured
silicon. Its debug target does not expose arbitrary CSR numbers as register
indices: an earlier attempt to read `p300`/`p7f0` was inconclusive and is retained
as a negative observation, not interpreted as mstatus or EXT_ILL.

This differs from the fixed public ESP-IDF6.1 S31 `soc_caps.h`, which declares
`SOC_CPU_HAS_FPU_EXT_ILL_BUG=1`. The public vector load/store fallback is also
conditional on a P4-only revision option. The image reports a private dirty SDK
revision; neither the exact hardware revision behavior nor that private SDK is
known. Therefore strict behavior remains the default. The private S31 runner's
explicit `--upstream-fpu-memory-reason` option adds the FPU reason bit only after
a real FS-Off FLW/FSW illegal-instruction trap, including their compressed forms.
It does not enable FS, advance PC, suppress the exception, change mtval/mepc,
or convert unrelated illegal instructions or data faults. A per-hart diagnostic
counter records each compatibility intervention. Shared RISC-V/P4/C3/C6 code is
unchanged; no product uses this option implicitly.

The source fixture `fpu-reason-probe.S` checks six real FS-Off traps: FLW, FSW,
C.FLW, C.FSW, C.FLWSP and C.FSWSP. For each, its own handler checks EXT_ILL,
enables FS, returns with `mret` and retries. The complete guest checks stored
binary32 1.5 and the six-event mailbox. Strict mode fails its explicit reason
check; the opt-in path prints PASS. The same source-built image, loaded by the
official emulator's real ROM at Flash offset`2000`, also prints PASS. Additional
unit cases preserve unrelated exception behavior, default/off policy, and
correct hart attribution. These diagnostic images are ignored local outputs,
not firmware catalog entries or replacements for Plus's official image.

With the opt-in reason and the prior declared electrical/clock inputs, the
unmodified full Plus2.0.68 image handles one actual FPU exception and proceeds.
Its next recorded fault is a store to `HP_SYS_CLKRST_CRYPTO_CTRL0_REG` at
`20587058`, PC`40000b3e`, after72,370,696 instructions. This is the next real
unsupported clock/reset path, not a successful board boot. Strict-mode results
are retained separately. **INK4 Plus remains unavailable, with no framebuffer.**

Use `tools/s31-upstream-fpu-probe.py` with caller-supplied firmware URL/hash,
reference executable/hash and an ignored output directory to reproduce the
reference handler observation. It keeps firmware only in RAM/stdin, validates
fixed instruction signatures, records only relevant architectural values, and
closes its own debugger/process even on failure. Reference downloads, source
versions, exact observations, negative history and validation are in the
[FP reason receipt](evidence/s31-bringup/fpu-reason.json) and
[original image result](evidence/s31-bringup/fpu-plus-2.0.68-compatible.json).
