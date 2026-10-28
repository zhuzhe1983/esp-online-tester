#!/usr/bin/env bash
# Public inputs only, stock dual-core Arduino-ESP32; isolated install.
# Usage: bash recipe.sh ABSOLUTE_OUTPUT_DIRECTORY
set -euo pipefail
out=$1
case "$out" in /*) ;; *) printf 'output must be absolute\n' >&2; exit 2;; esac
here=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$out/arduino-A2"
python3 - "$out" <<'PY'
import json, sys
from pathlib import Path
base = Path(sys.argv[1]) / 'arduino-A2'
(base / 'config.yaml').write_text(json.dumps({
 'board_manager': {'additional_urls': ['https://espressif.github.io/arduino-esp32/package_esp32_index.json']},
 'directories': {k: str(base / v) for k, v in [('data','data'),('downloads','downloads'),('user','user')]}}))
PY
cli=(arduino-cli --config-file "$out/arduino-A2/config.yaml")
nice -n 10 "${cli[@]}" core update-index
nice -n 10 "${cli[@]}" core install esp32:esp32@2.0.17
nice -n 10 "${cli[@]}" compile -j4 --fqbn esp32:esp32:esp32cam \
  --build-path "$out/build-A2" --output-dir "$out/fw-A2" "$here/p5_probe"
# Do not patch the SDK, sdkconfig, core/clock settings, or startup/allocator.
