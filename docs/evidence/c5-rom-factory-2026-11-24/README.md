# C5 ROM startup bring-up (incomplete)

The priority device is Waveshare ESP32-C5-LCD-2.73 (32 MiB Flash,8 MiB PSRAM,48 MHz XTAL,ILI9488 320×320 RGB666,CH32V006 reset/PWM,PREV2/NEXT3/BOOT28 and SPI microSD). Its unchanged manufacturer factory image is pinned in `checks.json`; bootloader0x2000,application0x20000. LCD1.47 remains a4MiB/no-PSRAM control, not substitute acceptance.

This checkpoint has reached the original factory bootloader, but **not its application or LCD pixels**. The actual rev100 ROM computes SHA through the C5 guest hardware engine, copies all three unchanged bootloader segments and jumps to0x4084bbaa. A source-pinned chip-level regression verifies23,072 loaded bytes against the original Flash payload at422498 instructions, with229 actual UART bytes and no fault. The earlier full LCD2.73 Machine probe reached422509steps and stopped atPC0x4084bc3a readingPCR_RESET_EVENT_BYPASS0x60096174. After actual analog bus integration, the common validated merged-image loader reaches422856steps and stops atPC0x4084bcac writingLP_WDT_SWD_CONFIG0x600b1c1c. The original SAR sequence performs three actual reads and writes, changing the explicit virtual VDET byte0x0f to0x02. Public SRAM-only gates remain until genuine app/display/inputs/storage acceptance.


## Source and state contracts

The LCD2.73 app identifies IDFv5.5.3,commit`2c211b236707889e8400c4dc5644dd5c4ee071e0`. Existing core model sources use v5.5.4,commit`735507283d5b2f9fb363a1901172dbd9e847945d`. The23 fixed boot/PMU/PCR/eFuse/USB/bus-monitor source files listed in the receipt were fetched in both revisions and are byte-identical. This comparison does not establish equivalence of every peripheral or all SDK code.

- PMU_IMM_PAD_HOLD_ALL+0xe4 accepts only documented self-clearing release triggers29/31. Pad holds remain rejected. LP_AON_GPIO_HOLD0 starts at documented zero and rejects nonzero hold requests until physical latches exist. Sources: C5`pmu_reg.h`,`gpio_ll.h` and TRMv1.1.
- PCR root-clock query exposes the configured48MHz XTAL and documented divider/debug reset fields. CPU/AHB/APB divider controls retain configuration; unmodeled RC/PLL selections reject. Dynamic CPU clock/analog PLL behavior is not claimed.
- UART0/1 PCR controls reset the actual UART controller, preserve already-transmitted console history, gate APB/FIFO/RX operations and reject unavailable clock sources/memory-power-down.
- LP_AON_STORE0..9 are real software scratch storage, POR zero, retained on the modeled HP reset. Configured strapping0x18 follows C5 TRM10.2 GPIO27/28 pull-ups; floating fields are explicitly virtual LOW. BOOT presses do not rewrite latched straps.
- Detached USBSerialJTAG uses a bounded64-byte TX FIFO with sealing. Without a host it never fabricates consumption or a connected endpoint. Other USB/JTAG engines remain unavailable.
- The C5 eFuse and SYSTIMER retain their separate [eFuse](../c5-efuse-2026-11-24/README.md) and [timer](../c5-systimer-2026-11-24/README.md) source/behavior contracts. Timer alarms use actual elapsed counter ticks and sources61–63.
- [Bus monitor](../c5-bus-monitor-2026-11-24/README.md) records actual PC/SP, CPU data/peripheral transfers and pre-exception PC, with IRQ27 and audited reset retention. A C5-only typed `Observer<C5>` supplies concrete CPU state. Fetch/DMA/host operations are excluded; trap stops clear the CPU context. Shared CPU/Bus/Core hot paths were not modified, and the default SRAM constructor does not install this observer. CLIC-local traffic is outside the modeled external bus monitor.

`esp32c5::load_rom` and the shared `Machine::load_rom` entry validate the exact472300-byte MP ELF SHA-256 and audited layout transactionally and reconstructs only its declared ROM-side initializer payloads. It leaves SRAM untouched. The real guest reset copy loop copies2188 bytes before ROM main at5914 instructions. The old exact6831-step PMU-failure assertion was replaced by this actual-reset-loop regression; the historical failure receipt is preserved. No function stubs, instruction modifications or host-side copy-loop execution were introduced. The default `Soc::load_rom_override` cold hook returns None for other chips; C5 uses it before generic ELF staging. Open upstreamPR263 andPR382 were inspected immediately before adding the hook; neither adds a ROM override. Their immutable heads are recorded in the receipt.

## Reproduce

Download inputs from the immutable URLs in `checks.json`, verify their SHA-256 values, then run:

