# PaperMono 主线整合验证

运行代码 `286640ae` 同时包含 NFC/LoRa 和 RGB/Angle/Hub。本次在主线重新生成 WASM，并用同一个官方 User Demo 1.2.0 镜像验证完整版、Lite 和配件路由。

- 完整版官方程序识别 Pro，实际网页显示 LoRa 与 NFC 区域。LoRa 发出 `Hello!`；没有虚拟对端时不会自动收包，显式注入匹配的 `PEER` 后屏幕显示 `Rx PEER`。
- NFC 无卡扫描结束显示 `NO CARD`；没有实现真实射频或 NFC 卡片交互。
- Lite 仍识别为 Lite，不接入这两只器件；原 Lite Flash/TF 存储命名继续沿用，产品之间保持隔离。
- Hub 真实 WASM 的 10 组供电、拔插、Button/RGB 同时工作、Angle ADC 与录制回放检查通过。

[整合收据](receipt.json)记录固定镜像 SHA、最终 WASM SHA、帧区域 hash 与检查结果；[本地工作台截图](workbench.png)来自隔离的无头浏览器，经目视确认。测试关闭了自身浏览器，保留用户页面及其存储。设备运行中的旧 Worker 不会自动替换；刷新页面后重新启动即可使用新的 WASM，无需换固件或清空存储。

复现官方探针时使用仓库 `tools/probe-official-papermono.mjs`，参数为收据中的 URL、SHA、输出目录、`15` 和场景 `lora` / `nfc`；Lite 场景为 `lite m5stack-c153-lite`。固件只在内存读取，未加入仓库或 Pages。Hub 使用 `tools/probe-unit-hub.mjs --build <按源码生成的 fixture build 目录>`；fixture 不发布。完整命令与功能边界见 [LoRa 记录](../papermono-lora/README.md)。

本次未做实机验证，不把数字通信子集视为完整 RF、Wi-Fi、音频或睡眠唤醒兼容。

## CPU 对照

相同 Rust/Cargo 源码的 `9d072179` 基准可执行文件对应合并前主线 `20e75f72`。5 对交替运行，每次 30 客体秒，使用 `/usr/bin/time -p` 的 user CPU。各工作负载的指令数、cycle、异常/中断数及 stdout hash 均一致。

| 工作负载 | 基准中位 CPU 秒 | 合并后 | 变化 |
| --- | ---: | ---: | ---: |
| hello | 0.26 | 0.26 | +0.00% |
| c3-hello | 2.48 | 2.49 | +0.40% |
| c6-hello | 1.65 | 1.65 | +0.00% |
| pocket-tank | 42.57 | 42.14 | -1.01% |

[完整样本](native-regression.json)保留匿名负载采样；所有工作负载的取值范围重叠。本轮暂停了任务自有的编译与 guest 测试，其他主机负载未控制；这些小幅变化不证明加速或稳定退化。首次因缺少 Pocket 模型输入而中断的 [前三组样本](native-regression-incomplete.json)也保留，未用新一轮掩盖旧测量。基准与候选可执行文件 SHA 见整合收据。
