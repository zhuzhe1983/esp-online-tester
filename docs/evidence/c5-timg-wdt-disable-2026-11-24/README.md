# C5 TIMG watchdog flashboot protection and disable

The unchanged Waveshare LCD2.73 factory image reaches a write to `TIMG0_WDTWPROTECT` at `0x60008064`, from ROM PC `0x400212f2`. The pre-integration native receipt records 573914 instructions, 573888 nominal cycles, 582 UART bytes and zero nonzero pixels. Its first exception is that exact write. This receipt describes the following original ROM sequence and the bounded controller implementation; it does not establish that the new module has executed in the factory image or that application/LCD output works.

The integration baseline is commit `8778a1bd2ee8a02215fc20c2bf7bf113341c974d`. Inputs remain the original factory image SHA-256 `cf10a4358d03898458c16390f3f16ceb0306befe2260160de0919d755b920942` and C5 rev100 ROM ELF SHA-256 `49693d745eb4b4e6e6c77be4f5213765d545d8c7a1d9742a8a619bf596c4e27a`. The cached typed bootloader segment starting at `0x4084bba0` has SHA-256 `10cd68d5f0b3f2ca718fd82d0f1b9da52fea27771463f481691f09411101aa4a`. No firmware bytes or instruction handlers are patched.

The fixed sources are ESP-IDF v5.5.3 `2c211b236707889e8400c4dc5644dd5c4ee071e0` and v5.5.4 `735507283d5b2f9fb363a1901172dbd9e847945d`. [sources.json](sources.json) retains exact URLs, response metadata and hashes for the C5 register headers, LL, HAL, bootloader and ROM symbol map. All seven source pairs are identical. TRM1.1 sections 16.2.2.3–16.2.2.4 establish write protection and independent SPI-flash boot protection; the PDF hash identifies the consulted document despite the mutable public PDF URL.

The important distinction is that `WDT_EN=0` does not deactivate flashboot protection. `FLASHBOOT_MOD_EN` has reset value 1 independently of `WDT_EN`'s reset value 0. TRM identifies MWDT0 as the automatic SPI-flash boot watchdog, with stage0 producing Core reset cause `0x07`. The same reset bit in TG1 does not establish an automatic MWDT1 boot latch. This implementation therefore has a real TG0 cold-flashboot counter and no automatic TG1 counter. Visible reset stage-action fields remain their header values 0; the special effective flashboot action is tracked separately.

The earlier original-ROM `disable_default_watchdog` path touches LP_WDT, not TG0. No prior executed TG0 disable was established. The new profile must consequently be selected at the actual fresh cold epoch, before elapsed time or owned MMIO. Attaching at the observed 573888-cycle fault and pretending to have counted the missing history is rejected.

The original disable sequence is:

| Caller / ROM PC | Operation | Effect |
| --- | --- | --- |
| `0x4084c2c6` / `0x400212f2` | Write `0x50d83aa1` to TG0 `+0x64` | Unlock configuration |
| `0x4084c2d2` / `0x400213a4` | Read TG0 `+0x48` | Reset word `0x0004c000` |
| `0x400213b0` | Write the word with bit14 cleared | Shadow becomes `0x00048000`; watchdog still counts |
| `0x400213b2` / `0x400213ba` | Read the word, then write it with bit22 set | Real UPDATE copies shadow configuration; flashboot counter stops |
| `0x4084c2dc` / `0x40021302` | Write 0 to TG0 `+0x64` | Lock configuration |

The caller invokes `wdt_hal_set_flashboot_en(false)`, not the general `wdt_hal_disable` path. UPDATE is a command, not permanent ready/status state. The test's individual instruction times 573889–573894 are synthetic adjacent samples for the exact operation order; only 573888 is the observed pre-integration fault time.

The module owns these exact word addresses:

