# C5 LP_WDT control checkpoint, 2026-11-23

Fresh validation after `760b8840` plus the private C5 LP_WDT control patch.
The test logs, actual-ROM boundary and Clippy output are preserved with exact
commands and normalized-path hashes in `checks.json`.

The official rev100 ROM input is the same 472300-byte ELF recorded in
[`../c5-memory-2026-11-23/inputs.json`](../c5-memory-2026-11-23/inputs.json),
SHA256 `49693d745eb4b4e6e6c77be4f5213765d545d8c7a1d9742a8a619bf596c4e27a`.
No firmware flash was loaded and no boot success is asserted.

Observed: 6825 instructions; exception5; first fault PC `0x4002c8ec`,
address `0x600c8004`, read32. `actual-rom-next-access.txt` disassembles the
same ELF at `Cache_Mask_All`. UART is empty. The patch does not model active
watchdog timing, clock failures or interrupt/reset delivery; see
[the explicit runtime contract](../../platform/c5-lpwdt-controls.md).
