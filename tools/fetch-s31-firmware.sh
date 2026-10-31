#!/bin/sh
# Fetch pinned ESP32-S31 example images for Machine acceptance (mirror of fetch-p4-firmware.sh).
#
# BLOCKED on hosts without ESP-IDF v6.1 `--preview set-target esp32s31`.
# This Mac worktree has IDF 5.3/5.4 only; do not claim hello_world boots until
# binaries are built, hashed and listed below. Metalio INK4 is not acceptance.
set -eu
DIR=${1:-esp32s31/firmware}
# BASE=https://…   # publish once hello_world artifacts exist
# IMAGES="s31-hello-bootloader.bin <sha256> $BASE/…
# s31-hello-ptable.bin <sha256> $BASE/…
# s31-hello_world.bin <sha256> $BASE/…"

echo "fetch-s31-firmware: no pinned IDF v6.1 esp32s31 images yet." >&2
echo "Build hello_world with: idf.py --preview set-target esp32s31" >&2
echo "Then add SHA-256 rows here (see tools/fetch-p4-firmware.sh) and re-run." >&2
echo "Destination directory would be: $DIR" >&2
exit 1
