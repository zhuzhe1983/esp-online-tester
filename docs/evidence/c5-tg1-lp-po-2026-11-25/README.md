# C5 independent TG1 APB control and LP pad-clock disable

This adds only PCR60096060 and LP_CLKRST600B0404 to the explicit virtual clock profile. The unchanged original startup's peripheral-disable sequence clears TG1 APB enable and writes zero to the LP clock-to-pad output word. Neither word opens a register range or creates a timer, clock waveform or calibration engine.

The fixed [C5 PCR header, IDF v5.5.3](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/soc/esp32c5/register/soc/pcr_reg.h) defines TG1 control0/APB enable and1/reset, with read-only WDT/TIMER0/TIMER1 ready bits2–4. The SDK named-field reset value is1D; the stored control is1. The implementation matches the existing TG0 contract exactly: writes may contain only bits0–4, read-only inputs are discarded, and reads return the stored low two bits plus1C while reset is clear. A held reset suppresses those ready bits. These are the existing functional-profile rules, not a claim about measured reset latency or silicon POR.

TG1 has its own control field and `BootClock::timg1_control()` getter. The getter returns1 outside the virtual profile, matching TG0's legacy default; new MMIO is still unavailable without that profile. TG1 gate/reset never stops, resets or starts TG0 RTC calibration. HP or analog reset restores both PCR controls to1; CPU and LP reset retain them. General timer clocks at PCR+58/+64 and watchdog function clocks at+5C/+68 remain separate owners and are not decoded here. Parent integration must publish the actual TG1 low control bits to the existing watchdog owner.

The [C5 LP clock header](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/soc/esp32c5/register/soc/lp_clkrst_reg.h) defines11 named clock-to-pad output gates, each with SDK reset1, giving7FF. The exact word accepts only whole-word zero, including repeated zero. Every nonzero write rejects transactionally; no output re-enable or reserved field is qualified without an implemented pad consumer. LP or analog reset restores7FF, while CPU/HP reset retains the latch. The latch has no dependency from LP_FAST, LP_SLOW, raw RC observations, CPU/AHB/APB clocks, PLL taps, RTC calibration or watchdog/LP timer clock observations. Closing it does not stop those internal clocks or synthesize a waveform.

Both fixed headers match IDF v5.5.4 at735507283d5b2f9fb363a1901172dbd9e847945d; the factory's v5.5.3 revision is2c211b236707889e8400c4dc5644dd5c4ee071e0. [checks.json](checks.json) records exact URLs/hashes and the retained paired-source provenance. No new fetch time is invented, and SDK definitions are not generalized into TRM/silicon POR claims. Existing FPGA_DEBUG behavior and raw RC getters are preserved.

The actual three-module standalone harness passes29 tests:25 original fixtures unchanged and four new fixtures. Checks cover every low-control/read-only input combination, reserved/alignment/neighbor rejection, failure-state equality, independent reset domains, all three LP_FAST selections, active PLL taps, HP bridge closure, raw RC rates and preserved FPGA_DEBUG. A running TG0 measurement still finishes at cycle30000 with6000 XTAL ticks despite TG1 reset/APB disable and LP pad-clock closure. Strict owned Clippy passes with warnings denied and no allowances. All negative write cases remain asserted; no failing test was weakened or removed.

Reproduce without a workspace build:

```sh
python3 docs/evidence/c5-tg1-lp-po-2026-11-25/run-owned-checks.py --output /tmp/c5-clock-controls-checks
```

The harness imports the actual `boot_clock.rs`, `clock_tree.rs` and existing calibration module. No Bus/Soc/lib, BootAnalog, shared module, other product, previous receipt, firmware or runtime path is changed. Full Cargo/native/WASM/browser/benchmark work was not run; parent wiring and the next original-image run remain separate integration checks.
