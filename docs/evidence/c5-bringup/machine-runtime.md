# C5 SRAM app-direct Machine follow-up

This follows checkpoint`c5b20387` and EX249. The independent C5 bus now implements
the shared `Soc`/`SocBus`/`Machine` API. Actual GCC14.2.0 code packaged by
Espressif esptool5.4.0 runs as an explicit SRAM app image on both `stamp-c5` and
`stamp-c5-dip`. This does not complete ROM/IDF boot. Shared CLI/WASM/browser
registration is a separate integration change, not part of this receipt's tests.

## API and image contract

```rust
use esp_soc::SocBus;
let mut m = esp32c5::machine([0; 6], 4 << 20);
m.bus.attach_board(esp32c5::board::make_board("stamp-c5").unwrap());
m.load_input(esp_soc::LoadKind::App, app_bytes)?; // existing kind3; staged at0x10000
m.boot_app(0x10000)?;                           // explicit SRAM copy/entry
m.bus.uart_input(0, b"1");
```

`machine`, `C5` and `Machine` are public exports; `board::make_board` recognizes
`stamp-c5`/`stamp-c5-dip` plus their exact product IDs. `bus.board`, `flash`,
`attach_board` and `attach_board_devices` support the existing front-end glue.
Flash staging allocation is bounded to32MiB even for an oversized host request;
the Stamp product profile is4MiB and0PSRAM. No eFuse, JEDEC or physical silicon
identity is invented. Front ends must reject ROM boot and unsupported sizes;
the separate `bringup rom` example remains available for the measured negative probe.

The explicit app-direct loader accepts:

- ESP magic0xe9, chipID0x17,1..16 nonempty, nonoverlapping segments; addresses
  and lengths are4-byte aligned and contained in HP`0x40800000..0x40860000` or
  LP`0x50000000..0x50004000` SRAM. The2-byte aligned entry has at least2loadedbytes.
- Full minimum revision at15 <=100; full maximum at17 is0(unset) or >=100.
  The deprecated byte14 is ignored, matching pinned IDF's
  `bootloader_common_loader.c`. No physical S016 silicon revision is inferred.
- XOR checksum seeded0xef over segment data at
  `floor((endOfSegments+16)/16)*16-1`; intervening footer padding is zero.
  Header23 is0or1; when1, the appended32-byte SHA-256 is checked. Remaining
  staging bytes must be0xff. This is integrity validation, not secure boot.
- No IDF application descriptor is required for a bare-metal SRAM image.
  Flash-mapped segments, PSRAM, arbitrary peripheral destinations and mismatched
  chips/revisions fail before any SRAM mutation. Unknown guest MMIO still faults.

The same guest UART0 ISR receives `1`(G28 low,blueon), `0`(G28 high,off),
`W`(G27 low,without changingG28), and `R`(explicit fixture RMT pin5). Each byte
is echoed. There is no physical programmable button or RGB. USB console is
unimplemented and is never aliased to UART; only UART0 is guest-qualified.

Host reset in an explicitly selected app-direct session resets digital
peripherals and the visible lamp, recopies that selected SRAM app and starts
its entry. This is an app-direct restart, not emulated ROM execution. Native
tests verify UART→on→off→wrongpin, reset whilelit, then UART→on again on bothSKUs.
They produce14000nominal cycles,14000slots,437retired instructions,5interrupts,
1reset,0exceptions and41UARTbytes perSKU. Idle slots are not retired instructions
or physical timing measurements. The standalone `10W` smoke produces8000cycles,
239retired,3interrupts,0exceptions,21UARTbytes.

## Review correction and retained ROM boundary

Independent review found checkpoint's `LP_CLKRST_RESET_CAUSE_REG` dropped
CORE0_RESET_FLAG bit5 and its independent SET30/CLR31 commands. The follow-up
implements initial`0x21` (chosen POWER_ONcause1+documented flagreset1), causeCLR29
preserving the flag, and flagSET/CLR preserving the cause; direct writes to RO
fields are ignored. A focused read/write test covers each case. Actual rev100
ROM remains6575slots thenloadfaultPC`0x400269b6`/address`0x600b2804`, with noUART
banner. The earlier checkpoint omission and its four-slot firstprobe remain
documented; no negative result was removed.

The W-command expansion intentionally changes the ELF bytes and UART ISR
instruction counts. Historical checkpoint hashes/counts remain in `results.json`;
the new fixture hashes and checks are in `machine-results.json`.

## Reproduce

```sh
RISCV_GCC=/path/to/riscv32-esp-elf-gcc ESPTOOL=esptool sh firmware/c5-uart-probe/build.sh
CARGO_TARGET_DIR=target/c5-build CARGO_BUILD_JOBS=2 cargo run -p esp32c5 --example app_image -- stamp-c5 firmware/c5-uart-probe/probe.bin 10W
CARGO_TARGET_DIR=target/c5-build CARGO_BUILD_JOBS=2 cargo run -p esp32c5 --example app_image -- stamp-c5-dip firmware/c5-uart-probe/probe.bin 10W
C5_ROM_ELF=/path/to/esp32c5_rev100_rom.elf CARGO_TARGET_DIR=target/c5-build CARGO_BUILD_JOBS=2 cargo test -p esp32c5 -- --include-ignored
CARGO_TARGET_DIR=target/c5-build CARGO_BUILD_JOBS=2 cargo clippy -p esp32c5 --all-targets -- -D warnings
CARGO_TARGET_DIR=target/c5-build CARGO_BUILD_JOBS=2 cargo check --target wasm32-unknown-unknown -p esp32c5
```

Eleven focused checks pass with the public ROM (ten without), nativeClippy
passes with warningsdenied, and the crate compiles forwasm32. Wholeworkspace,
actualWASM/browser and image-visibleLED acceptance remain integration work.

## Restart after a real guest fault

Review of `af662aba` found that `Machine.exceptions` is cumulative while the
C5 debug stop ceiling was fixed at1. After one actual guest load fault,
`reboot()` restored the correct app bytes but the old count immediately stopped
the new execution before READY. Explicit `boot_app()` on the same Machine had
the same problem. This negative result is preserved here.

The follow-up adds only a cold lifecycle policy:
`Soc::EXCEPTION_BUDGET_AFTER_RESET` defaults toNone; C5 opts intoSome(1).
Successful `boot_app()` and `reboot()` rebase the debug ceiling to cumulative
exceptions plus the requested budget. The total is never erased, and existing
chips retain their original default. No instruction-path arithmetic or new
Machine field is added. C5 clears its first-fault observation only after a
successful image validation/copy.

The regression executes an actual `lw t0,0(zero)` from SRAM and observes
`Exceptions(1)`, then reset→READY→UART IRQ→blueLED. It executes a second real
fault and observes `Exceptions(2)`, then explicit app boot→READY→UART IRQ→blueLED.
Cumulative telemetry remains1and2 throughout the respective restart, while
the next stop ceilings are2and3. Twelve C5 checks (including actual ROM),
209shared `esp-soc` checks and Clippy with warnings denied pass after this fix.
