# C5 LP_WDT digital control subset

`0x600b1c00` is decoded from Espressif's C5 register map. The implementation
uses the exact reset values and register definitions in fixed IDF v5.5.4
commit `735507283d5b2f9fb363a1901172dbd9e847945d`; see
[the original-source manifest](../research/c5-lpwdt-sources-2026-11-23/manifest.json).

RWDT and SWD have separate protection registers. Both C5 keys are
`0x50d83aa1`, explicitly defined by C5 `lpwdt_ll.h`; no older-chip key is
accepted as an alias. Locked configuration writes do not mutate state.
RWDT stage-hold values, disabled configuration, the write-only feed trigger,
SWD configuration and its read-only reset flag have distinct state. WT fields
do not latch. Guest writes cannot manufacture a reset flag. Unit tests seed an
internal flag only to verify the protected clear trigger; production startup
always uses the documented clear flag.

This is a digital configuration subset, **not an autonomous watchdog**.
LP clock failure, analog timeout/reset signals, counter timing, flash-boot
auto-enabling and interrupt generation are unimplemented. RWDT starts
disabled at POR; an unlocked write enabling it faults atomically. SWD retains
its documented POR configuration, without inventing a clock-failure event.
Changing from disabled back to enabled, changing automatic-feed while active,
or requesting an active SWD feed is rejected. Interrupt registers remain
unmapped. No guessed LP oscillator rate or software completion flag is used.

Three new tests cover POR, separate keys, read-only and write-trigger behavior,
and atomic rejection of unsupported active modes. All C5 targets passed
34 tests, zero failures, one external-ROM test ignored in the ordinary run.
The actual external-ROM test then passed separately; all-target Clippy passed.

With the unchanged official 20260528 rev100 ROM, execution now reaches
6825 steps and faults at PC `0x4002c8ec` reading `0x600c8004`. The actual ROM
disassembly names `Cache_Mask_All`; this next unsupported block is C5 CACHE.
No firmware flash was installed, no PC was altered, and UART remains empty.
Prior 6575/6818-step checkpoints are retained as historical evidence.

Commands, output hashes and the actual ROM disassembly are in
[the validation receipt](../evidence/c5-lpwdt-2026-11-23/checks.json).
