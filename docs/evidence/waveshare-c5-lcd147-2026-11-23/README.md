# C5 LCD147 native execution evidence

The original guest, source, ELF and ESP image are in
`firmware/c5-lcd147-probe`. `guest-command.json` records the final native
command, actual UTC interval and runner binary hash. `inputs-and-hashes.json`
records exact source/input/framebuffer hashes. The generated app SHA256 is
`36574c6f4a0362e7ef6f5f0a062ea8ec49bac374dadde176edd8a3853ce19f1f`.

The public Machine App/SD/script path produced172×320 pixels, a quiet control
with0 changed pixels,35,840 changed pixels after `gpio boot 0`, and an exact
restore after `gpio boot 1`. Native guest retirement and dispatch both total
31,000,000. There were0 exceptions and0 resets. The guest issued SD commands
and compared all512 bytes written/read at LBA32; no filesystem was mounted.
Raw framebuffers are RGB565 little endian, gzip-compressed without timestamps.

`checks.json` and logs retain29 passed/0 failed/1 external ignored C5 tests,
then the explicitly supplied actual-ROM test separately passes. Eight new
transport tests cover exact C5 routes, clock gates, serial time floor,
full-address GDMA TX/RX/owner/bounds, independent8MiB PSRAM DMA and invalid
MMU rejection, actual LCD reset/backlight wiring, and public SD delegation.
All-target Clippy passed with warnings denied. Source archive hashes and
BSP archive members were verified, and regeneration of the transport reset
metadata was byte-identical.

The actual rev100 ROM still stops after6575 steps on a load at
`0x600b2804` from PC`0x400269b6` in its RNG stack-guard setup. It has no UART
banner and has not executed the factory bootloader/application. The official
factory download receipt is retained separately; it is not credited with the
diagnostic's results. Physical4MiB flash/no PSRAM derives from the official
hardware reference/schematic, not from this guest's allocation.

No browser/WASM result is asserted by this receipt. See
`docs/platform/c5-display-transport.md` for protocol boundaries, exact source
pins and unsupported timing/peripherals. Local executable/checkout paths in
logs are normalized; `path-normalization.json` retains original and normalized
hashes. This changes no measured value or guest output.
