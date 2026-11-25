# C5 SEC input clock selector

The parent unchanged-factory run stops on READ `6009613c` at guest PC `42002542`:27039905 instructions,27039872 nominal cycles,3711 UART bytes,cause5. The original log is10127 bytes/SHA `343092d814921b0232115aec3a51a9539fb9b4f35dc02c5ba9c7edebbdb60097`. This change implements that exact clock-mux configuration boundary in BootClock. It has not been run against the factory here; subsequent writes, Key Manager initialization, FreeRTOS and application output are not execution claims.

Only `esp32c5/src/boot_clock.rs` and this new evidence directory change, from baseline `3b57e6053b2ce75f319726769c1862c100fe833a`. The previous WDT branch/ref remains preserved. ClockTree, calibration, analog, Bus, Soc, lib and all shared tests are unchanged.

## Fixed register and real request

The fixed IDF v5.5.3 ref is `2c211b236707889e8400c4dc5644dd5c4ee071e0`; v5.5.4 is `735507283d5b2f9fb363a1901172dbd9e847945d`. [source-pins.json](source-pins.json) retains paired URLs, hashes and available capture metadata; each of these four source pairs is identical between those revisions:

| Fixed C5 primary source (link553) | bytes / SHA-256 |
| --- | --- |
| [pcr_reg.h](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/soc/esp32c5/register/soc/pcr_reg.h) | 103653 / `e2537baeb66eabd30ae7e4663ec28b7afeb6cba68ab9cb19d4e33d1d5bd33d3a` |
| [pcr_struct.h](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/soc/esp32c5/register/soc/pcr_struct.h) | 76955 / `97c2ec971bbc3935d2634e36b2ba487e5f8253ee0b5c32c90fa29d253340bc11` |
| [esp32c5/esp_crypto_clk.h](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/esp_security/src/esp32c5/esp_crypto_clk.h) | 346 / `8e7f128b3b483172ba8566a6704acc94d7a264f81f3b8e13466028ce53d3d28d` |
| [esp_security/src/init.c](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/esp_security/src/init.c) | 4720 / `7787a1b730c1e093f7d030c854321ac5fd336d8493ed3c599299b5747179553e` |

PCR_SEC_CONF at+13c has selector[1:0],POR0:0=XTAL,1=RC_FAST,2=PLL_F480M; selector3 is unspecified. Bit2 resets SEC and has POR0; bits31:3 are reserved. The narrow supported writes are whole-word0 or2. RC_FAST selection, selector3, reset2 and every reserved-bit request reject. Reads return the real qualified mux latch, with no fabricated READY or completion bit. The unselected legacy/SRAM path keeps its prior unsupported behavior.

The fixed C5 `esp_crypto_clk_init` sets the selector to2 and describes480M SPLL as its source. Fixed `esp_security_init` calls this before `esp_key_mgr_init`. The original typed routine at42002542 reads the word,42002544 clears the selector bits,42002546 ORs2,4200254a stores it. The model POR0 makes the expected resulting write2; only the failed READ has actually been observed in the parent run. No PC bypass, eFuse override or force-success call is added.

The input hashes remain the original factory `cf10a4358d03898458c16390f3f16ceb0306befe2260160de0919d755b920942`, C5 rev100 ROM `49693d745eb4b4e6e6c77be4f5213765d545d8c7a1d9742a8a619bf596c4e27a` and typed app seg2 `3e23c10184628e7c5dcea63972581d1683ba76345ad250af08dbb22c43646a45`. Raw captures/firmware are not copied into this receipt.

## Pre-DPA input is not a consumer clock

`sec_source_hz() -> Option<u32>` reports only the selected **pre-DPA source**:48MHz XTAL for selector0, or480MHz SPLL for selector2 when the actual caller-supplied `ClockInputs.pll_ready` is true. Source2 while PLL is unavailable returnsNone; the write does not grant readiness or fall back to XTAL. `sec_clock_source() -> Option<u8>` exposes the qualified raw selector; both getters returnNone without the strict profile.

PLL_F480M is the direct PLL source, independent of the divided taps gated by PCR+128. The fixture leaves only the USB48 tap enabled and still observes the available480M input; the factory-snapshot fixture preserves CPU240MHz,AHB/APB40MHz and divider-gate mask23 while selecting SEC input2. This does not claim a480MHz functional encryption or Key Manager clock.

[TRM1.1](https://documentation.espressif.com/esp32-c5_technical_reference_manual_en.pdf) section9.2 CRYPTO_CLK states a functional limit of160MHz and a separate random DPA-divider strategy controlled through HP_SYSTEM_SEC_DPA_CONF/eFuse settings. The PDF is identified by SHA `b2e7944dec0ee8e933042de69eb418da28a959d75afddde660775e9395739240`. No DPA distribution, divider result, encryption engine or Key Manager functional timing is modeled here. Initial `sec_clock_hz` naming was rejected during source review because it could imply an incorrect consumer rate; the final API and report explicitly say pre-DPA input. No compatibility alias returns a guessed160/480MHz consumer clock.

The later original path through PCR+164/+168 and60087028 belongs to Key Manager. An early XTS_AES identification was corrected by the fixed-source audit; none of those words is accepted by this patch. Their reset, memory power, READY and state need a separate owner. Source availability must not be used to fabricate those states. `None`/`false` for unsupported mux controls is a simulator qualification boundary, not evidence of a hardware abort; parent strict-profile glue owns that classification.

## State, reset and checks

`pump` publishes the observed PLL readiness only while the strict clock profile is selected. Rejected SEC values are checked before pumping, so a failed transaction changes neither mux, existing actual observation nor calibration state. CPU/LP reset retains the mux and observed PLL state. HP reset restores mux0 while retaining the external analog observation; analog reset additionally withdraws that observation. Raw-profile deselection clears both, and later selection cannot reuse stale readiness. There is no new timeout, per-tick consumer or ready scheduler.

Default report literals remain unchanged. A selected PLL input adds `SEC_input_mux=PLL480 pre_DPA_source_hz=... consumer=unmodeled`. Existing clocks/calibration continue their original behavior; selecting SEC neither commits a CPU root nor completes RTC calibration.

```sh
python3 docs/evidence/c5-sec-clock-2026-11-25/run-owned-checks.py --output "$TASK_TMP/final-checks"
```

The actual BootClock/ClockTree/calibration harness passes35 tests:all29 previous test bodies are byte-identical, plus6 focused fixtures. Stable Rust1.100.0/edition2021 strict clippy-driver passes with warnings denied and no allowances. New checks cover exact POR/RMW/profile gating, direct source versus PLL taps, readiness loss/restoration, atomic invalid values, exact-address/neighbor rejection, CPU/HP/LP/analog/raw retention, active RTC calibration and the parent-observed CPU240/AHB40/APB40 clock snapshot.

Retained negative results:the first focused run passed33/34 because a fixture incorrectly copied a65M shortened-calibration timeout expectation; the fresh POR threshold33554431 plus terminal edge at48MHz gives167772160 nominal cycles. Only that expected value changed. The next strict lint flagged adjacent independent reset `if` statements on one line; splitting the line fixed it without a behavior change or allowance. Initial guessed SEC/XTS LL file paths returned404; ignored connector receipts retain them, and only the correct fixed C5 setter is cited as evidence. Latest-branch search was used for discovery only. [checks.json](checks.json) records these results, exact source/output hashes and limits.

No Cargo/workspace/native/WASM/browser/benchmark or push ran in this lane. Parent owns shared integration, Key Manager implementation and the next unchanged-factory run. This receipt retains compact numeric/source/command facts and omits raw private traces, home paths and process/session inventories.
