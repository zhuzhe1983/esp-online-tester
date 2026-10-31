#!/usr/bin/env bash
# Source ESP-IDF v5.4.3/export.sh first; tools remain caller-owned.
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
FIXTURE="$ROOT/examples/classic-camera"
BUILD=${1:?usage: build.sh absolute-output-directory}
: "${IDF_PATH:?source ESP-IDF v5.4.3/export.sh first}"
test "$(git -C "$IDF_PATH" rev-parse HEAD)" = ea1c174c1cbb7348bd8ba0ff1eb306246938dd80
CAMERA="$FIXTURE/components/esp32-camera"
if [ ! -d "$CAMERA" ]; then
  mkdir -p "$FIXTURE/components"
  git clone https://github.com/espressif/esp32-camera.git "$CAMERA"
  git -C "$CAMERA" switch --detach b817e9b81041980608d1a3d3e0b08d86e3666382
fi
test "$(git -C "$CAMERA" rev-parse HEAD)" = b817e9b81041980608d1a3d3e0b08d86e3666382
test -z "$(git -C "$CAMERA" status --porcelain)"
mkdir -p "$BUILD"
DEFAULTS="$FIXTURE/sdkconfig.defaults"
# C10_PSRAM=ON (with C10_JPEG=ON) adds the 4 MiB external RAM profile.
if [ "${C10_PSRAM:-OFF}" = ON ]; then DEFAULTS="$DEFAULTS;$FIXTURE/sdkconfig.psram"; fi
nice -n 10 idf.py -C "$FIXTURE" -B "$BUILD" -D SDKCONFIG="$BUILD/sdkconfig" \
  -D SDKCONFIG_DEFAULTS="$DEFAULTS" -D C9_LIFECYCLE="${C9_LIFECYCLE:-OFF}" \
  -D C10_JPEG="${C10_JPEG:-OFF}" -D C10_PSRAM="${C10_PSRAM:-OFF}" build
