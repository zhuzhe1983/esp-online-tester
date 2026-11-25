# C5 SWD feed pulses use the observed LP fast clock

`LpWdt::set_fast_clock(cycle, hz) -> bool` changes only the digital SWD feed pulse rate. It accepts `None`,17.5MHz,24MHz and48MHz after the strict SWD profile is selected. Other values, including zero, reject without pumping or changing state. The default24MHz behavior, report text and28 existing watchdog fixtures remain intact. RWDT continues using its independent slow-clock observation. Analog request and expiry stay at the existing virtual204M/228M nominal240MHz cycles; these are declared850ms/950ms inputs, not measured silicon timings.

The [C5 LP watchdog header, IDF v5.5.3](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/soc/esp32c5/register/soc/lp_wdt_reg.h) defines the10-bit SWD signal width. [C5 TRM1.1](https://documentation.espressif.com/esp32-c5_technical_reference_manual_en.pdf), sections16.3 and16.6, separates the constant analog watchdog source from the feed signal measured in LP_DYN_FAST_CLK cycles. The [LP clock header](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/soc/esp32c5/register/soc/lp_clkrst_reg.h) defines FAST_CLK_SEL[3:2] as RC_FAST,XTAL/2 or XTAL. These fixed files match v5.5.4. Exact source hashes, versions and retained TRM hash are inherited from the older source receipt and referenced in [checks.json](checks.json); no new fetch timestamp is asserted.

The independent clock owner supplies the actual supported functional rate after power/tuning/mux checks. This module neither reads a requested mux nor manufactures oscillator readiness. For a fresh300-tick pulse:

| Observed rate | Completion after nominal CPU cycles |
| --- | ---: |
| 17.5MHz | 4115 |
| 24MHz | 3000 |
| 48MHz | 1500 |
| unavailable | no feed progress; analog watchdog still expires |

Each pulse retains its remaining ticks multiplied by240M and the last settled cycle. The controller pumps the old rate before changing it, subtracting elapsed cycles times the old rate with u128 intermediates. No completed tick or fractional phase is discarded at a source boundary. For300 ticks, one cycle at24MHz and one at17.5MHz leave71,958,500,000 numerator units. After switching to48MHz,1499 more cycles leave6,500,000 units and the next cycle completes the pulse at cycle1502. Losing the clock preserves that numerator; restoration resumes it. Only completion restarts analog age and increments the appropriate automatic/software counter.

At stable rates, complete automatic periods aggregate as `REQUEST + ceil(widthTicks * 240M / hz)`. Each new automatic pulse starts the declared functional phase afresh; completion rounding does not claim physical oscillator phase. Catch-up remains bounded independently of elapsed time. Checked absolute deadlines, saturating counters and bounded products cover u64 maximum time without a guest panic. Width changes and retriggers while a pulse is in flight retain their rejection rules. Turning AUTO off preserves the already issued pulse and its automatic classification.

A previously uncovered case is corrected: a software pulse begun before the850ms warning could suppress that request until completion, leaving an already-due request deadline. A missing fast source could then prevent the950ms expiry from being processed. The analog request now rises once at its fixed timestamp during an in-flight pulse, while expiry at or before completion wins. This correction does not change either analog constant or restart age. The independent timeline fixture covers both a crossing24MHz pulse and a stalled pulse through expiry.

Pending or dispatched resets prevent a changed source observation. The same observation is idempotent without advancing the pending state. Existing chronological SWD/RWDT preview/replay and tie priority remain. A focused test observes RWDT Core events at cycles2 and7 and verifies that neither future SWD progress nor a feed leaks across those barriers. CPU/HP resets retain the pulse and source. LP controller reset clears the pulse while retaining the external rate and analog age; its clock owner must publish any mux reset at the actual reset timestamp before later catch-up. Analog reset restores the default24MHz observation and a fresh analog age.

The actual-module harness passes37 tests: the original28 bodies are unchanged and nine new fixtures cover rates, phase, pause/resume, analog expiry, invalid observations, barriers, reset domains, AUTO disable and million-period/u64 catch-up. Strict owned Clippy passes with warnings denied and no allowances. The first new fixture run passed35/36: it incorrectly expected SWD completion after acknowledging only the first repeating RWDT reset. It was strengthened to assert the real next Core event at7, then explicitly stop that independent RWDT before checking SWD completion. An initial lint command used edition2024, which differs from the repository's2021; its diagnostics and the small new-code style fixes are retained in the check summary. No assertion was weakened.

Reproduce without a workspace build:

```sh
python3 docs/evidence/c5-swd-fast-clock-2026-11-25/run-owned-checks.py --output /tmp/c5-swd-fast-checks
```

The harness imports the actual `lp_wdt.rs` and its existing RWDT module; it supplies no replacement implementations. No Cargo/full build, native firmware, WASM, browser or benchmark was run. Bus/Soc clock publication and full original-image acceptance remain separate integration work. Older receipts are unchanged; their hashes are retained in this additive receipt.
