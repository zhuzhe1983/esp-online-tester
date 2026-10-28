#!/usr/bin/env bash
# Stock Arduino-ESP32 3.1.1, isolated installation, same P6 sketch and assertions.
# Usage: bash recipe.sh ABSOLUTE_OUTPUT
set -euo pipefail
out=$1
case "$out" in /*) ;; *) exit 2;; esac
here=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$out/arduino-A31"
python3 - "$out/arduino-A31" <<'PY'
import json, sys
from pathlib import Path
base = Path(sys.argv[1])
(base / 'config.yaml').write_text(json.dumps({
 'board_manager': {'additional_urls': ['https://espressif.github.io/arduino-esp32/package_esp32_index.json']},
 'directories': {k: str(base / v) for k, v in [('data','data'),('downloads','downloads'),('user','user')]}}))
PY
cfg="$out/arduino-A31/config.yaml"
test -f "$cfg" # Missing configs silently fall back to the global installation.
nice -n 10 arduino-cli --config-file "$cfg" core update-index
nice -n 10 arduino-cli --config-file "$cfg" core install esp32:esp32@3.1.1
nice -n 10 arduino-cli --config-file "$cfg" compile -j4 \
  --fqbn esp32:esp32:esp32cam --build-path "$out/build-p6" \
  --output-dir "$out/fw-p6" "$here/../classic-psram-arduino3/p6_probe"
python3 "$here/provenance.py" "$out" "$out/provenance.json"
