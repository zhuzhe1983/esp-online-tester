#!/bin/sh
# The ESP32-P4 example images the goldens boot, and the page's P4 + C6 demo (built from
# examples/*-p4 with ESP-IDF v5.5.3, the rev >= 3 hello with v5.5.4; each example's README says
# how), pinned by SHA-256 so CI and a fresh checkout get the same bytes. The page's P4 hello
# (p4-hello-*) stays committed.
#   tools/fetch-p4-firmware.sh [DIR]      default web/wasm/fw/public
# A file already in DIR with the right hash is kept, so a cached copy costs no download; any other
# is fetched from its URL and must match its hash, or the script fails.
# Images over 1 MB, and images only the tests boot, live in mikroverk/esp32sim-firmware, pinned by
# commit so a checkout keeps fetching the bytes it was tested with; P4_FIRMWARE_BASE_URL overrides
# the base.
set -e
cd "$(dirname "$0")/.."
DIR=${1:-web/wasm/fw/public}
BASE=${P4_FIRMWARE_BASE_URL:-https://raw.githubusercontent.com/mikroverk/esp32sim-firmware/d918cb24fe2798a2479fec4ecabaf43ea1a5ed42/p4}
# file, SHA-256, URL
IMAGES="p4-hello-smp-bootloader.bin fc8698c1520e0d6469861e917d84568c94d17721faa93937896ae427e8f91717 $BASE/p4-hello-smp-bootloader.bin
p4-hello-smp_world.bin 9476bccf819114fe30ad55dde4d0917e7f5213df34ec6f159b81c4a84e053f8b $BASE/p4-hello-smp_world.bin
p4-hello-rev3-bootloader.bin 8421ccef77abbbef954571409f07aa58860dc4b1f4dd517875715307df0987d6 $BASE/p4-hello-rev3-bootloader.bin
p4-hello-rev3_world.bin 103c6c59c2814601ef4db17d7cdb0f298f50203c43d2c2f76a9ee50892db39cd $BASE/p4-hello-rev3_world.bin
p4-hello-rev3-smp-bootloader.bin bc55f5ab96558d37ac399306e84675a61c27733a15a2eeb7c2c3d2376f5fd823 $BASE/p4-hello-rev3-smp-bootloader.bin
p4-hello-rev3-smp_world.bin ec158a81669da5bf149e75ea008f96cdd7eca73c7e86b7ea13fc5dcf7be78879 $BASE/p4-hello-rev3-smp_world.bin
p4-sdcard-bootloader.bin 985366855e83f25f6b9399c79aa3eac01742e0950d3e6406e24db0a32bd76816 $BASE/p4-sdcard-bootloader.bin
p4-sdcard.bin 411c93b78fc0efb199e3576c510bf0de23f74086a87cc3a9204c67a207428749 $BASE/p4-sdcard.bin
p4-sdcard-rev3-bootloader.bin c2c05529d819afe7e6e8bea8694622a10c426c4fc2d5f9e2a68974806a9fa315 $BASE/p4-sdcard-rev3-bootloader.bin
p4-sdcard-rev3.bin 497b53119c4f84724058a87a46051e4654e7828f4124024915fc40080c7037b6 $BASE/p4-sdcard-rev3.bin
p4-sdsectors-bootloader.bin 7a2e21c03a9ae5e6ac3d4b90e981843ca2348264e70cb302085e06b189a0dd71 $BASE/p4-sdsectors-bootloader.bin
p4-sdsectors.bin e658889c0ed35e7ca4ca0ca2acecbf4a48e1e4d87057341e6f31143c8d057062 $BASE/p4-sdsectors.bin
p4-usbmsc-bootloader.bin 365a7cafa9e101bb564422724a5d46d22b8dd18ea1594457b559dbeae2f8f485 $BASE/p4-usbmsc-bootloader.bin
p4-usbmsc-ptable.bin f6992f2eded46b7b73dccaf32c0702dd50c683bf834222916c5cddf7b85dac74 $BASE/p4-usbmsc-ptable.bin
p4-usbmsc.bin f47d38b4eae7c954e0c874a2ff6cfbba3a96a93c0765927672634517e4bb4150 $BASE/p4-usbmsc.bin
p4-scintilla-lcd35-bootloader.bin 6ec32ade43d31404cdfc24548459ebc61fbd74c7f8c77690c243268541e34198 $BASE/p4-scintilla-lcd35-bootloader.bin
p4-scintilla-lcd35-ptable.bin 6f0657eb6b8007c0dbfed6f64cf7a0d59f8ee1752af898e2f66dd218846b1835 $BASE/p4-scintilla-lcd35-ptable.bin
p4-scintilla-lcd35.bin af2bfdf68f7fd13f6c9046dcc2a6933389e0a8c1218c2eecd470d3495d523547 $BASE/p4-scintilla-lcd35.bin
c6-hosted-bootloader.bin 42ae8347814c4e2ac477caede2797d798261ef253c114dffc344068783f9d643 $BASE/c6-hosted-bootloader.bin
c6-hosted-ptable.bin dbbebb5ee220599f4b12fb5c43c01c38b59b2720b5df0d14f7d1b857f6524c92 $BASE/c6-hosted-ptable.bin
c6-hosted-otadata.bin 7d2c7ac4888bfd75cd5f56e8d61f69595121183afc81556c876732fd3782c62f $BASE/c6-hosted-otadata.bin
c6-hosted-slave.bin 7192ec47329c8f3d91e30f5f1da4ba9ee89a86cf1a363f27531c316f971cf44a $BASE/c6-hosted-slave.bin
c6-hosted-rawtp-slave.bin 3b0dd12a47b69cec91e20a2264393824de0bf9238c01d690f1233ed38535bf2c $BASE/c6-hosted-rawtp-slave.bin
p4-hosted-bootloader.bin 0afa47b7d81bac1e57ba261abfb8a9f622c555da56319434ad8b0f346f7cb062 $BASE/p4-hosted-bootloader.bin
p4-hosted-ptable.bin 7f00b6c042a89b15b0cac534f82ed988caf29278ff5700b0c511eb1b5bb7c820 $BASE/p4-hosted-ptable.bin
p4-hosted-station.bin b0e99c2150ba445d07b5a5f7cbe4ccd3e1e30b39075e3c93bcf736d74271f1e1 $BASE/p4-hosted-station.bin
p4-hosted-rev3-bootloader.bin 42d99a6f7470a7324077cf29de6dd5012aa2d4fbfd0f2ee0a55ba376981b86ed $BASE/p4-hosted-rev3-bootloader.bin
p4-hosted-rev3-station.bin 0a99eb9dbcf11a9b22740b4d524e44b022e327adbe737fd82390353200a11367 $BASE/p4-hosted-rev3-station.bin
p4-hosted-rawtp-rx.bin ca8efaf3d8ea2d5f67b2dd48171a75ad883ef4ecb46c9c92f1a6c5f51a180ac6 $BASE/p4-hosted-rawtp-rx.bin
p4-hosted-rawtp-tx.bin 8c5a3bcf2fc99d982119697e35ff7e3903805589489e69221ab828cdfd1d820e $BASE/p4-hosted-rawtp-tx.bin"

sha256_ok() {  # FILE HASH; -c without GNU-only --status, so macOS shasum and sha256sum both work
  if command -v shasum >/dev/null 2>&1; then echo "$2  $1" | shasum -a 256 -c >/dev/null 2>&1
  else echo "$2  $1" | sha256sum -c >/dev/null 2>&1; fi
}

mkdir -p "$DIR"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
echo "$IMAGES" | while read -r name hash url; do
  if [ -f "$DIR/$name" ] && sha256_ok "$DIR/$name" "$hash"; then continue; fi
  curl -fsSL --retry 5 --retry-all-errors --retry-delay 5 "$url" -o "$tmp/$name"
  sha256_ok "$tmp/$name" "$hash" || { echo "$url: SHA-256 mismatch" >&2; exit 1; }
  mv "$tmp/$name" "$DIR/$name"
  echo "fetched $name"
done
echo "P4 example images in $DIR: $(echo "$IMAGES" | wc -l | tr -d ' ') pinned files present"
