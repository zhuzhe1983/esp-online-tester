# PaperMono LoRa 内部协议 fixture

ESP-IDF 5.5.1 原生驱动，经真实 SPI3 中断队列及 RX GDMA、GPIO21 BUSY、GPIO5 IRQ、PM1/IOE 电源与复位控制运行。验证关电无响应、SX1262 的 `SX1261` 芯片族字符串、FIFO 跨末端回绕、TX_DONE、无对端 RX 超时、显式 PEER 接收、RSSI/SNR 读回、复位及断电。Lite 必须没有 LoRa/NFC 器件。

```sh
# 已激活 ESP-IDF 5.5.1 的 shell
idf.py -C tools/fixtures/papermono-lora-firmware -B "$PWD/output/papermono-lora-build" -D SDKCONFIG="$PWD/output/papermono-lora-sdkconfig" build
tools/wasm-build.sh
node tools/probe-papermono-lora.mjs --build output/papermono-lora-build
```

探测器使用网页实际 runtime/worker/WASM，通过 `flasher_args.json` 加载；先拒绝错误频道，再传入明确的虚拟对端报文，并验证客体串口输出。二进制只生成到忽略的 `output/`，不加入固件库或 Pages。此 fixture 不证明真实射频性能；固定官方镜像的画面验收见 `docs/platform/evidence/papermono-lora/`。
