#!/bin/sh
# The mask ROM ELFs the emulator boots from (Apache-2.0, espressif/esp-rom-elfs), pinned by release and
# SHA-256 so CI, the Pages build and a fresh checkout all get the same bytes. The release is fetched by its
# download URL, not through the GitHub API, so API rate limits don't apply. Files already in DIR with the
# right hash are kept, so a cached DIR costs no download.
#   tools/fetch-rom-elfs.sh DIR
# A newer release only matters if it changes one of these files: check its tarball against Espressif's
# esp-rom-elfs-<release>-checksum.sha256, then update RELEASE, TARBALL_SHA and any ELF hash that changed.
# 20260528's ELFs are byte-identical to 20241011's (the copy ESP-IDF 5.x installs), except
# esp32p4_rev300_rom.elf (the ESP32-P4 rev >= 3, ECO5), which 20241011 does not have.
set -e
[ $# -eq 1 ] || { echo "usage: $0 DIR" >&2; exit 2; }
DIR=$1
RELEASE=20260528
TARBALL_SHA=caa463d3cbef2430a5a35847c1d9f2f152403b17a802050927ff60c8da54fe46
ROMS="esp32_rev300_rom.elf 920b70635440517866aab2230964a570d2cf2b676658d93c52fbac108c1cca31
esp32s3_rev0_rom.elf c0ce0f338d1de1bdc6efbef1591779a2a42c1ab7d759d3c6ae8ae63a7dd34cfd
esp32c3_rev3_rom.elf 19ac22e08707df926fb0cf4c54795d4067b983fae8635f396ded173a6d78fc3c
esp32c6_rev0_rom.elf 788e1d38724aeb8fd974fa10c4a7b089c02627d35342ce84b9e0b12b239f3551
esp32p4_rev0_rom.elf 948f2c7d108d04a9c3a982f9c1c498fb17f2c4a68abc51f87782592034d7ce44
esp32p4_rev300_rom.elf 90e723f71fe4701adee7e40bfab6971d8ecf5e356f4d5a81e77743248abc3a97"
URL=https://github.com/espressif/esp-rom-elfs/releases/download/$RELEASE/esp-rom-elfs-$RELEASE.tar.gz

sha256_ok() {  # FILE HASH; -c without GNU-only --status, so macOS shasum and sha256sum both work
  if command -v shasum >/dev/null 2>&1; then echo "$2  $1" | shasum -a 256 -c >/dev/null 2>&1
  else echo "$2  $1" | sha256sum -c >/dev/null 2>&1; fi
}
have_all() {
  echo "$ROMS" | while read -r name hash; do sha256_ok "$DIR/$name" "$hash" || exit 1; done
}

mkdir -p "$DIR"
if have_all; then echo "mask ROMs already in $DIR (esp-rom-elfs $RELEASE)"; exit 0; fi

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
curl -fsSL --retry 5 --retry-all-errors --retry-delay 5 "$URL" -o "$tmp/rom.tar.gz"
sha256_ok "$tmp/rom.tar.gz" "$TARBALL_SHA" || { echo "$URL: SHA-256 mismatch" >&2; exit 1; }
mkdir "$tmp/x"
tar -xzf "$tmp/rom.tar.gz" -C "$tmp/x"
echo "$ROMS" | while read -r name hash; do
  f=$(find "$tmp/x" -name "$name" | head -n 1)
  [ -n "$f" ] || { echo "$name is not in esp-rom-elfs $RELEASE" >&2; exit 1; }
  sha256_ok "$f" "$hash" || { echo "$name: SHA-256 mismatch" >&2; exit 1; }
  cp "$f" "$DIR/$name"
done
echo "mask ROMs from esp-rom-elfs $RELEASE into $DIR"
