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
