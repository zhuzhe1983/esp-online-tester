# Explicit I2C source frequency

`I2c::set_external_clock_hz(hz, num, a, b)` is an opt-in source clock for a
SoC with a separately established oscillator rate. It uses the existing
divider `num + 1 + b/a` (with no fractional term when `a == 0`) for both
scheduled byte deadlines and address-phase SCL observations. The reference
deadline unit remains an 80 MHz APB tick; chip adapters convert their CPU
cycles to that unit.

This is required by the C5 boards' explicit 48 MHz crystal configuration.
C5 IDF v5.5.4 `i2c_ll.h` programs the divider in PCR, and `pcr_reg.h` defines
XTAL/RC_FAST selection separately from its enable gate. Original source
bytes are retained in
[the source manifest](../research/c5-i2c-sources-2026-11-23/manifest.json).
This shared change alone does not wire a C5 I2C controller or claim a board
factory boot.

Zero Hz is rejected atomically. The calling SoC must reject unmodeled clock
sources and handle disabled/reset gates before starting a transfer. Every
existing clock setter explicitly restores its previous source selection;
callers that do not use this new API retain 40 MHz XTAL / 17.5 MHz nominal
RC_FAST behavior. The new field lives inside the already boxed program state,
so the controller object's field layout is unchanged.

Two new tests check actual callback timing at 48 MHz, fractional division,
zero rejection, and restoration through all existing setters. The prior I2C
tests remain unchanged: 66 matching unit tests passed, zero failures.
All-target `esp-periph` Clippy passed with warnings denied. This is a
functional clock/deadline check, not a physical oscillator measurement or
whole-system performance benchmark. Validation is recorded in
[the check receipt](../evidence/c5-i2c-source-clock-2026-11-23/checks.json).