| Registers | Addresses | Header reset / semantics |
| --- | --- | --- |
| TG0 CONFIG0–5 | `0x60008048`, `0x6000804c`, `0x60008050`, `0x60008054`, `0x60008058`, `0x6000805c` | `0x0004c000`, `0x00010000`,26000000,134217727,1048575,1048575 |
| TG0 FEED / WPROTECT | `0x60008060`, `0x60008064` | FEED reads 0, any unlocked write feeds; key reset `0x50d83aa1` |
| TG1 CONFIG0–5 | `0x60009048`, `0x6000904c`, `0x60009050`, `0x60009054`, `0x60009058`, `0x6000905c` | Same complete visible reset words; no automatic counter |
| TG1 FEED / WPROTECT | `0x60009060`, `0x60009064` | Same protected command and key semantics |
| PCR TG0/TG1 WDT clock configuration | `0x6009605c`, `0x60096068` | Source21:20 reset 0; functional gate22 reset 1 |

CONFIG0 accepts named mask `0xff9ff000` and UPDATE22. Prescaler CONFIG1 accepts bits31:16 plus divider-reset command 0. The entire 32-bit stage-hold fields and key are retained. Reserved writes reject when unlocked. A non-key value in WPROTECT ignores all protected writes, including commands, as specified; the key itself is always writable when the group APB is accessible and reset released. The key's documented POR is the unlock key itself. There are no modeled read-only watchdog status words in this address subset. FEED, UPDATE and divider-reset are commands with a readback of 0 after their real functional effects.

Configuration has separate shadow and active snapshots. UPDATE synchronously copies the former to the latter; the functional policy models no crossing latency. While cold flashboot is active, changing holds, prescale, stage actions or reset lengths rejects. The guest may feed, reset the divider phase, gate the independent source, and clear FLASHBOOT with UPDATE. Inactive configuration retains all named fields, without claiming to execute staged MWDT behavior. Every ordinary `WDT_EN=1` request and every clear-to-set FLASHBOOT re-enable request rejects. General four-stage execution, interrupt status/routing, physical reset pulse widths, GP timers and RTC calibration are outside this module. In particular `+0x68`, `+0x70`–`+0x7c` and adjacent timer addresses do not alias it. No RWDT stage0 eFuse multiplier is applied to MWDT ticks.

The declared cold profile uses the actual supplied 48MHz XTAL, source0, prescaler 1 and 26000000 source ticks. In the existing nominal240MHz scheduler this yields an absolute deadline at cold epoch+130000000 cycles. Fractional source ticks persist between pumps. FEED resets the stage counter while preserving independent source/divider phase; the TRM does not state that feeding resets that divider. DIVCNT_RST clears the accumulated functional divider phase. This explicit command policy does not claim physical subcycle synchronization accuracy. Group APB gating blocks register access without inventing an independent source-clock stop. PCR gate22 pauses source edges. Group reset withdraws the cold latch. The parent supplies actual APB/reset observations and XTAL frequency; unknown active clocks cause an explicit unsupported boundary rather than an invented rate or a successful pause.

There is a source disagreement: the fixed C5 PCR header/TRM labels source1 as RC_FAST and source2 as PLL80M, while both pinned C5 `mwdt_ll_set_clock_source` versions program 1 for PLL80M and 2 for RC_FAST. Source0 XTAL agrees. Active cold protection therefore accepts only source0. Inactive PCR source bits are retained without a functional claim for the conflicting selections. The explicit cold source and epoch are a supported virtual platform contract; they do not reconstruct undocumented early silicon mux changes, clock accuracy, reset pulse timing, or physical CPU instruction durations. CPU DFS does not change the scheduler's nominal240MHz timebase.

`enable_cold_spi_boot(cycle, ClockInputs)` returns `Result` and preserves state on failed selection. Repeating an already selected profile advances its existing history without rearming it. `disable_virtual_profile()` preserves readable configuration but withdraws timer and pending reset; raw-ROM/SRAM deselection cannot silently start another cold epoch. CPU-only reset preserves the controller. HP/Core and System reset restore TIMG/PCR configuration and disarm the cold latch without replay. Only an explicit Chip/analog reset creates a fresh epoch, requiring another explicit source-checked cold selection. Reset domains reflect the supported controller contract, not independently measured per-flop retention.

