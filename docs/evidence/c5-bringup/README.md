# ESP32-C5 independent bring-up (EX249)

Checkpoint `c5b20387` executes actual C5 SRAM-linked guest code on the shared RV32IMAC
interpreter, with a new C5 bus and separate Stamp-C5/S016-DIP board identities.
Both variants receive UART bytes through the C5 source47/CLIC interrupt route;
the guest ISR changes the active-low GPIO28 blue LED. A separate explicit
GPIO5 RMT fixture transmits `[1,0]` and polls guest TX_END. No C3/C6 SoC is used.

This is **not yet a platform runtime**. There is no Machine/CLI/WASM product
registration in this checkpoint. Stock ESP-IDF/UIFlow, flash boot, radio, USB,
eFuse identity, dynamic clocks, LP-core execution and analog charging are not
implemented. S016/S016-DIP catalog runtime remains unavailable. The board has
one blue GPIO LED, no RGB or programmable button; the charger's red nCHG LED
is not connected to a guest GPIO. DIP is a separate mechanical SKU with the
same source-declared electrical mapping, not an independently verified binary.

The later [Machine/app-image follow-up](machine-runtime.md) adds an explicit SRAM
app-direct API and fixes the independently reviewed reset-flag omission, while
retaining all checkpoint results below.

## Official sources and differences

ESP-IDF v5.5.4 is pinned to
`espressif/esp-idf@735507283d5b2f9fb363a1901172dbd9e847945d`.
`sources.json` retains URL, actual byte count and SHA-256 of inspected files.
The official [C5 datasheet v1.5](https://documentation.espressif.com/esp32-c5_datasheet_en.pdf)
specifies RV32IMAC, up to240MHz, 320KiB ROM, 384KiB HP SRAM, 16KiB LP SRAM and
29GPIOs. Stamp-C5 uses ESP32-C5HF4 with in-package4MiB Quad flash and no PSRAM;
the datasheet permits v1.0/v1.2 silicon, so no physical board revision is guessed.
This implementation targets the **audited MP rev100 ROM**, not beta rev0.

Important independent C5 mappings:

| Item | Contract |
| --- | --- |
| ROM / reset | `0x40000000..0x40050000`, reset entry `0x40000000` |
| HP / LP SRAM | `0x40800000..0x40860000`, `0x50000000..0x50004000` |
| Flash cache (not implemented) | `0x42000000..0x44000000` |
| Boot image | chip ID `0x17`; not a C3/C6 image; image loading not implemented at this checkpoint |
| CLIC | `0x20800000`, control bytes `0x20801000`, standard mintstatus`0xFB1` / mintthresh`0x347`; input IDs16–47 |
| Interrupt matrix | `0x60010000`,84sources; GPIO31,UART0=47,UART1=48,RMT55 |
| GPIO | base`0x60091000`; enable`+0x34`,input`+0x64`,PIN0`+0xd4`,matrix inputs`+0x2d4`,outputs`+0xad4`; selector256 |
| UART | `0x60000000` / `0x60001000`;8-bit FIFO count,CONF0 RX reset bit22 |
| RMT | `0x60006000`,compact2TX layout,symbol memory`+0x400`,signal64+channel |
| Clock limits | IDF declares40/48MHz XTAL and160/240MHz PLL selections; fixture uses nominal240MHz CPU/80MHz RMT arithmetic, no physical timing claim |

Shared `Uart`, `Gpio`, `RmtCompact` and `ClicCore` provide only matching register
semantics. The C5 GPIO address/selector adapter and CLIC config format are
independent. Unsupported addresses fault with the guest PC/address/direction.
Related EX205 covers different C3 digital mappings; EX241/EX247 establish the
shared CLIC machinery, not this C5 map or its firmware compatibility.

## Real ROM negative result

The official esp-rom-elfs20260528 tarball checksum matches the repository's
existing pinned release. It contains both C5rev0 and C5rev100; lack of ROM is
**not** a blocker. The rev100 ELF's RAM initializer sections are reconstructed
into their declared ROM locations without replacing an instruction or return
value; the real guest reset loop then copies them. Beta rev0's different ROM
layout is rejected.

The first probe stopped after4dispatch slots atPC`0x40001616`, reading
`LP_ANA_DATE=0x600b2ffc`. Implementing only that documented version-register
reset value and power-on reset-cause register lets the unchanged ROM execute
6575slots before a load fault atPC`0x400269b6`, address`0x600b2804`
(`LPPERI_RESET_EN`). No UART banner is produced. Remaining LP reset/eFuse,
clock, flash/MMU and boot behavior is **unimplemented**, not unavailable
documentation; no IDF/stock boot success is claimed.

The exact official UIFlow2 v2.5.4 download returned HTTP400,
`BURN_CAPTCHA_INVALID`. The raw request identifier was discarded. No BIN was
obtained or executed. `espressif/esp-emulator@b2f2cb1991f09ba9036ba35d894025ef9b44d3cc`
advertises C5 execution but publishes installer/docs/binaries, not reusable
backend source in its repository; it was not imported or used for a success claim.

## Reproduction and checks

Base source: `02ad301783d2502fc4946a02050ff6bc1513ead8`. New crate sources are
this receipt's containing commit. Rust stable1.99.1. Commands, source/input
hashes and numeric results are retained in `results.json`.

```sh
RISCV_GCC=/path/to/riscv32-esp-elf-gcc sh firmware/c5-uart-probe/build.sh
CARGO_TARGET_DIR=target/c5-build CARGO_BUILD_JOBS=2 cargo clippy -p esp32c5 --all-targets -- -D warnings
CARGO_TARGET_DIR=target/c5-build CARGO_BUILD_JOBS=2 cargo test -p esp32c5
C5_ROM_ELF=/path/to/esp32c5_rev100_rom.elf CARGO_TARGET_DIR=target/c5-build CARGO_BUILD_JOBS=2 cargo test -p esp32c5 -- --include-ignored --nocapture
node tools/check-evidence-privacy.mjs
```

Seven focused checks pass with the public ROM supplied (six without it). They
exercise actual UART/CLIC/LED/RMT guest code; wrong C6 maps; SRAM bounds;
wrong IO_MUX/disabled outputs; malformed loading; and exact ROM failure.
No timing experiment or speed claim was made. Existing CPU/peripheral hot paths
are untouched. Full workspace/CI checks belong to integration; this checkpoint
only ran the focused crate tests and Clippy.

Evidence is curated from public inputs; private absolute paths, transient
request IDs and unrelated machine data are omitted. The public fixture's
complete20-byte UART expectation is retained as a test assertion, not a private
console capture. This reduction does not change any measured work counts or hashes.
