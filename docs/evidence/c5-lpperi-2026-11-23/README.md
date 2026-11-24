# C5 LPPERI checkpoint, 2026-11-23

This is a fresh actual-ROM boundary probe after the C5-only digital LPPERI
and deterministic virtual RNG implementation. It is not a factory-firmware
success receipt. The fixed ROM input and normalized command/output hashes
are recorded in `execution.json` and `checks.json`.

Observed boundary: 6818 steps, exception5, trap PC `0x40001600`; first
unsupported read is PC `0x4001def2` accessing `0x600b1c1c` (LP_WDT+0x1c).
UART remains empty. No firmware flash image was loaded and no guest PC was
patched. `actual-rom-next-access.txt` is objdump output from the same ROM.

The digital protocol tests and all-target C5 Clippy pass. The virtual RNG is
explicitly reproducible simulation input, not silicon entropy. Full behavior
and omissions are described in
[the platform contract](../../platform/c5-lpperi-virtual-rng.md).