`pump(cycle, inputs)` first advances using the old source and gates. On timeout it freezes at the exact event timestamp and does not accept subsequent clock/gate/reset observations. `take_core_reset()` returns that timestamp as `Option<u64>` exactly once; the tripped state stays frozen until reset dispatch. Read/write requests at that boundary reject, so no reset-period MMIO availability is claimed. Parent arbitration must preview clones, select the earliest event across watchdogs, commit actual controllers only to that timestamp, perform the reset and then publish new clock observations. `reset_domains` cannot rewind an already committed controller; its caller must obey that chronological contract. A late pump to 228000001 thus still reports the TG0 Core event at 130000000, before a later LP event, and cannot use the late gate observations to erase it. Same-time watchdog priority and actual Machine reset dispatch are checked in the subsequent integrated checkpoint.

## Integrated checkpoint

`integration-checks.json` retains exact checked source/input hashes, commands, negative checks and limitations. All241 C5 tests pass, including all four developer-input tests; both workspace/native and release-WASM Clippy pass with warnings denied. The12 joint-controller tests are included in241, not additional passes. The focused release suite explicitly disables LTO; full default-LTO workspace and final product checks remain pending.

The unchanged factory actually executes its key/clear14/UPDATE/lock sequence. TG0 remains selected from epoch0, counts114777 source ticks before that guest disable, records one UPDATE, and reports no feed, timeout or clock fault. Its final config is`0x00048000`, key0, active0. The guest continues to586172 instructions/633 UART bytes and prints the early-RNG initialization message. Its next actual fault is PC`0x4084f778`, reading PCR SARADC config`0x60096088`. No application, LCD or TF execution is inferred.

Joint dispatch previews LP/TIMG states and commits only to the first reset time; same-time priority follows TRM1.1 Table9.1-1. Applied CPU/Core reset acknowledges coincident lower-priority LP requests while retaining its actual count/phase. Global pending reset blocks later TIMG MMIO advancement and watchdog writes. Both reset paths settle a later independent event already due at a late service point before another guest instruction, while returning the captured cause of the reset just applied. Cold profile selection is transactional, raw mode cannot restart a latch, and an explicit Chip reset creates a fresh checked epoch. The diagnostic Bringup now stops on a reset request without retiring another instruction.

Watchdog controller timestamps do not make the rest of the simulator asynchronously cycle accurate: Bus/CPU time remains at the retired-instruction service boundary and physical reset pulses are unmodeled. These bounds are explicit in the receipt.

Validation used the standalone owned-module harness, avoiding Cargo workspace/shared builds:

```sh
rustc --edition=2021 --test esp32c5/src/timg_wdt.rs -o "$TASK_TMP/timg-wdt-disable-tests"
"$TASK_TMP/timg-wdt-disable-tests" --nocapture
```

All 14 tests pass. They cover full POR words/key, exact ROM clear/UPDATE/lock order, independent flashboot activation, protected-write ignores, inactive fields, ordinary activation rejection, APB/source gates, ambiguous source rejection, real feed/divider phase effects, exact timestamp ordering/freeze, transactional cold selection, reset/deselection retention, chunk-invariant edge counting, bounded huge jumps and exact-address rejection. The evidence receipt omits local home paths, process/session inventories and raw private captures. Bus routing, original-firmware rerun, cross-watchdog integration tests and full warnings-denied Clippy remain integration checks.

完整逐 test stdout 已移至 ignored 输出；原路径、bytes/hash、固定 artifact/source receipt 与重现命令见[留存清单](../c5-evidence-retention-final-2026-11-25/stdout-manifest.json)。测试结果、负结果和历史 raw/normalized hash 保持不变；此留存整理没有运行组件或原厂机。
