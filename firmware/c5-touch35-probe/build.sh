#!/bin/sh
set -eu
cd "$(dirname "$0")"
: "${RISCV_GCC:=riscv32-esp-elf-gcc}"
: "${ESPTOOL:=esptool}"
"$RISCV_GCC" -march=rv32imac_zicsr_zifencei -mabi=ilp32 -Os -ffreestanding -fno-builtin -fno-pic -msmall-data-limit=0 -nostdlib -Wl,--build-id=none -Wl,-T,link.ld start.S probe.c -o probe.elf
"$ESPTOOL" --chip esp32c5 elf2image --min-rev-full 100 --max-rev-full 199 --flash-size 32MB -o probe.bin probe.elf
