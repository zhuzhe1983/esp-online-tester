#!/bin/sh
# Fetch the stock AWTRIX 3 v0.98 images for the Ulanzi TC001 (Blueforcer/awtrix3, the web flasher's
# files, pinned by commit and SHA-256) and lay them out as the 4 MB flash image the clock carries:
# bootloader 0x1000, partitions 0x8000, boot_app0 0xe000, app 0x10000, the rest erased (0xff).
#   examples/ulanzi-tc001/fetch-awtrix.sh [out.bin]     (default: awtrix-flash.bin)
set -e
out=${1:-awtrix-flash.bin}
BASE=https://raw.githubusercontent.com/Blueforcer/awtrix3/a69571703a88c7e564113f08fe95322a35d4219b/docs/ulanzi_flasher/firmware
get() { curl -fsSL --retry 5 --retry-all-errors --retry-delay 5 "$1" -o "$2"; }
sha256_ok() {  # FILE HASH; -c without GNU-only --status, so macOS sha256sum and shasum both work
  if command -v shasum >/dev/null 2>&1; then echo "$2  $1" | shasum -a 256 -c >/dev/null 2>&1
  else echo "$2  $1" | sha256sum -c >/dev/null 2>&1; fi
}
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
head -c 4194304 /dev/zero | LC_ALL=C tr '\000' '\377' > "$tmp/flash.bin"
for entry in bootloader.bin:4096:a1108edbc6368aeb8d22c31b2a2594fd0b5b388928f1c6b0d248004d659dcd9c \
             partitions.bin:32768:148b959cbff1c38aa8e1d5c0ba9d612c54997b945e56a63f41223eef650653a1 \
             boot_app0.bin:57344:f94c5d786a7a8fab06ac5d10e33bf37711a6697636dc037559ea19cc410a17f0 \
             firmware.bin:65536:d1ae4dabafba7894c7b32cc29f3af24a2b11e039b85bd82b13e0bd01218878d5; do
  name=${entry%%:*}; rest=${entry#*:}; off=${rest%%:*}; sha=${rest#*:}
  get "$BASE/$name" "$tmp/$name"
  sha256_ok "$tmp/$name" "$sha" || { echo "$name: SHA-256 mismatch" >&2; exit 1; }
  dd if="$tmp/$name" of="$tmp/flash.bin" bs=4096 seek=$((off / 4096)) conv=notrunc 2>/dev/null
done
mv "$tmp/flash.bin" "$out"
echo "wrote $out"
