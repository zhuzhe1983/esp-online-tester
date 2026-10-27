#!/usr/bin/env bash
set -euo pipefail
task_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
task_output=${1:?Supply an ignored output directory, for example output/s31-uart-probe}
task_prefix=${RISCV_TOOLCHAIN_PREFIX:-riscv32-esp-elf-}
mkdir -p -- "$task_output"
for task_name in probe irq-probe trng-probe wdt-probe mmu-probe; do
  "${task_prefix}gcc" -march=rv32imafc_zicsr -mabi=ilp32f -nostdlib -nostartfiles \
    -Wl,--build-id=none -Wl,--no-relax -Wl,-T,"$task_dir/probe.ld" \
    "$task_dir/$task_name.S" -o "$task_output/$task_name.elf"
  "${task_prefix}objdump" -d "$task_output/$task_name.elf" > "$task_output/$task_name.disassembly.txt"
done
"${task_prefix}gcc" -march=rv32imafc_zicsr -mabi=ilp32f -nostdlib -nostartfiles \
  -Wl,--build-id=none -Wl,--no-relax -Wl,-T,"$task_dir/flash-worker.ld" \
  "$task_dir/flash-worker.S" -o "$task_output/flash-worker.elf"
"${task_prefix}objcopy" -O binary --only-section=.text "$task_output/flash-worker.elf" "$task_output/flash-worker.bin"
"${task_prefix}objdump" -d "$task_output/flash-worker.elf" > "$task_output/flash-worker.disassembly.txt"
