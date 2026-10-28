# IDF sd_card (SDMMC) for the ESP32-P4

The ESP-IDF v5.5.3 `storage/sd_card/sdmmc` example (public domain or CC0, as its sources say) for
the ESP32-P4 silicon rev < 3 (v1.x), unchanged except for `sdkconfig.defaults`, which here holds
the target and revision lines in place of the example's `CONFIG_FATFS_VFS_FSTAT_BLKSIZE=4096`.
Built with `idf.py set-target esp32p4 build`. Its P4 defaults are the reference board's: slot 1
of the SDMMC host routed through the GPIO matrix to CLK 43, CMD 44, D0-D3 39-42, four lines,
default speed (20 MHz), the card powered by on-chip LDO channel 4, both cores.

The bootloader and app are `web/wasm/fw/public/p4-sdcard-bootloader.bin` and `p4-sdcard.bin`,
pinned and fetched by `tools/fetch-p4-firmware.sh`; the partition table is hello_world's
(`p4-hello-ptable.bin`, the same bytes).
They are the input of the `sdcard-p4` golden, which runs them on a FAT32 card image the test
builds (`tests/sd_image.rs`). `sdkconfig.defaults.rev3` is the same without the revision lines,
for rev ≥ 3 (IDF v5.5.4, `idf.py -D SDKCONFIG_DEFAULTS=sdkconfig.defaults.rev3 set-target esp32p4
build`): `p4-sdcard-rev3-bootloader.bin` and `p4-sdcard-rev3.bin`, the input of `sdcard-p4-rev3`:

    ../../target/release/esp32sim-p4 --flash-mb 2 \
      --bootloader build/bootloader/bootloader.bin --ptable build/partition_table/partition-table.bin \
      --app build/sd_card.bin --sd-rw card.img --max-seconds 1

The example mounts the card, prints its CID name, type, speed and size, writes `hello.txt`,
renames it to `foo.txt`, reads it back, writes and reads `nihao.txt` and unmounts. Formatting on
a failed mount is off, so an unformatted image ends in `Failed to mount filesystem`. See
../../docs/esp32p4.md.
