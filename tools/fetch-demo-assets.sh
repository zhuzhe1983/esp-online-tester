#!/bin/sh
# Everything the page's demos need that is not committed, fetched into web/ exactly as the Pages
# workflow does (it runs this script):
#   - the classic ESP32, ESP32-S3, C3, C6 and P4 mask ROM ELFs (Apache-2.0, espressif/esp-rom-elfs, pinned by release and
#     SHA-256; tools/fetch-rom-elfs.sh) into web/wasm/fw/ — every demo boots from the ROM;
#   - the ESP32-P4 and ESP32-C6 ESP-Hosted images for the P4 + C6 demo (mikroverk/esp32sim-firmware,
#     pinned by commit and SHA-256; tools/fetch-p4-firmware.sh) into web/wasm/fw/public/;
#   - xterm.js for the Terminal tab (MIT, pinned; tools/fetch-web-vendor.sh);
#   - the Linux-on-esp32-S3 flash image (GPL-3.0, svermigo/Linux-on-esp32-S3, release 0.7, pinned by
#     commit and SHA-256) for the linux demos; 16 MB, skipped with --no-linux;
#   - the official MicroPython ESP32_GENERIC v1.29.0 image (MIT, micropython.org, pinned by SHA-256;
#     tools/fetch-micropython.sh) for the T-Display demo.
# The module itself comes from tools/wasm-build.sh.
#   tools/fetch-demo-assets.sh [--no-linux]
set -e
cd "$(dirname "$0")/.."
linux=1
for arg in "$@"; do
  case $arg in
    --no-linux) linux=0 ;;
    *) echo "usage: $0 [--no-linux]" >&2; exit 2 ;;
  esac
done
FW=web/wasm/fw
LINUX_URL=https://raw.githubusercontent.com/svermigo/Linux-on-esp32-S3/9a543fb18a1385eafe7afea973ec1021f6479679/images/linux-esp32s3-native-full.bin
LINUX_SHA=e264b4abbce7610bdc0164d0f31c341c45fae98277c701039f67eded8580961d
get() { curl -fsSL --retry 5 --retry-all-errors --retry-delay 5 "$1" -o "$2"; }
sha256_ok() {  # FILE HASH; -c without GNU-only --status, so macOS sha256sum and shasum both work
  if command -v shasum >/dev/null 2>&1; then echo "$2  $1" | shasum -a 256 -c >/dev/null 2>&1
  else echo "$2  $1" | sha256sum -c >/dev/null 2>&1; fi
}
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

tools/fetch-rom-elfs.sh "$FW"

tools/fetch-p4-firmware.sh

tools/fetch-web-vendor.sh

tools/fetch-micropython.sh "$FW"

if [ "$linux" = 1 ]; then
  if [ -f "$FW/linux-esp32s3-native-full.bin" ] && sha256_ok "$FW/linux-esp32s3-native-full.bin" "$LINUX_SHA"; then
    echo "Linux image already present"
  else
    get "$LINUX_URL" "$tmp/linux.bin"
    sha256_ok "$tmp/linux.bin" "$LINUX_SHA" || { echo "Linux image: SHA-256 mismatch" >&2; exit 1; }
    mv "$tmp/linux.bin" "$FW/linux-esp32s3-native-full.bin"
    echo "Linux image fetched"
  fi
fi
ls -la "$FW"/*_rom.elf
