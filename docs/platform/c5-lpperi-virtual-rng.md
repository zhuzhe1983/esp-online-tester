# C5 LPPERI and the deterministic virtual RNG

The C5-only adapter decodes `LPPERI` at `0x600b2800`. Its clock/reset
registers, RNG configuration, data aliases and date register use the masks and
reset defaults in Espressif IDF v5.5.4 commit
`735507283d5b2f9fb363a1901172dbd9e847945d`. Exact source bytes and retrieval
receipts are in [the source manifest](../research/c5-lpperi-sources-2026-11-23/manifest.json).

## Virtual source contract

The generated data is a deterministic simulation input. It is **not physical
entropy**, a cryptographic RNG, a calibrated ring oscillator, or a simulation
of the hardware timer/CRC mixer. The fixed initial state is `0xc5e01001`;
an enabled read of DATA or DATA_SYNC applies xorshift32 shifts 13, 17 and 5.
Each such read counts as one virtual sample, modulo 256. DATA_SYNC shares the
same generator; no additional physical synchronizer latency is claimed.

Clock bit24, reset bit24, and configuration bit0 gate sample production.
Disabled reads return the last value and do not advance state. Reset restores
the documented configuration default and this explicit virtual seed. DATA and
the sample counter are read-only. Prescaler/timer-XOR settings have masked
readback; they do not claim oscillator timing or change the virtual stream.

Only the documented digital register subset is decoded. Other LP engines,
analog I2C registers and unimplemented offsets fault. Clock/reset fields for
unmodeled engines latch, without creating those engines. The bus-wide and LP
CPU reset triggers are rejected because their effects are not yet implemented.
This code is private to C5 and does not change RNG behavior on other SoCs.

## Verification and actual ROM boundary

The two new protocol tests exercise masks, reset hold, disabled sampling,
read-only data, deterministic restart, and rejection of unknown engines.
All C5 targets passed: 31 tests, zero failures, one external-ROM test ignored
in the ordinary suite. That external test was then run separately and passed.
All-target C5 Clippy passed with warnings denied.

With the unmodified official rev100 ROM ELF from the 20260528 Espressif
release, actual guest execution now stops after 6818 steps at a load of
`0x600b1c1c`, from PC `0x4001def2`. The ROM symbol/disassembly identifies
`clear_super_wdt_reset_flag`; the register header maps it to LP_WDT
SWD_CONFIG. No firmware flash was installed in this probe. UART is empty,
and neither a ROM boot banner nor a successful factory boot is claimed.
The earlier 6575-step LPPERI fault remains unchanged in the preceding
LCD1.47 evidence as a historical checkpoint.

See [the execution receipt](../evidence/c5-lpperi-2026-11-23/execution.json)
and [the exact validation commands](../evidence/c5-lpperi-2026-11-23/checks.json).
