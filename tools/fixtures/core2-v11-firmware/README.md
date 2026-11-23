# Core2 K010-V11 接线检查固件

采用 ESP-IDF 5.5.1 与 ESP32 目标。夹具读取 AXP2101、INA3221，启用 ALDO4、ALDO2 和 BLDO1，走真实 SPI 写入三个不同颜色的像素，并在 GPIO39 有效时打印 FT6336U 原始坐标。夹具输出只用于验证 v1.1 的板级通路。

```sh
export IDF_TARGET=esp32
idf.py -C tools/fixtures/core2-v11-firmware -B "$PWD/output/core2-v11-build" -D SDKCONFIG="$PWD/output/core2-v11-build/sdkconfig" build
esptool --chip esp32 merge-bin -o output/core2-v11-build/merged.bin \
  0x1000 output/core2-v11-build/bootloader/bootloader.bin \
  0x8000 output/core2-v11-build/partition_table/partition-table.bin \
  0x10000 output/core2-v11-build/core2_v11_wiring.bin
cargo run --release -p esp32 --example core2_factory_probe -- output/core2-v11-build/merged.bin web/wasm/fw/esp32_rev300_rom.elf core2-v11
```

预期串口包含 `CORE2 V11 AXP2101=4A INA3221=3220 LCD ready`、`CORE2 V11 TOUCH 160 120` 和 `CORE2 V11 TOUCH 65 260`。
