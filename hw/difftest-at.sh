#!/bin/sh
# Run both hardware and emulator to a breakpoint, then single-step N instructions and compare.
#   hw/difftest-at.sh <pc-hex> [steps]
set -e
cd "$(dirname "$0")/.."
# Inputs (flash dump, reset-regs.txt) come from DIFF_DIR. Everything read from the chip goes to
# DIFF_OUT, a fresh temporary directory by default: the efuses hold the chip's MAC address and other
# device identifiers, which must not land in tracked files (AGENTS.md).
D=${DIFF_DIR:-hw}; O=${DIFF_OUT:-$(mktemp -d "${TMPDIR:-/tmp}/esp32sim-difftest.XXXXXX")}; mkdir -p "$O"
FLASH_MB=${FLASH_MB:-16}; IMG=${FLASH_IMAGE:-$( [ -f $D/flash-8M.bin ] && echo $D/flash-8M.bin || echo $D/flash-0-1M.bin )}
PC=${1:?pc}; N=${2:-3000}
OCD=$(ls -d ~/.espressif/tools/openocd-esp32/*/openocd-esp32 | head -1); SCR=$OCD/share/openocd/scripts
GDB=$(ls ~/.espressif/tools/xtensa-esp-elf-gdb/*/xtensa-esp-elf-gdb/bin/xtensa-esp32s3-elf-gdb | head -1)
if P=$(pgrep -f openocd-esp32); then echo "openocd is already running (PID $P); stop it first" >&2; exit 1; fi
echo "captures in $O"
# 1) real efuses + strap, as in difftest.sh
$OCD/bin/openocd -s $SCR -f board/esp32s3-builtin.cfg -c init -c halt -c "sleep 200" -c "mdw 0x60007000 128" -c "mdw 0x60004038" -c resume -c shutdown 2>&1 | grep -E "^0x6000" > "$O/jtag-regs.txt"
grep "^0x60007" "$O/jtag-regs.txt" > "$O/efuse.txt"
STRAP=$(grep "^0x60004038" "$O/jtag-regs.txt" | awk '{print $2}')
$OCD/bin/openocd -s $SCR -f board/esp32s3-builtin.cfg -c "esp32s3.cpu0 configure -rtos none" > "$O/openocd.log" 2>&1 & OPID=$!; trap 'kill $OPID 2>/dev/null' EXIT
sleep 4
$GDB -batch -nx -ex "set pagination off" -ex "set confirm off" -ex "target extended-remote :3333" -ex "monitor reset halt" -ex "flushregs" \
     -ex "hbreak *0x$PC" -ex "continue" -ex "flushregs" -ex "info registers pc" -ex "delete" \
     -ex "set \$steps=$N" -ex "set \$outfile=\"$O/hw-trace.txt\"" -ex "source hw/steptrace.py" > "$O/gdb.log" 2>&1 || true
kill $OPID 2>/dev/null || true
./target/release/esp32sim --boot rom --flash-mb $FLASH_MB --flash-image $IMG --efuse-regs "$O/efuse.txt" --regs-init $D/reset-regs.txt \
     --strap 0x$STRAP --regtrace "$O/emu-trace.txt" --regtrace-from-pc 0x$PC --regtrace-max $N --console none --no-dump 2>/dev/null
python3 hw/compare.py "$O/hw-trace.txt" "$O/emu-trace.txt"
