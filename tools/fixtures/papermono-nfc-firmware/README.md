# PaperMono NFC 内部协议 fixture

ESP-IDF 5.5.1 源码测试，经实际 I2C0 SDA47/SCL48、GPIO6 与 SPI3 中断队列运行：只读身份、振荡器就绪中断、读清、屏蔽/取消屏蔽、REQA 发送完成、无应答超时、无伪造接收数据、Stop 取消超时。Lite 必须在 0x50 返回 NACK，不能复用完整版芯片。

```sh
# 已激活 ESP-IDF 5.5.1 的 shell
idf.py -C tools/fixtures/papermono-nfc-firmware -B "$PWD/output/papermono-nfc-build" -D SDKCONFIG="$PWD/output/papermono-nfc-sdkconfig" build
tools/wasm-build.sh
node tools/probe-papermono-nfc.mjs --build output/papermono-nfc-build
```

测试使用实际网页 worker/WASM，通过 `flasher_args.json` 加载。二进制只生成到忽略的 `output/`，不加入公开固件目录或 Pages。NFC 无卡验收与官方固件截图另见 `docs/platform/evidence/papermono-nfc/`；本 fixture 不证明真实射频性能或卡内容协议兼容。
