# Unit Button U027 internal fixture

This ESP-IDF **5.5.1** fixture reads the real guest GPIO peripheral on the **StopWatch** board model. It is not catalog firmware and its binaries are never committed or deployed. U027 V1.0's white signal is StopWatch Port A GPIO11; yellow GPIO10 is NC at the unit. GPIO11 starts with a guest pull-down, GPIO10 with a guest pull-up.

Build from the repository root after activating ESP-IDF 5.5.1:

```sh
idf.py -C tools/fixtures/unit-button-firmware -B "$PWD/output/unit-button-build" \
  -D SDKCONFIG="$PWD/output/unit-button-build/sdkconfig" build
# Build the emulator, and make its S3 ROM available locally as documented.
tools/wasm-build.sh
node tools/probe-unit-button.mjs --build output/unit-button-build
```

The probe creates a test-only in-memory fetch origin for the actual local build. It boots ROM → bootloader → app with no guest patches or stubs, drives the public runtime, and asserts fresh `gpio_get_level()` output after each transition. It also resets and replays four recorded accessory events. Output is a JSON receipt containing input hashes and observed GPIO results. See [runtime documentation](../../../docs/platform/unit-button-runtime.md).

For UI checks, choose StopWatch, attach Unit Button, select local firmware, and select `flasher_args.json`, `bootloader/bootloader.bin`, `partition_table/partition-table.bin`, and `unit_button_fixture.bin` together. The fixture has no display output; the black screen is expected. Press the 3D cap or the accessory panel button and inspect UART0 for `U027_READ G11=0 G10=1` then `G11=1`.

Guest UART0 commands configure real guest peripherals:

| Character | Action |
| --- | --- |
| `o` / `i` | M5PM1 Grove 5V off / on |
| `u` / `d` | GPIO11 pull-up / pull-down |
| `h` / `n` | Set / clear M5PM1 HOLD_5V |
| `s` | M5PM1 shutdown command |
| `r` | `esp_restart()`; the external switch remains physically held |

The test does not claim RC timing, bounce, analog contention, physical hardware calibration or arbitrary official firmware compatibility. The initial failed probe used a stale serial match after pause; assertions were corrected to consume only new serial bytes. The passing receipt does not reclassify that harness failure as a firmware failure.
