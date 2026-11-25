# C5 super-watchdog automatic feeding

The original LCD273 bootloader writes `0x12c40000` to `LP_WDT_SWD_CONFIG` at `0x4084bcac` after unlocking the separate SWD key. This enables automatic feeding while preserving signal width300. Before this extension, the actual unchanged ROM/factory run stops on that store after422856 instructions, with229 UART bytes, real SAR reads/writes3/3 and VDET byte02. Application entry, pixels and SD commands remain absent. The new full-ROM result requires parent bus integration.

## Exact sources and timing distinction

Fixed IDF v5.5.3 is `2c211b236707889e8400c4dc5644dd5c4ee071e0`; v5.5.4 is `735507283d5b2f9fb363a1901172dbd9e847945d`. The seven compared source pairs are identical; [checks.json](checks.json) retains URLs and hashes.

- [C5 LP_WDT register definitions](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/soc/esp32c5/register/soc/lp_wdt_reg.h) specify separate RWDT/SWD protection words, AUTO18, flag-clear19WT, width29:20, DISABLE30, FEED31WT and flag0RO. Interrupt raw/status/enable/clear use bit30 for SWD and bit31 for RWDT.
- [C5 LP watchdog LL](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/hal/esp32c5/include/hal/lpwdt_ll.h) supplies both C5 keys as `0x50d83aa1`; its timer APIs cover RWDT. C5 has no separate SWD LL in this version's HAL directory. The [actual bootloader function](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/bootloader_support/src/esp32c5/bootloader_esp32c5.c) uses direct SWD register writes, matching factory instructions `0x4084bc8a..0x4084bcae`.
- [C5 TRM1.1](https://documentation.espressif.com/esp32-c5_technical_reference_manual_en.pdf),16.3, describes an independent analog watchdog with a fixed, nonselectable clock, timeout below about1s and warning roughly100ms earlier. It does not publish an exact analog oscillator rate or timeout. A warning requests feeding; automatic mode lets the controller feed without CPU intervention. Expiry requests System Reset, cause`0x12`; its flag must be inspected and explicitly cleared after reset.
- Width300 is the length of the signal sent to the analog circuit in **LP_DYN_FAST_CLK cycles**, not the analog timeout. TRM9.2.4.2 and [C5 LP_CLKRST reset selection](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/soc/esp32c5/register/soc/lp_clkrst_reg.h) give active-mode LP_FAST reset sourceXTAL/2, hence24MHz for this48MHz board. Width300 therefore spans12.5us, represented as3000 cycles on the emulator's nominal240MHz timeline. RC_FAST, sleep switching and calibration are not substituted for this source.
- [C5 interrupt enumeration](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/soc/esp32c5/include/soc/interrupts.h) evaluates `ETS_LP_WDT_INTR_SOURCE` to18. Register bit30 is not source30.

## Explicit virtual profile and real lifecycle

`new()` retains the existing strict register subset. Only `enable_virtual_swd(cycle)` after exact ROM validation enables the autonomous profile. Its **chosen virtual** analog expiry is950ms and request is850ms; neither number is a measured/exact silicon value. POR DISABLE=0 means the watchdog is active from that cold-power-on cycle, not first activated by the bootloader's AUTO write. The current ROM prefix is only about1.76ms on the nominal emulator timeline, so it has not reached the first warning.

At a real modeled warning, the engine asserts the feed request and SWD interrupt raw state. AUTO causes a pulse of the guest-configured nonzero width. Only completed pulses restart the analog age and increment feed counters. Software FEED is a genuine protected write-trigger pulse; it reads zero afterward. A late pulse that finishes at/after expiry cannot prevent that reset. AUTO enabling alone does not count as a feed. Turning AUTO off leaves an already issued pulse intact; disabling the watchdog cancels the pulse and stops deadlines. Re-enabling starts a new virtual watchdog age.

The raw interrupt is hardware-set by an unserved analog feed request; software acknowledgement cannot suppress that continuing source. After an actual feed removes the request, RAW WTC/CLR WT acknowledge the latched interrupt. ENA controls delivery through source18, while ST is RAW&ENA. Neither RO stores nor reset-flag clear create feeds or events.

Expiry latches flag0, emits one pending logical System Reset event and records the trip. It does not queue repeated host resets or silently restart the analog timer. The unserved request/trip persists until a real feed or disable. This is a functional event model; the undocumented physical reset pulse waveform, repeated reset oscillation and clock-edge synchronizers are not claimed.

The engine uses lazy absolute deadlines and arithmetic aggregation for repeated automatic cycles. Tests compare irregular and single pumps for widths1/300/1023 and exercise one million completed automatic cycles without a growing queue. Checked deadlines never wrap; behavior beyond the representable u64 timeline is not a physical-time claim.

## Integration contract and reset scopes

Existing `read(off)` and `write(off,value)` use offsets from base`0x600b1c00`. The parent must call `pump(current_cycle)` before every MMIO access, and when `deadline` is due. `irq()` supplies source18; `take_reset()` consumes an actual expiry event for reason`0x12`. `report()` names the virtual timing and reports request/feed/reset counts. Parent owns all bus/soc/IRQ/reset wiring.

`reset_domains(cycle,lp,analog)` preserves the controller and analog age on CPU/Core reset. LP digital reset restores controller config/keys/interrupt enable and cancels its pulse; the pending analog request, age, trip and reset flag remain. A continuing request reasserts raw state. This permits the documented post-reset flag check and real AUTO recovery. Analog reset restores all controls and a fresh virtual age. A dedicated LP_CLKRST WDT reset can use the LP-controller reset path; its physical analog-latch details remain outside the public-source precision contract.

`disable_virtual_swd()` removes the autonomous profile, deadlines and pending host reset event when selecting SRAM or raw-ROM modes; readable configuration/flags remain. Legacy active-AUTO, active FEED and re-enable operations still reject. Old SRAM constructors and their two LP_WDT tests remain unchanged.

Active RWDT, stage actions (including flashboot actions when EN31 is clear), chip-reset enable, RWDT interrupt enable, reserved SWD bits, zero-width active configuration, pulse retrigger/width changes while busy, feed-while-disabled, reserved DATE clock debug, other clocks and sleep modes remain unsupported. Locked writes retain hardware ignore semantics. No register RAM, calibration completion, fabricated feeding or reset-free active watchdog is supplied.

## Validation and evidence retention

Eight module tests, both existing LP_WDT integration tests and standalone Clippy with warnings denied pass. The first Clippy attempt found an unnecessary unwrap; it was corrected without allowances. Ordinary library lint awaits parent wiring of the private APIs; no unused-method allowances were added. No shared file, broad suite, benchmark, firmware byte or validation was changed.

Version-pinned sources, original typed factory disassembly and native baseline stay in the ignored common research cache. Committed evidence contains public source hashes and aggregate checks; raw logs and local workspace identities are omitted.

## Bus integration checkpoint

The strict-ROM loader now enables the profile at its current cycle, and raw-ROM/SRAM selection disables it. MMIO and due deadlines pump actual state; source18 reaches the real interrupt matrix, and only expiry requests reset0x12. CPU/Core reset retains LP state; System reset retains the analog request and reset flag, allowing actual guest-style AUTO recovery. An external-input integration regression exercises these events through the bus, using the exact pinned ROM.

The original factory probe now passes the SWD store and stops reading the LP clock mux at423210steps. A narrowly supported documented POR mux0x4 then advances it to423286steps, PC0x4084c8a8 readingPMU_HP_ACTIVE_ICG_MODEM0x600b000c. The LP mux reports selection only; alternate sources/dividers reject. Application/LCD/TF remain unverified. [Integration checks](integration-checks.json) retain 146 full C5 passes before that narrow mux extension, 9 final boot-control passes, focused Clippy and exact negative-boundary hashes.
