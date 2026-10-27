# DinMeter K134 内部协议 fixture

ESP-IDF 5.5.1 源码测试：原版 K134 物理 SPI 引脚、ST7789V2 四象限像素与固定屏幕方向、GPIO42 按压、A41/B40 软件中断全四相计数，以及官方出厂固件同配置的 PCNT 半四相计数。不是出厂固件，不加入公开固件目录。

```sh
# 已激活 ESP-IDF 5.5.1 的 shell
idf.py -C tools/fixtures/dinmeter-firmware -B "$PWD/output/dinmeter-build" -D SDKCONFIG="$PWD/output/dinmeter-sdkconfig" build
tools/wasm-build.sh
node tools/probe-dinmeter.mjs --build output/dinmeter-build
```

Probe 经实际页面 worker/WASM、本地 `flasher_args.json` 和内存 manifest 验证输入、像素、录制与回放。`dinmeter-fixture.invalid` 只用于测试传输。二进制保存在忽略的 `output/`，不发布到 Pages；源码和输入 hash 用于复现。源据见 `docs/platform/dinmeter-runtime.md`。一轮四相输入的方向按 A41 先降约定，不声称已在实物上核对顺时针方向或每圈机械档数。
