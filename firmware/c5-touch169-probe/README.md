# C5 Touch1.69 original SRAM diagnostic

This program is authored for this emulator. It is not Waveshare factory firmware. The board is16MiB flash/8MiB Quad PSRAM; the guest starts from SRAM, explicitly initializes all8MiB of the virtual PSRAM and uses it as the framebuffer read by GDMA.

`build.sh` requires RISC-V GCC with `rv32imac_zicsr_zifencei` and esptool5.1.0. The public `touch169_probe` native example loads `probe.bin`, exercises actual touch/BOOT paths and saves raw frames/PNGs. Physical pins, exact source revisions, clock choices, negative cases and factory limitations are in [`waveshare-c5-touch169.md`](../../docs/platform/waveshare-c5-touch169.md).

The guest modifies only virtual RAM and peripheral state. This board has no SD socket. It does not modify a flash or SD image, emulate a battery, prove factory compatibility, or identify the physical manufacturer of the in-package PSRAM.
