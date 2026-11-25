# C5 rtc_clk_init modem-domain controls

This extension models the first digital clock permissions reached by the unchanged LCD273 bootloader. The measured pre-integration boundary is423286 instructions, PC`0x4084c8a8` reading PMU`0x600b000c`,229 UART bytes, real SAR reads/writes3/3, VDET02 and no application/pixels/SD commands. It does not claim PLL startup or a post-integration original-firmware result.

## Sources and scope

Fixed IDF v5.5.3 is `2c211b236707889e8400c4dc5644dd5c4ee071e0`, matching the factory app descriptor. Eight relevant header/LL/boot-source pairs are byte-identical to v5.5.4 `735507283d5b2f9fb363a1901172dbd9e847945d`; fixed URLs/hashes are in [checks.json](checks.json).

The actual [rtc_clk_init.c](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/esp_hw_support/port/esp32c5/rtc_clk_init.c) preinitialization configures ACTIVE modem code2, enables bit2 of three no-gating maps, then pairs the modem-code update and ICG-switch update commands. Its source states these domains must be available for the I2C master used during clock setup. Original instructions `0x4084c8a8..0x4084c91a` perform those exact accesses.

| Absolute address | Supported behavior |
| --- | --- |
| `0x600b000c` | ACTIVE modem code31:30; header reset0; code0 or ACTIVE2 |
| `0x600a9c0c` | MODEM_APB permission bitmap31:28; reset0; bitmap0 or4 |
| `0x600af020` | LP_APB bitmap31:28 and I2C_MST bitmap27:24; reset0; each bitmap0 or4 |
| `0x600b00dc` | Bit31WT captures requested ACTIVE modem code; reads0 |
| `0x600b00d0` | Bit28WT applies the three permissions for captured code; reads0 |

[PMU LL](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/hal/esp32c5/include/hal/pmu_ll.h), [MODEM_SYSCON LL](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/hal/esp32c5/include/hal/modem_syscon_ll.h), [MODEM_LPCON LL](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/hal/esp32c5/include/hal/modem_lpcon_ll.h) and the [PMU enum](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/esp_hw_support/include/esp_private/esp_pmu.h) establish fields, commands and ACTIVE2. Bit2=4 grants permission for code2; set map bits disable clock gating. Code1/3, other bitmap values, RF/PHY fields, other immediate commands and reserved bits remain unsupported.

## Functional update policy and genuine gates

The public PMU headers label detailed semantics `need_des`. TRM1.1 clock/reset and PMU chapters do not independently define these modem-map fields or the split of the two immediate updates. **Capture on+dc, apply on+d0 is a declared functional latch abstraction**, grounded in the complete vendor command sequence. It is not a claimed silicon micro-sequence or asynchronous timing model. R/W configuration changes alone do not alter applied permissions. Zero-valued commands do nothing; unsupported command bits reject before any side effect.

After application, master MMIO and real SAR transactions require both MODEM_APB and the modeled master-facing LP_APB permission. I2C_MST permission gates the functional clock; the existing I2C force-on bit bypasses that functional permission only. It cannot bypass either APB permission. EN/FORCE, PMU SAR power/reset, target route and controller choice remain required as before. The map does not gate unrelated LP peripherals, the CPU, flash, SHA or a nonexistent RF engine. The configuration registers remain accessible for recovery in this bounded subset.

Header reset code0/maps0 alone cannot explain the already executed three SAR masked RMWs before this preinitialization. The existing explicit virtual USB3V3 profile therefore keeps a **bootstrap-unmanaged** permission phase until the update command applies real permissions. This preserves observed guest execution without inventing a silicon reset-latch value. Reports name that phase; after application they expose the three effective permission bits and actual command counts. No ready/calibration status is introduced.

## API, reset contract and validation

The existing absolute `read(address)` / `write(address,value)` API is unchanged. Parent bus glue adds exactly the five addresses above. Diagnostic permission bits are MODEM_APB0, master-facing LP_APB1 and I2C_MST2. The previous SAR profile, genuine byte transfer engine and its negative tests remain intact.

CPU-only reset retains state. HP reset restores modem maps/controller registers and the virtual bootstrap phase while retaining PMU's LP-domain configured/captured code. LP reset clears the PMU code and returns the virtual bootstrap phase. Analog reset clears all module state and diagnostic counts. Bootstrap and latch-reset behavior are explicit functional profile choices where detailed public latch reset semantics are absent; the module does not claim general PMU sleep/wake, analog power sequencing or security retention accuracy.

Eight tests and standalone Clippy with warnings denied pass. New checks exercise the unchanged instruction/source sequence, real gate removal/restoration, functional-only force override, command capture/apply separation, WT0, invalid writes preserving every field, reset domains and preservation of the original SAR3RMW result. No bus/soc/shared code, broad build, benchmark, firmware bytes or validation was changed. DIG_REG, FOSC/PLL control, lock and calibration remain unsupported for subsequent source-backed work.

The ignored common cache retains fixed sources, typed factory disassembly and the native negative log. Committed evidence contains public hashes and aggregate checks; local identities and raw logs are omitted.

## Integrated original-factory checkpoint

The five addresses are now routed through the C5 bus. The unchanged LCD273 factory+ROM+prepared-card Machine run performs the actual code2 request, bitmap4 writes and both update triggers: effective permissions7, updates1/1. It advances from423286 to423328steps and stops atPC0x4084c924 readingLP_CLKRST_FOSC_CONFIG0x600b0418. Actual SAR transfers remain3reads/3writes; UART remains229B and no application, LCD or SD command is reached. [Integration checks](integration-checks.json) retain 94 focused passing checks, all-target Clippy and the new negative-boundary hash.
