#!/bin/sh
# Build the ESP32-S31 hello_world images the Machine golden boots, from source, and pin them by
# SHA-256 (no published S31 artifacts exist yet, so this builds instead of downloading).
#   . $IDF_PATH/export.sh   # ESP-IDF v6.1, commit below
#   tools/fetch-s31-firmware.sh [DIR]      default esp32s31/firmware
# Source: $IDF_PATH/examples/get-started/hello_world, unmodified, plus
# tools/s31-hello/sdkconfig.defaults (reproducible build, fixed project version, no build date).
# Built in target/s31-hello-build (not committed). A file already in DIR with the right hash is kept.
set -eu
cd "$(dirname "$0")/.."
DIR=${1:-esp32s31/firmware}
IDF_COMMIT=fff9895c82d744c7237be8847347bdd1b07c6643   # v6.1
# file, SHA-256, build output
IMAGES="s31-hello-bootloader.bin d60f6fac20b132ffe97194e3f160cb575b0fc52cea744e0a14c06658973d0b68 bootloader/bootloader.bin
s31-hello-ptable.bin 7f00b6c042a89b15b0cac534f82ed988caf29278ff5700b0c511eb1b5bb7c820 partition_table/partition-table.bin
s31-hello_world.bin 6b965db02941e8db4895d27c2ad8eea1733e22ddf9a82ccd4d6f77ae62c6335c hello_world.bin"

sha256_ok() {
  if command -v shasum >/dev/null 2>&1; then echo "$2  $1" | shasum -a 256 -c >/dev/null 2>&1
  else echo "$2  $1" | sha256sum -c >/dev/null 2>&1; fi
}
all_ok=1
for n in $(echo "$IMAGES" | awk '{print $1":"$2}'); do
  sha256_ok "$DIR/${n%%:*}" "${n#*:}" || all_ok=0
done
if [ $all_ok = 1 ]; then echo "S31 hello images in $DIR: 3 pinned files present"; exit 0; fi

: "${IDF_PATH:?source ESP-IDF v6.1 export.sh first}"
got=$(git -C "$IDF_PATH" rev-parse HEAD)
[ "$got" = "$IDF_COMMIT" ] || { echo "IDF_PATH is at $got, need $IDF_COMMIT (v6.1)" >&2; exit 1; }
B=target/s31-hello-build
rm -rf "$B"; mkdir -p target
cp -R "$IDF_PATH/examples/get-started/hello_world" "$B"
rm -rf "$B/build" "$B/sdkconfig"
cp tools/s31-hello/sdkconfig.defaults "$B/sdkconfig.defaults"
(cd "$B" && idf.py --preview set-target esp32s31 >/dev/null && idf.py build >/dev/null)
mkdir -p "$DIR"
echo "$IMAGES" | while read -r name hash out; do
  sha256_ok "$B/build/$out" "$hash" || { echo "$name: SHA-256 mismatch ($(shasum -a 256 "$B/build/$out" | cut -d' ' -f1))" >&2; exit 1; }
  cp "$B/build/$out" "$DIR/$name"; echo "built $name"
done
