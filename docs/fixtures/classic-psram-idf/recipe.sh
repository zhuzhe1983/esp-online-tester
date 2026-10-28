#!/usr/bin/env bash
# Usage: bash recipe.sh ABSOLUTE_NEW_OUTPUT_DIRECTORY
set -euo pipefail
out=$1
case "$out" in /*) ;; *) printf 'output must be absolute\n' >&2; exit 2;; esac
mkdir "$out"
here=$(cd "$(dirname "$0")" && pwd)
image=espressif/idf@sha256:52bc81e7f212b6cc63b31ea57b8270badb3236e44df8567a57e2d1a6c74c5000
docker run --rm --platform "${IDF_PLATFORM:-linux/arm64}" -e CMAKE_BUILD_PARALLEL_LEVEL=4 -e SOURCE_DATE_EPOCH=1609459200 \
  -v "$here:/input:ro" -v "$out:/out" -w /out "$image" bash -c '
    set -euo pipefail
    test "$(git -C "$IDF_PATH" rev-parse HEAD)" = 38eeba213aa695aabfd6d89aa9f5078dbe5a94c3
    cp -R /input /out/source
    cd /out/source
    nice -n 10 idf.py -B /out/build -DCCACHE_ENABLE=0 reconfigure
    nice -n 10 ninja -C /out/build -j 4
    mkdir /out/fw
    cp /out/build/bootloader/bootloader.bin /out/fw/bootloader.bin
    cp /out/build/partition_table/partition-table.bin /out/fw/partition-table.bin
    cp /out/build/psram.bin /out/fw/psram.bin
    python /out/source/provenance.py /out/build /out/source/sdkconfig /out/provenance.json
  '
