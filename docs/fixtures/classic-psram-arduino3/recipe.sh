#!/usr/bin/env bash
# Isolated stock Arduino 3.0.7, dual core, BOOT_INIT=n. No SDK overrides.
# Usage: bash recipe.sh ABSOLUTE_OUTPUT
set -euo pipefail
out=$1
case "$out" in /*) ;; *) exit 2;; esac
here=$(cd "$(dirname "$0")" && pwd)
bash "$here/../classic-psram/recipe.sh" A30 "$out"
nice -n 10 arduino-cli --config-file "$out/arduino-A30/config.yaml" compile -j4 \
  --fqbn esp32:esp32:esp32cam --build-path "$out/build-p6" \
  --output-dir "$out/fw-p6" "$here/p6_probe"
python3 "$here/provenance.py" "$out" "$out/provenance.json"
