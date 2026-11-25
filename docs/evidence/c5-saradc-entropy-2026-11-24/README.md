# C5 boot SAR configuration and virtual trigger capture

## Integrated checkpoint

The later `integration-checks.json` records the Bus/Soc wiring, exact source/input hashes, commands and limits. All266 focused C5 tests pass, including all four developer-input tests. Both workspace/native and release-WASM Clippy pass with warnings denied. After the equivalent checked-Option lint correction, all21 digital/Bus SAR tests pass again; those overlap266. The focused release suite explicitly disables LTO, so default-LTO workspace and final product checks remain pending.

The unchanged factory now completes the actual SAR sequence: guest initial-code targets2150, explicit ideal raw inputs2048,104 timer triggers and52 captures per selected pattern, with no clock fault. These are functional virtual captures, not physical conversion or entropy. The guest prints the partition table, selects `factory`, and announces application segment0 at physical`0x20020`, virtual`0x42130020`,1904700 bytes. It reaches796274 instructions/1353 UART bytes, then stops at PC`0x4084efac` reading LP_TIMER_UPDATE`0x600b0c10`. No application entry, LCD or guest TF execution is inferred.

Four Bus fixtures check actual clock/REGI2C inputs, deadline-triggered captures, independence from the deterministic RNG, CPU/Core/LP/Chip retention, transactional rejection of live unsupported power withdrawal and raw-ROM deselection. The integration does not publish rejected analog clock observations to PLL/MSPI. ADC reset and pending reports remain at the Bus service boundary; only watchdog controller resets have event-at arbitration. No asynchronous whole-system accuracy is claimed. The following source/module section retains the earlier pre-integration boundary as history.

The unchanged LCD2.73 factory image reaches `bootloader_random_enable`, then faults at PC `0x4084f778` reading `PCR_SARADC_CONF` at `0x60096088`. The pre-integration native receipt records 586172 instructions, 586112 nominal cycles, 633 UART bytes and zero nonzero pixels. TG0 flashboot has actually been cleared through UPDATE: origin0, count114777, active0, updates1. The UART's original “Enabling RNG early entropy source...” message is guest text; it does not establish real entropy in the emulator. The SAR additions described here still require bus integration and an unchanged-firmware rerun.

The committed integration base is `51119b2c530c3fb7e473ed1dfde7413ee5572d24`. The original factory SHA-256 remains `cf10a4358d03898458c16390f3f16ceb0306befe2260160de0919d755b920942`, and the rev100 ROM ELF SHA-256 remains `49693d745eb4b4e6e6c77be4f5213765d545d8c7a1d9742a8a619bf596c4e27a`. The typed disassembly of the bootloader segment at `0x4084e5a0` has SHA-256 `24c947a7383661b9ed9b89036eb85dd4e78b243faec54255ba71ea0e0b9d8014`. No firmware, NVS, ROM instruction, calibration function or checksum is patched.

Fixed source revisions are ESP-IDF v5.5.3 `2c211b236707889e8400c4dc5644dd5c4ee071e0` and v5.5.4 `735507283d5b2f9fb363a1901172dbd9e847945d`. [sources.json](sources.json) retains fixed URLs, final response URLs, UTC fetch times, media types, sizes and hashes. All 14 source pairs are byte-identical. The TRM1.1 PDF hash identifies the consulted document despite its mutable public URL. Relevant sources are `bootloader_random_esp32c5.c`, C5 `adc_ll.h`/`regi2c_ctrl_ll.h`, and the C5 PCR, APB_SARADC, PMU, REGI2C and LPPERI headers.

The typed original function at `0x4084f768`–`0x4084f956` performs the following operations. The later operations are statically decoded, not claims that the pre-integration run completed them:

| Original PC(s) | Operation |
| --- | --- |
| `0x4084f778`–`0x4084f7ae` | PCR SARADC function reset1→0, APB register reset1→0, then REG_CLK_EN2=1 |
| `0x4084f7b4`, `0x4084f7c0` | Enable CLKM22 and select0 XTAL |
| `0x4084f7ce` | Enable SAR digital clock through ADC CTRL6 |
| `0x4084f7e2`, `0x4084f7e8`, `0x4084f7f6` | CLKM NUM=0, B=0, A=0 |
| `0x4084f804`, `0x4084f80e`, `0x4084f818` | Existing PMU RF_PWC26 clear,27 set,26 set, enabling PERIF_I2C power/reset release |
| `0x4084f820`, `0x4084f832` | SAR REGI2C block `0x69`, byte7 DTEST1:0=0, ENT6=1 |
| `0x4084f844`, `0x4084f856` | Byte8 internal-reference bits0 and2 set |
| `0x4084f868`, `0x4084f87c`, `0x4084f88e`, `0x4084f8a2` | Byte1 low nibble=8, byte0=102; byte4 low nibble=8, byte3=102: two actual guest initial-code targets2150 |
| `0x4084f8cc`, `0x4084f8e8` | Set six-bit pattern0=`0x1f` and pattern1=`0x27`; final pattern-table word `0x007e7fff` |
| `0x4084f8f8`, `0x4084f916` | Set length2 and SAR divider15; final CTRL `0x400087c0` |
| `0x4084f922`, `0x4084f928` | Set timer target200, then TIMER_EN24; final CTRL2 `0x010c81fe` |
| `0x4084f93e`, `0x4084f944`, `0x4084f94c` | Existing LPPERI RNG sample, RTC-timer and RNG-timer configuration bits set |

