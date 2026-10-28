#!/usr/bin/env bash
# Public inputs only; isolated install. No global board package changes.
# Usage: bash recipe.sh A2|A30 ABSOLUTE_OUTPUT_DIRECTORY
set -euo pipefail
profile=$1
out=$2
case "$out" in /*) ;; *) printf 'output must be absolute\n' >&2; exit 2;; esac
case "$profile" in A2) version=2.0.17;; A30) version=3.0.7;; *) exit 2;; esac
here=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$out/arduino-$profile"
python3 - "$out" "$profile" <<'PY'
import json, sys
from pathlib import Path
base = Path(sys.argv[1]) / ('arduino-' + sys.argv[2])
(base / 'config.yaml').write_text(json.dumps({
 'board_manager': {'additional_urls': ['https://espressif.github.io/arduino-esp32/package_esp32_index.json']},
 'directories': {k: str(base / v) for k, v in [('data','data'),('downloads','downloads'),('user','user')]}}))
PY
cli=(arduino-cli --config-file "$out/arduino-$profile/config.yaml")
nice -n 10 "${cli[@]}" core update-index
nice -n 10 "${cli[@]}" core install "esp32:esp32@$version"
nice -n 10 "${cli[@]}" compile -j4 --fqbn esp32:esp32:esp32cam \
  --build-path "$out/build-$profile" --output-dir "$out/fw-$profile" "$here/p0_probe"
# No sdkconfig overrides, source patches, startup/allocator intercepts or unicore substitutions.
