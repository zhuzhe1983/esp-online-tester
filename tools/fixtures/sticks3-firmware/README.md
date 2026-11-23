# StickS3 K150 diagnostic firmware

Build with ESP-IDF v5.5.1. This diagnostic exercises the K150 simulator through the ESP32-S3 ROM and the IDF drivers. It belongs in a loopback-only test server and is excluded from the product firmware catalog.

```sh
fixture_dir="$(mktemp -d)"
idf.py -C tools/fixtures/sticks3-firmware -B "$fixture_dir/build" -D SDKCONFIG="$fixture_dir/sdkconfig" reconfigure
ninja -C "$fixture_dir/build" -j4
mkdir -p web/wasm/fw/local
esptool --chip esp32s3 merge-bin -o web/wasm/fw/local/sticks3-fixture.bin \
  0 "$fixture_dir/build/bootloader/bootloader.bin" \
  0x8000 "$fixture_dir/build/partition_table/partition-table.bin" \
  0x10000 "$fixture_dir/build/sticks3_fixture.bin"
tools/wasm-build.sh
python3 -m http.server --bind 127.0.0.1 --directory web 8787
```

Provide the standard editor ROM at `web/wasm/fw/esp32s3_rev0_rom.elf` using the repository demo assets. In a second terminal, run the loopback browser verification.

```sh
playwright_wrapper="$HOME/.codex/skills/playwright/scripts/playwright_cli.sh"
export PLAYWRIGHT_CLI_SESSION="sticks3-check-$$"
trap '"$playwright_wrapper" close' EXIT
"$playwright_wrapper" open http://127.0.0.1:8787/platform/
"$playwright_wrapper" run-code --filename=tools/fixtures/sticks3-browser-check.js
```

The test uploads the local merged image, reads the guest UART and compares actual 135 × 240 frame output for idle, A, both buttons and B. It checks the 3D front and side button hits, the guest's BMI270 reading after a 1 g X-axis orientation, and restoration of the K150 scene in the player. Separate UIFlow2 and Xiaozhi results are recorded in the [K150 runtime report](../../../docs/platform/sticks3-runtime.md).