```sh
cargo run --release -p esp32c5 --example factory_boundary -- waveshare-c5-lcd273 32 8 "$C5_ROM_ELF" "$C5_LCD273_FACTORY" 1000000
cargo run --release -p esp32c5 --example factory_machine -- waveshare-c5-lcd273 32 8 "$C5_ROM_ELF" "$C5_LCD273_FACTORY" 1000000
C5_ROM_ELF="$C5_ROM_ELF" cargo test --release -p esp32c5 --test guest --test cpu_monitor -- --include-ignored
cargo test --release -p esp32c5 --test boot_controls --test lcd273 --test machine --test psram
cargo clippy --release -p esp32c5 --all-targets -- -D warnings
```

The [MSPI boot module](../c5-mspi-boot-2026-11-24/README.md) now models the reached controller transfers. Strict MP loading alone enables two bounded opaque SPI0 configuration words witnessed in the original mask ROM. SRAM loading and raw ROM writes disable that compatibility; HP reset retains only an already-validated profile. Other values, SPI0 software commands and unknown engines still reject. The source conflict and virtual initial values remain explicit, with no claimed silicon register semantics.

The [guest SHA module](../c5-sha-boot-2026-11-24/README.md) performs real CPU-block SHA1/224/256 compression from guest M/H words, with15 independent NIST answers and context-restore checks. SHA DMA and other crypto engines remain unavailable.

The [cache boot](../c5-cache-boot-2026-11-24/README.md) and [freeze](../c5-cache-freeze-2026-11-24/README.md) modules now run the reached reset/invalidate/freeze lifecycle on an explicit coherent no-line representation. Their source-conflicted power/freeze semantics and unsupported modes remain stated; no physical hit/miss, ACK_BUSY/ACK_ERROR or timing accuracy is claimed.

Both native probes accept a final optional IMG argument. LCD2.73 acceptance uses
the platform's deterministic64MiB FAT32 image from`createFat32Image()` in
`web/platform/sd-card-image.js`; the receipt pins its hash. The probes copy that
file into`MemStore`, so guest writes do not alter the source IMG. The prepared-card
run reaches the same current boot fault with zero guest SD commands. A prepared
card and existing SRAM SD diagnostics do not establish factory filesystem mounting.

Focused checks pass as listed in the compact receipt. The native Machine probe uses the common scheduler; no browser factory run, workspace gate, performance comparison or complete product acceptance is claimed yet. Exact silicon clock timing, RF, USB host/PHY, analog power and broad reset-domain coverage remain outside this checkpoint. Existing SRAM product diagnostics are separate results.

Original input/log hashes and negative boundaries are retained. Raw private host paths and transient console/tool logs stay in ignored local storage; no external archive upload is claimed. The public inputs and commands reproduce the bounded claim. Manual privacy review and the repository evidence checker apply before publication.

## Context-aware cold image loading

`Machine::check_image_at` optionally delegates to the SoC with the actual physical capacity, placement and validated-ROM state. Other chips keep their existing checks. C5 merged inputs require exactly one completely validated aligned partition table; ambiguous or excessive candidates reject before Flash mutation. Standalone images use their actual placement and declared capacity. SRAM app-direct remains strict without an active validated ROM. Raw page writes and storage restores do not pass through this image-only hook. The common Machine probe now loads the pinned ROM followed by the original merged factory image via `LoadKind::Flash`, with identical original input hashes.

[Context and analog integration checks](context-loader-checks.json) record 138 passing C5 tests including the available external inputs, focused Clippy, exact native boundary and raw-log hashes. This is a loader and bootloader checkpoint, with no application, LCD or filesystem success claim.

## Clock and Flash foundation checkpoint

Strict ROM validation alone selects the functional clock/PLL/watchdog profiles. Raw-ROM writes and SRAM selection clear source observations, deadlines and pending clock work. Analog controls pump the old clock inputs before their writes and new observations afterward. The actual programmed PLL enables CPU80MHz/AHB40MHz configuration and supplies MSPI480MHz plus SPI2/LEDC source observations; requested mux values alone cannot grant a clock. Source loss withdraws access or cancels an incomplete whole SPI2 transfer. The scheduling timeline stays nominal240MHz, with no physical DFS-cycle accuracy claim.

The original bootloader performs13 REGI2C reads and15 writes, four bias-target writes and one finite virtual PLL calibration before its real QIO/32MB UART messages. The disabled PMU/BOD interrupt subset accepts only remaining disabled; unimplemented event generators cannot be enabled. [Foundation checks](clock-foundation-checks.json) retain194 C5 passes with all four pinned external-input tests, both warning-denied Clippy configurations, source hashes and earlier failures. Full-workspace default-LTO tests and actual app/browser/product acceptance remain pending.
