#!/bin/sh
set -eu
cd "$(dirname "$0")"
: "${RISCV_GCC:=riscv32-esp-elf-gcc}"
"$RISCV_GCC" -march=rv32imac_zicsr_zifencei -mabi=ilp32 -nostdlib -Wl,--build-id=none -Wl,-T,link.ld probe.S -o probe.elf
