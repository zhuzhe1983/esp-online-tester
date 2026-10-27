# Dial K130 内部协议 fixture

ESP-IDF 5.5.1 源码测试：原版 K130 引脚，GC9A01 像素、FT3267 坐标/IRQ、GPIO42 按压和 A41/B40 编码器中断。不是出厂固件，不加入公开固件目录。

```sh
# 已激活 ESP-IDF 5.5.1 的 shell
idf.py -C tools/fixtures/dial-firmware -B "$PWD/output/dial-build" -D SDKCONFIG="$PWD/output/dial-sdkconfig" build
tools/wasm-build.sh
node tools/probe-dial.mjs --build output/dial-build
```

Probe 使用实际 `web/wasm/worker.js`，经本地 `flasher_args.json` 加载，再执行内部内存 fetch manifest 的录制/回放测试。`dial-fixture.invalid` 是测试传输，不是可用的公开固件地址。二进制只存在忽略的 `output/`，不发布到 Pages；源码与输入 hash 保留用于复现。
