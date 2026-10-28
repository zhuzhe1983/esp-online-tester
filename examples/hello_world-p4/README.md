# IDF hello_world for the ESP32-P4

The ESP-IDF v5.5.3 `get-started/hello_world` example for the ESP32-P4 silicon rev < 3 (v1.x),
built with `idf.py set-target esp32p4 build`: single-core FreeRTOS from `sdkconfig.defaults`, and
both cores with `idf.py -D SDKCONFIG_DEFAULTS=sdkconfig.defaults.smp build`. Both select rev < 3
(`CONFIG_ESP32P4_SELECTS_REV_LESS_V3=y`, `CONFIG_ESP32P4_REV_MIN_100=y`). The bootloader,
partition table and app are `web/wasm/fw/public/p4-hello-*` (single-core, committed) and
`p4-hello-smp*` (the two cores; same partition table; pinned by `tools/fetch-p4-firmware.sh`), the
input of the `hello-p4` goldens.

`sdkconfig.defaults.rev3` (one core) and `sdkconfig.defaults.rev3-smp` (both) leave the revision
to IDF's default, rev ≥ 3 (minimum v3.1), as a stock `idf.py set-target esp32p4 build` does, and
esp32sim-p4 runs them on its rev ≥ 3 model: `p4-hello-rev3*` and `p4-hello-rev3-smp*`, built with
ESP-IDF v5.5.4 (`idf.py -D SDKCONFIG_DEFAULTS=sdkconfig.defaults.rev3 set-target esp32p4 build`)
and pinned by `tools/fetch-p4-firmware.sh`, are the input of the `hello-p4-rev3` goldens. They
need the rev 3 ROM, `esp32p4_rev300_rom.elf`.

    ../../target/release/esp32sim-p4 --flash-mb 2 \
      --bootloader build/bootloader/bootloader.bin --ptable build/partition_table/partition-table.bin \
      --app build/hello_world.bin --elf build/hello_world.elf --max-seconds 27

Expect `Hello world!`, the ten-second countdown and `rst:0xc (SW_CPU_RESET)` after each
`esp_restart()`. See ../../docs/esp32p4.md.
