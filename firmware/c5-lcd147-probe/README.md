# Original C5 LCD147 SRAM diagnostic

This is a project-authored diagnostic, not a Waveshare factory image. It uses
the exact LCD147 wiring and explicit nominal48MHz crystal; SPI runs at24MHz
and backlight LEDC at5kHz. LCD pixels travel through the actual C5 GPIO matrix,
SPI2 and full-address GDMA model. BOOT28 changes a rectangle and restores it
on release. The guest saves all 512 original bytes at SD LBA32, writes and
reads a test pattern, restores the original block, then verifies the restore.
Allow initialization to finish before stopping the test. It does not mount FAT or exercise flash, PSRAM, USB, RF, physical RGB
timing, RTC or analog power behavior. This board has4MiB embedded flash and no
PSRAM according to its official hardware reference and schematic.

Build with GCC14.2.0_20260121 and esptool5.1.0 using `sh build.sh`. `RISCV_GCC` and
`ESPTOOL` may select executables; `PYTHONPATH` must point at the installed
esptool package if it was installed with pip's target option. The linker
places all code/data in HP SRAM; the image header allows C5 MP rev100…199.

Run `cargo run -p esp32c5 --release --example lcd147_probe -- firmware/c5-lcd147-probe/probe.bin /tmp/c5-lcd147-result`.
The runner uses the public App, attached SD store and named script interfaces and produces
framebuffers, PNGs, console output and an execution receipt. The corresponding
source/electrical limits are in `docs/platform/c5-display-transport.md`.

The initial native receipt (commit 1f2097ab) used an explicitly disposable card
and a different image hash. The current program restores its scratch block;
new native and public Worker results are recorded separately in
`docs/evidence/waveshare-c5-lcd147-web-2026-11-23`. No factory firmware was edited.
