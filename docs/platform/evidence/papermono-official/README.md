# PaperMono UserDemo 1.2.0 运行证据

官方二进制来自 M5Burner CDN，2,868,208 bytes，SHA-256
`72c290bc2ffa216041b276660277369bd17ecef92315e367d98bb2c96c8428fa`。
[source.json](source.json) 固定发布来源及 CORS 检查；没有保存或镜像固件二进制。

最终 WASM SHA-256 为 `de37cf4ee07c35f64efc44a80d61a7ffa3c967b2370b1cbb3123e031f4698ff4`。
[interaction.json](interaction.json) 包含实际输入回执、guest 时间、帧号、全帧与亮度/静音区域 hash，及相关启动串口。
最终运行生成 9 帧，六个检查点的局部像素断言通过。单次验收耗时 30.816 s host，到达约 21.97 s guest；该数据不构成性能比较或实机刷新速度证明。

| 检查点 | 真实固件输出 |
| --- | --- |
| [启动](boot.png) | 官方仪表盘，BL30%，TF INIT FAIL、默认 IMU 值和无效 RTC 日期如实保留 |
| [B](button-b.png) | BL20% |
| [A](button-a.png) | BL30%，标签 hash 与启动一致 |
| [触摸 +](touch-plus.png) | BL40% |
| [触摸静音](touch-mute.png) | MUTE 图标改变 |
| [再次触摸](touch-unmute.png) | MUTE 图标区域恢复原 hash |

图片是 WASM framebuffer 的 PNG 编码，没有添加 UI 或修饰。截图来自数字 EPD 寄存器模型，四灰阶端点不代表真实墨水屏的光学精度。

[negative-results.json](negative-results.json) 保留以下失败及原始 receipt hash：未实现软件 I2C 导致板型误识别；仅接 I2C0 而官方实际使用 I2C1；首次绘制的 10 s 主机回执窗口不足；第一次输入测试裁错了亮度标签区域。最后一项是测试错误，保留当时真实帧 hash；更正裁切后重新执行了完整输入用例，没有删除断言。

[sources.json](sources.json) 记录官方驱动、原理图、面板手册及 I2C/触摸协议出处、实际读取时间和 hash。NXP PDF 的 web 读取与 curl TLS 失败分别记录，未虚构失败请求的 hash。[checks.json](checks.json) 固定源文件 hash、Rust 1.99.1、两项 Clippy、1,336 个 CI 模式 Rust 测试、8 项 WASM demo 和平台运行/存储/本地多文件/worker 回归结果。golden 输出未更新。

完整范围与复现命令见 [PaperMono 运行说明](../../papermono-runtime.md)。未做实机验证；RTC 走时、持续 IMU 采样、TF、NFC、LoRa、音频、联网和睡眠唤醒仍不属于通过范围。Pro SKU 在此模型下由官方程序识别为 Lite。

隐私处理：不保留主机账户、绝对本地路径或完整串口。保留原始 receipt SHA-256、输入/帧/执行数值和相关串口摘录；省略设备存储作用域字段不影响上述图像与输入结论。
