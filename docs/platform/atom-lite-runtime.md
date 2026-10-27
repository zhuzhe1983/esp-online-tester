# Atom-Lite C008 运行适配

原版 C008 已接入经典 ESP32 板型 `atom-lite`，官方 Factory Test v1.0 可以从 ROM 启动。3D 按键、键盘 A 与嵌入页均通过真实客体输入验证：G39 拉低后，固件从等待状态进入 Grove 检测，G27 RMT 输出红色 `[20,0,0]`。目录保持 `partial`，固件保持可确认试运行；这不代表全部出厂测试通过。

## 来源与接线

[官网](https://docs.m5stack.com/en/core/Atom-Lite)与[原理图](https://static-cdn.m5stack.com/resource/docs/products/core/ATOM%20Lite/img-2e58eac3-d9ef-4be4-b486-d4dd9a8324fa.webp)给出 ESP32-PICO-D4、4 MB Flash、G39 用户键、G27 单颗 SK6812 3535、G12 红外和 Grove G26/G32。按键用 4.7 kΩ 外部上拉，按下接地；模型不混用 AtomS3-Lite 的 G41/G35 或 Stamp-S3 的 G0/G21。当前模拟器配置不接 PSRAM，知识库仍保留官网未明示容量的未知项。

本次实际下载的完整出厂镜像为 1,048,576 字节，SHA-256 `1c8fa292e31819c4a3ca36c714e93d08f7a8a58f16918f8fdf1df0210e546ce4`；引导、分区表和应用分别位于 `0x1000/0x8000/0x10000`，应用描述报告 `arduino-lib-builder` / `esp-idf: 1b16ef6cfc`。完整地址与抓取时间见 [sources.json](evidence/atom-lite/sources.json)。该镜像仅临时下载验收，未入库或放入站点资产。

官方 M5Atom `ebbf736396f9459bf1adc7b588a7f1825b366844` 的 `FactoryTest/ATOM_LITE` 示例采用 G27、GRB 800 kHz，循环彩虹、按住暂停；**实际分发 BIN 与该示例不同**。启动黑灯并等待按键是本次 BIN 的实际行为，不能据未出现彩虹认定 RMT 损坏，也没有为此修改 RMT 或 CPU。官网当前原理图画 CH552T，而教程 USB 驱动段介绍 FTDI；保留这项来源差异，不由驱动说明推定用户硬件版次。

## 实现与验证范围

- 复用经典 ESP32 CPU、ROM、GPIO 与 RMT。新板型只挂接 G39 外部高电平和 G27 24-bit GRB 链；错误引脚和不完整 RGB 帧不会更新灯。未添加逐 tick 彩色动画。
- 前端按经典芯片选择 ROM、`0x1000` 引导偏移并检查 app 芯片。默认官方出厂条目保留第一项；UIFlow2 仍可尝试但未在本次运行验收。
- 官方 BIN 原生运行与生产 WASM 均打印 `M5Atom initializing...OK`，按键后打印 `Testing grove gpio32:0; gpio26:0` 并输出红灯。工作台实际鼠标、重启后的键盘 A，以及分享 mini 页均复核。无屏产品不伪造 framebuffer。
- 厂家原始 STL 保留；在盖板弹片内添加命中区，在已有灯缝下放置照片估算的 LED。灯位/深度不冒充 PCB CAD；背部仍是 `UNKNOWN` 占位板。金色、紫透、正反面、悬停和 2D 均检查。现有 USB 简化件未作为精确装配验收，整机 1:1 仍为部分完成。
- 红外收发、物理 Wi-Fi/BLE、外接 Grove 测试夹具、完整出厂流程及模拟电源未验证；没有实机验证。串口 Grove 数值不是已连接外部设备的证据。

## 复现

```sh
node tools/probe-atom-lite-official.mjs \
  https://m5burner-cdn.m5stack.com/firmware/863975d5491b3e07aec72d90518afd6e.bin \
  1c8fa292e31819c4a3ca36c714e93d08f7a8a58f16918f8fdf1df0210e546ce4
cargo run --release -p esp32 --example atom_lite_probe -- <firmware.bin> web/wasm/fw/esp32_rev300_rom.elf
python tools/rebuild-product-model.py build m5stack-c008 --scratch <independent-output>
```

完整命令与退出码见 [checks.json](evidence/atom-lite/checks.json)，实际 WASM 收据见 [official-wasm.json](evidence/atom-lite/official-wasm.json)，浏览器输入、分享与材料检查见 [browser.json](evidence/atom-lite/browser.json)。全部 Rust 1434 项、132 组，双 Clippy、11 WASM 场景、100 平台测试、真实录制回放及 CI 的 JIT/时序/历史检查通过，未重建 golden。[CPU 收据](evidence/atom-lite/cpu.json)采用固定 main `84e67571` 的三组 hello、各五对交替运行：S3 −4.0%、C3 +0.858%、C6 0.0%；每组 UART、指令与周期一致。宿主计时分辨率和跨主线修订限制使这些数值不构成性能改进结论。未改 S3 核心，因此未重复 Pocket Tank。任务浏览器和预览服务均已关闭。
