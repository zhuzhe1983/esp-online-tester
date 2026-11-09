#!/bin/sh
# Run the blink firmware on the emulated Seeed Studio XIAO ESP32-C6.
#   examples/xiao-esp32c6/run.sh [--max-seconds 10]
# Build the firmware first (ESP-IDF 5.x):
#   cd examples/xiao-esp32c6 && idf.py set-target esp32c6 build
# The XIAO's user LED is low active: the pin sinks the current, so a low pin is a lit LED.
set -e
cd "$(dirname "$0")/../.."
B="${XIAO_BUILD_DIR:-examples/xiao-esp32c6/build}"
[ -f "$B/xiao_blink.bin" ] || { echo "no $B/xiao_blink.bin: build the project (idf.py set-target esp32c6 build) or set XIAO_BUILD_DIR" >&2; exit 2; }
[ -x target/release/esp32sim-c6 ] || cargo build --release -p esp32sim
exec ./target/release/esp32sim-c6 --boot rom --flash-mb 4 --board xiao-esp32c6 \
    --bootloader "$B/bootloader/bootloader.bin" --ptable "$B/partition_table/partition-table.bin" \
    --app "$B/xiao_blink.bin" --elf "$B/xiao_blink.elf" "$@"
