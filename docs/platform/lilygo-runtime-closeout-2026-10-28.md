# LILYGO 本轮适配收尾

本轮五款的部分运行适配已合入本地 `main`：原版 T-Display、非触摸 T-Display-S3、原版 T-Deck、T-Embed CC1101 V1.0、T-Beam V1.2 SX1276。三个并行任务均已结束。按暂停范围，T-Watch S3 与新增指定的 T5 4.7 英寸保留队列，未开始板级实现。未推送或刷写硬件。

产品清单有 127 个 LILYGO 商品身份、21 个系列，五个精确型号具备部分板级运行模型。系列与具体产品分开，Spark 与 M5Burner 共用入口位置，固件 watchlist 仍为平铺共享列表。[各型号范围](lilygo-onboarding.md)列明所有限制。

| 型号 | 已验证输出 | 保留限制 |
| --- | --- | --- |
| 原版 T-Display | 官方 ROM、菜单、双按键、轻睡眠、深睡眠及唤醒；native/Worker 五帧一致 | 电池、射频、充电与实机 |
| T-Display-S3 | 官方 TFT 示例、GPIO i8080、LEDC、GPIO0/14；浏览器 170×320 帧与 native 一致 | 出厂/LVGL DMA 阻塞仍未解决，默认保持 trial |
| T-Deck | 官方 ROM、触摸、35键、轨迹球及中键；19 个实际输出检查点与 CAD 点击 | 音频、SD、真实 RF、电源及完整第二 MCU 执行 |
| T-Embed CC1101 | 原厂菜单、SPI 屏、旋钮、双键、八颗 RGB 输出；实际 CAD 交互 | 硬件自检仍 Mixed/Unknown；电池、NFC、RF、SD、音频、睡眠 |
| T-Beam V1.2 | 官方 ROM、GPIO38、PMU 中断、SX1276 数字收发、改频与实际 CAD 按键 | PSRAM、GNSS、OLED、真实 RF、完整电源；3D 为明确标识的局部估算 |

Rust 集成代码快照 `46c92c2b` 的 workspace/all-targets Clippy、release WASM jit-tests Clippy 均以 `-D warnings` 通过；release workspace 共 1,594 项测试、136 个 suite 通过，包含 ignored、排除 `external_`，无 golden 更新。平台、目录、知识和模型共 21 项检查通过，导入器 `--check` 无变化。所有型号均通过实际 Worker 子集；真实浏览器与分享验证分别保留在型号证据中。共享主干另有独立任务提交，Rust 数字明确绑定上述快照，不将未重跑的其他改动计入该结果。

最终复跑修正了两项验收脚本问题：T-Beam 的启动流程会执行大量 GNSS 波特率探测，主机等待超时后改为最多 300 秒等待同一个 guest 42 秒检查点，独立复跑通过；T-Deck 的滚动用例使用两段明确抬手的拖动，避免把连续触点误作第二次手势，原有输出 hash 全部保持不变。失败结果保留，未放宽像素断言或修改官方 BIN。

性能比较使用 2a8aa920 基线和集成 native 二进制，每项一个预热对、七个交错测量对；指令数、guest cycles 与控制台 hash 一致。classic hello、S3 hello、C3 hello、C6 hello、pocket-tank 的 median user CPU 分别为 0.40→0.39、0.27→0.25、2.50→2.51、1.63→1.71、3.84→3.79 秒。C6 +4.91% 保留为待低负载复测项；聚合后台 CPU 最高 823.6%，不宣称隔离性能通过或将差异归因于某项改动。

[收尾回执](evidence/lilygo-final-runtime/receipt.json)、[CPU 原始数值](evidence/lilygo-final-runtime/cpu.json)、[S3 浏览器画面](evidence/lilygo-final-runtime/s3-browser.png)。证据只保留相关来源、hash、数值、帧与限制，不保留私人进程清单。

本任务的浏览器、HTTP 服务均已关闭。合并且干净的运行工作树和性能基线工作树已移入系统废纸篓，分支引用保留；恢复记录位于本地 `.git/task-cleanup/`。其他任务的工作树、浏览器和未提交改动保持原状。
