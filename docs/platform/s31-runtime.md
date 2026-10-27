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