The function starts a timer; a passive register bank is insufficient. TRM46.5.5 says the ADC timer uses ADC_CTRL_CLK and triggers after twice its target. `adc_ll_digi_set_trigger_interval` gives the same factor2. With this guest's XTAL48MHz, CLKM NUM0/A0/B0 and the LL's `(NUM+1)` integer divider, the timer uses48MHz and triggers every400 controller-clock edges, or2000 nominal240MHz scheduler cycles. SAR divider15 configures the separate SAR reader/analog clock and is not multiplied into this timer interval. Its expected value is validated for this supported mode. Physical conversion duration, analog clock accuracy and CPU DFS instruction duration are not inferred from this scheduler calculation.

TRM46 describes one physical ADC and six public channels0–5, and marks pattern bit5 reserved. The fixed C5 LL explicitly uses bit5 as an ADC-unit encoding for RNG compatibility, and its source comment says C5 has no ADC2. The original guest sets pattern0 to ADC1 reserved internal channel7/atten3 and pattern1 to the compatible ADC2 reserved internal channel1/atten3. The implementation follows only this source-backed boot compatibility route. It does not expose a second physical ADC, claim public channel7 support, substitute another chip's ADC map, or infer an internal voltage. The stale “each item one byte” pattern-header comment is superseded by the LL's six-bit shifts and the typed actual instructions.

The new digital module owns exactly:

| Register | Address | Complete header reset word |
| --- | --- | --- |
| PCR_SARADC_CONF | `0x60096088` | `0x00000001` |
| PCR_SARADC_CLKM_CONF | `0x6009608c` | `0x00004000` |
| APB_SARADC_CTRL | `0x6000e000` | `0x40038240` |
| APB_SARADC_CTRL2 | `0x6000e004` | `0x0000a1fe` |
| APB_SARADC_SAR_PATT_TAB1 | `0x6000e018` | `0x00ffffff` |
| APB_SARADC_SAR_PATT_TAB2 | `0x6000e01c` | `0x00ffffff` |

PCR CONF accepts only bits3:0. Bit0 is explicitly “no use” in the pinned header and does not silently become an APB gate; REG_CLK_EN2 controls actual ADC register access. CLKM accepts named bits22:0. CTRL permits only changes to clock6, divider14:7, length17:15 and pointer-clear23. CTRL2 permits only target23:12 and TIMER_EN24 changes; other named fields retain their complete defaults. Pattern words store24-bit fields. Unsupported/reserved changes reject transactionally. Inactive source/divider/pattern configuration can be retained, but enabling a timer with an unsupported source, fractional divider, interval, external pattern, length or mode rejects. One-shot/START, public ADC channels, filters, DMA, conversion data/status, IRQ and DONE addresses remain unknown. No readback turns START into ready or manufactures a conversion result.

The virtual profile is explicit and off by default. Integration supplies `[2048,2048]` as two ideal constant12-bit raw inputs. These codes have no voltage, calibration, noise or entropy meaning. `enable_virtual_profile(cycle, raw)` validates those inputs and returns `Result`; repeated equal selection preserves existing history. It cannot attach to an already enabled unprofiled timer or replace active input history. `disable_virtual_profile()` withdraws capture/deadlines while preserving readable guest bits. No default input or successful fallback is supplied when the profile is absent.

Each actual400-edge timer trigger captures the selected declared virtual input and advances the two-pattern pointer. This is a zero-conversion-latency functional capture policy for the bounded virtual path. It is not a physical ADC reader FSM, conversion completion or analog settling model. The public TRM restarts the timer after conversion completion; here the explicit virtual capture completes at the trigger boundary, so the next400-edge interval starts there. Telemetry exposes trigger count, per-pattern capture counts, pointer, declared inputs, last virtual codes/timestamp, divider phase and actual register fields. It does not publish ADC DONE/data/status/IRQ/DMA registers. Capture work is O(1) for any elapsed interval, including very large jumps.

