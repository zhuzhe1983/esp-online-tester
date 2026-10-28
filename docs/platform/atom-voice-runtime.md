# Atom Voice C008-C：按键与 RGB，官方蓝牙固件保留已知失败

本次对象是 ESP32-PICO-D4 / 4 MB Flash 的 Atom Voice（旧称 Atom Echo，C008-C），不是 ESP32-S3 的 Atom VoiceS3R。板型 `atom-voice` 接入 G39 按键与 G27 SK6812；厂商出厂 v1.0.0 完整镜像经原始 ROM 在 native 和生产 WASM 执行，输出真实 RMT 红色 `[5,0,0]`，随后在 classic Bluetooth 初始化发生 `StoreProhibited`，`EXCVADDR=0x19c` 并重启。因此保持 `trial`，不宣称 A2DP、录音或整机兼容。

## 资料与范围

- [官方产品页](https://docs.m5stack.com/en/atom/Atom_Voice)：24 × 24 × 16.8 mm，G39 按键，G27 RGB；Grove G26/G32。
- NS4168 输出为 DATA22 / BCLK19 / LRCK33；SPM1423 PDM 为 CLK33 / DATA23。保留这些音频用途，不把它们作为 RGB 或 Grove。
- [固定出厂源码](https://github.com/m5stack/M5-ProductExampleCodes/tree/1bc38736e61a362080d1dffc4020b3a2a1dff6ae/Core/Atom/AtomEcho/Factory_BT_SPEAKER_Firmware/a2dp_sink_led) 初始化 NVS、I2S、G32、RMT 红灯和 BT。该固件没有 G39 按键处理程序；按钮到客体 GPIO 的输入验收不代表其会改变出厂应用界面。
- 出厂镜像 2,097,152 bytes，SHA-256 `fa9a4c4491a83fa5b60310ee35b14e3cef71bfeac5e1cd0883b84448f6923951`；[M5Burner CDN](https://m5burner-cdn.m5stack.com/firmware/776bd8c3e6b5b3ca14ea7ef8c0da304a.bin) 与固定仓库 `BIN/Atom_echo_a2dp_led_0x00.bin` 相同。固件自报 ESP-IDF `v3.2.3-dirty`。
- 来源 URL、UTC 抓取时间、响应字节数与 hash 见 [sources.json](evidence/atom-voice/sources.json)。二进制、原始下载和原始串口日志不入库；未刷实体硬件。

## 旧版 ESP-IDF 的 FRC2 与 classic 双核调度

新增仅 ESP32 classic 使用的 FRC2，依据 ESP-IDF v3.2.3 的 `frc_timer_reg.h`、`soc.h` 与 `esp_timer_esp32.c`：基址 `0x3ff47000`、中断源 57，APB 上数、除频 1/16/256、比较状态、W1C、32 位回绕、软件重载与中断矩阵。读计数器本身不推进时间。单测覆盖除频相位、40/80 MHz APB 路由、两核中断、禁用/保留模式及大 tick 输入。

这不是完整 FRC 控制器：FRC1、edge IRQ 和 AUTOLOAD 尚未实现；AUTOLOAD/保留除频不推进，不能当成受支持模式。LOAD 与 ALARM 相等时在下一 APB 边沿触发，是依据 `esp_timer_impl_advance` 用法的模型推断，未实机测时。

补 FRC2 后 native 可以进入 BT；生产 WASM 原来的共享默认 quantum 256 却停在双核 DPORT 握手，JIT 与解释器均重现。原始镜像的 native 控制实验把 quantum 改成 256，同样停在 `0x40081887/0x40081ca9`；保持 VQ=1024、改回 64 则进入红灯/BT，单独将 VQ=1 仍不能解除 quantum=256 的停滞。固定 IDF 的 `dport_access.c` 与 `dport_panic_highint_hdl.S` 对应请求/应答自旋路径。

因此只在 classic 构造器统一使用原有 native quantum 64，WASM 跟随。没有改 S3 或共享 scheduler，也没有跳过客体握手、改固件指令或伪造成功。此为已观测旧固件的兼容修正，不声称 cycle-accurate 时序或找到硬件上同类故障。此前实验机制索引已核查 `docs/experiments.md` 的 EX047（quantum）、EX133/EX177（VQ）；本次不是 S3 加速实验，保留负结果与 native/WASM 对照。

## 3D 与输入

保留厂家 CAD 的原始两片壳体和装配坐标，背面纸贴独立于外壳；派生 G39 可点击区域位于盖板 U 形弹片内。LED 透过既有 U 形缝隙显示，位置和深度按官方 Bluetooth 操作照片估计，未增加假孔，也不声称有完整 PCB CAD。内部仍是 `UNKNOWN` 空板，把文字移出透孔可见区域；复用既有 `labelOffsetMm` 合同。

LED 视觉校准中的人工颜色只用于检查材质/位置。固件行为证据必须来自客体 RMT，见 [wasm.json](evidence/atom-voice/wasm.json)。原始 CAD/照片尺寸对照沿用 C008-C 的可追溯资产报告；前视参考还包含独立硬币，既有 silhouette 得分存在裁剪歧义，不能宣称精确 1:1 全部通过。

## UIFlow 与小智调查

2026-10-28 再次查询官方 Burner 项目 `1897200000000002571`。它的项目说明与设备分类都是 **Atom-Lite**（device `2093000000000000008`），但历史版本 `2090702395866066946` 名为 `v2.5.1-ATOM-ECHO`，文件名含 `M5STACK_Atom_Echo` 和 `esp32-4mb-atomecho`。这是发布方元数据冲突；目前保留候选及来源，不将 Atom-Lite 的最新版本自动映射给 Voice。

已审查的官方小智目录只有 VoiceS3R/EchoS3R、AtomS3R 等项目，没有可据此精确映射的 classic C008-C 项目。未知不等于固件不存在；不把 S3R 镜像套到 classic。出厂固件仍列表第一项及默认，其他明确来源可继续通过远程/本地入口尝试。

## 复现与验证

```sh
cargo run --release -p esp32 --example atom_voice_probe -- <factory.bin> <esp32_rev300_rom.elf>
node tools/probe-atom-voice-official.mjs <official-https-url> <expected-sha256>
```

执行收据、浏览器实际按钮/mini、全量检查和 CPU 对照分别见此目录下证据。真实 GPIO 输入、RMT 输出、外观及未模拟的音频/无线边界分别报告，不把预览或输入排队当成整机测试通过。

最终检查：1,438 项 Rust / 132 组、双 Clippy、11 个 WASM 场景、100 个平台测试，以及 CI 的 JIT、时序、BLE 与历史证据检查通过。真实录制回归曾有一次回放按钮漏采样；未改 S3 或录制代码的复跑通过，保留此时序敏感失败，见 [checks.json](evidence/atom-voice/checks.json)。未更新 golden。

[CPU 五对收据](evidence/atom-voice/cpu.json)：固定 main `84e67571` 参考，S3 0.0%、C3 +0.435%、C6 +1.299%、classic +2.778%；每对 UART、指令数与周期一致。classic 中位数为 0.36→0.37 秒，差值与计时百分秒精度接近，不能归因为确定的性能回退或改进；跨修订对照也有来源范围限制。未改 S3 core，未重复 Pocket。任务浏览器与 8100 服务已关闭，未改用户 8088。
