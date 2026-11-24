# Explicit C5 LEDC configuration

`LedcLayout::C5` describes the six-channel ESP32-C5 block. It has the
20-bit timer/duty fields, a nine-bit GPIO output selector with inversion at
bit 9, output signals 0 through 5, and gamma range RAM at
`0x400 + channel * 0x40`. The first range's `DUTY_NUM` occupies bits 30:21.
Clock frequency comes from the C5 adapter's PCR clock selection; zero is
unknown or stopped and does not select a legacy 40 MHz crystal.

The fixed ESP-IDF v5.5.4 register, HAL and GPIO sources are retained in the
[source manifest](../research/c5-ledc-sources-2026-11-23/manifest.json).
Existing S3, C3, C6 and P4 branches keep their prior behavior. Static duty
updates latch at timer wrap. Gamma fades remain unsupported and neither
change the published duty nor produce a successful completion interrupt.

`next_output_change()` returns the APB-reference ticks until an already
pending static duty update latches. A chip adapter can use it to schedule a
single output refresh without querying a board every CPU instruction. The
method adds no callers to existing chips and does not advance the timer.
The C5 adapter is responsible for converting reference ticks to its clock,
applying PCR and IO_MUX gates and initializing C5-specific register defaults.

[Focused evidence](../evidence/c5-ledc-2026-11-23/results.json) covers all four
legacy layouts and the new C5 channel limit, matrix inversion, clock gating,
exact latch boundary, lazy deadline and unsupported fade behavior. Full
workspace and product acceptance remain separate checks.
