#!/usr/bin/env bash
set -euo pipefail
if [[ -z "${IDF_PATH:-}" || "$(python "$IDF_PATH/tools/idf.py" --version)" != 'ESP-IDF v5.5.1' ]]; then
  echo 'Activate ESP-IDF v5.5.1 export.sh before building.' >&2; exit 1
fi
project_dir=$(cd -- "$(dirname -- "$0")" && pwd)
build_dir=${1:?Usage: build.sh /absolute/private/build-directory /absolute/M5StampFly}
[[ "$build_dir" = /* ]] || { echo 'Build directory must be absolute.' >&2; exit 1; }
export STAMPFLY_SOURCE=${2:?Supply the pinned M5StampFly source checkout as second argument}
python "$project_dir/check-sources.py" "$STAMPFLY_SOURCE"
mkdir -p "$build_dir"
python "$IDF_PATH/tools/idf.py" -C "$project_dir" -B "$build_dir" -D "SDKCONFIG=$build_dir/sdkconfig" reconfigure
ninja -C "$build_dir" -j3
python -m esptool --chip esp32s3 merge_bin -o "$build_dir/stampfly-uart-demo.bin" --flash_mode dio --flash_size 8MB 0x0 "$build_dir/bootloader/bootloader.bin" 0x8000 "$build_dir/partition_table/partition-table.bin" 0x10000 "$build_dir/stampfly_uart_demo.bin"
