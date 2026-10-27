# ESP32-S31 bring-up

Status on 2026-10-27: Metalio INK4 Plus remains **unavailable** in this platform. The
first executable baseline uses Espressif's separate precompiled emulator, not this
repository's core. It reaches the official application's startup and then fails;
neither a working product display nor complete board support has been established.

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