`ClockInputs { xtal_hz, reserved_path_ready }` uses actual current observations. Old observations advance the timer before a new observation is latched. PCR function clock, SAR digital clock and function reset control timer progression; a known clock gate pauses and preserves phase. APB gating independently blocks MMIO. Unknown active XTAL or a live unsupported analog route returns an explicit failure and removes the schedulable deadline, rather than inventing a rate or readiness. Starts require the complete supported configuration and actual analog configuration. `deadline()` returns the next absolute trigger time for machine scheduling. Machine reset arbitration must advance this controller only to the chosen reset timestamp before publishing new clock observations.

The existing boot analog master remains responsible for its real routing, gates, controller selection, PMU power/reset release, synchronous byte access and VDET byte10. The only extension is opt-in storage for bytes0/1/3/4/7/8 in block `0x69`. They start at explicitly declared virtual zero values, because public sources give named fields without analog-byte POR values. Only low nibbles of bytes1/4, DTEST1:0 and ENT6 in byte7, references0/2 in byte8, and the full low bytes0/3 exist. Unsupported TSENS, ground-calibration, DREF and other analog targets reject. Byte10 and all previous VDET behavior are unchanged.

`sar_reserved_path_ready()` is a configuration predicate, not conversion readiness: it requires actual guest writes to all six targets, DTEST0, ENT1, both references and the existing PMU26/27 power/reset state. It does not depend on the I2C master clock as a substitute for the ADC clock. Initial-code reporting combines the actual written bytes;2150 is not host seeded, permanently required or derived from the virtual samples. Tests also write targets0 and4095. The original disable path clears TIMER_EN before resetting both pattern tables, zeroing guest calibration targets, disabling the internal route and restoring CLKM divider4/XTAL. Those configuration operations have real state effects; later activation outside the bounded mode still rejects.

LPPERI RNG is unchanged. It remains the pre-existing deterministic sample-per-DATA-read source, with no feed from this timer or its constant samples. Neither an original boot log nor successful virtual capture upgrades it to silicon entropy, a noise source, a calibrated ADC reading or a cryptographic oracle. Application security/entropy correctness remains unverified.

CPU-only and LP-only reset preserve this HP digital controller. Core/HP reset restores its digital PCR/ADC defaults and capture state; Chip/analog reset also withdraws the raw-input profile for explicit reselection. Both original PCR reset pulses converge on complete ADC defaults. The sparse public register descriptions distinguish function and APB reset but do not supply a complete per-flop retention map, so independently asserting either reset is a declared convergent functional policy, not measured silicon retention. Analog target bytes follow the existing virtual analog retention policy: CPU/Core and LP power transitions preserve written bytes, the LP PMU reset can withdraw their power predicate, and analog reset restores virtual initial bytes while retaining opt-in selection. These limits do not skip either original reset pulse or manufacture a ready/status bit.

Validation uses standalone owned-source harnesses, avoiding Cargo/shared builds:

```sh
rustc --edition=2021 --test esp32c5/src/boot_saradc.rs -o "$TASK_TMP/c5-saradc-tests"
"$TASK_TMP/c5-saradc-tests" --nocapture --test-threads=1
rustc --edition=2021 --test esp32c5/src/boot_analog.rs -o "$TASK_TMP/c5-saradc-analog-tests"
"$TASK_TMP/c5-saradc-analog-tests" --nocapture --test-threads=1
```

Unit-test operation timestamps are synthetic samples; the actual timer start must come from the guest TIMER_EN write after integration. No ADC trigger history is backfilled from the observed fault time.

The digital harness passes 17 tests; the analog harness passes 17, including its 13 previous VDET/MODEM/DIG/PLL tests. Coverage includes actual digital write order/defaults, real trigger deadlines, alternate explicit raw codes, phase/chunk invariance, independent APB/source gates, old-input ordering, actual byte transfers, strict missing configuration, field/source/mode rejection, raw/profile/reset retention and bounded huge jumps. [checks.json](checks.json) records source/output hashes and compact results. Test output is curated only by stripping leading/trailing blank lines; both raw and normalized hashes are retained. Evidence omits local home paths, personal process/session inventories and raw private captures. Bus integration, original-firmware rerun, cross-module scheduler/reset tests and full warnings-denied Clippy are separate integration checks.

完整逐 test stdout 已移至 ignored 输出；原路径、bytes/hash、固定 artifact/source receipt 与重现命令见[留存清单](../c5-evidence-retention-final-2026-11-25/stdout-manifest.json)。测试结果、负结果和历史 raw/normalized hash 保持不变；此留存整理没有运行组件或原厂机。
