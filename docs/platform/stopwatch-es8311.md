# StopWatch ES8311 探测与初始化修复

2026-10-28，StopWatch 板级模型补齐内置 ES8311 的 I²C 地址 `0x18`。
此前该地址没有挂载设备，读写会收到 NACK；自定义固件在开机自检中把
ES8311 标为异常，与这一缺失一致。未取得用户的产测 v0.4 二进制或源码，
因此没有宣称该固件的自检已经复测通过。

采用与 Cardputer-Adv 相同的控制寄存器子集：`0xFD/0xFE/0xFF` 初始读数为
`83/11/00`，配置写入可读回。设备加入现有 SDA47/SCL48 内部总线，GPIO
软件 I²C 与两个硬件控制器访问同一份状态；不会响应相邻地址 `0x19`。
本次未改变通用总线、CPU、执行时序或每 tick 的处理流程。

这仍不是完整 ES8311 模型：其余寄存器采用现有 `Reg8Device` 存储，未实现
完整上电默认值、软件复位、只读约束、音频供电、时钟/滤波、PCM 播放或
麦克风采集。初始化成功不能作为听到声音、采集录音或完成语音对话的证据。

依据为 [M5Stack StopWatch 引脚表](https://docs.m5stack.com/en/core/StopWatch)
和 [ES8311 rev10.0（2021-01）第 29 页](https://files.waveshare.com/wiki/common/ES8311.DS.pdf)。
前者明确 SDA47、SCL48、地址 `0x18`，后者给出芯片标识。

验证结果：

- 新增芯片探测、标识、跨控制器配置读回和板级重建测试：修复前因缺少设备失败，
  修复后通过。已有 GPIO 探测测试增加 ES8311 ACK、相邻地址 NACK 和跨路径读回。
- StopWatch 的 8 个集成测试和 8 个板级单元测试全部通过。
- 同一官方小智 v2.2.6 镜像在修复前报告 `ES8311: Open fail`；修复后的 Node
  WASM 与独立无头 Chromium 均报告 `Es8311AudioCodec initialized` 和
  `Open codec device OK`，未再出现 `I2C_If: Fail`。
- 浏览器从 ROM 启动，以 WASM 解释器运行至 25.46 客体秒，真实 468×466
  framebuffer 显示配网模式；`appDirect=false`、`stubs=[]`、无运行错误。
- 官方 User Demo V0.5 在实际 WASM 中仍启动到 A+B 操作引导页。
- 两项 Clippy（warnings denied）、WASM 构建和四组平台检查通过。

输入 hash、命令、构建标识及结果摘要见
[回归收据](evidence/stopwatch-es8311/verification.json)。原始日志、画面和完整运行
收据保留在忽略目录 `output/stopwatch-es8311/` 与
`output/playwright/stopwatch-es8311/`，没有将第三方固件加入仓库。

复现：

```sh
cargo test --release -p esp32s3 --test stopwatch
cargo test --release -p esp32s3 --lib board::stopwatch
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy --release --target wasm32-unknown-unknown -p esp32sim-wasm --features jit-tests -- -D warnings
tools/wasm-build.sh
ESP32SIM_NO_WASM_JIT=1 node tools/probe-official-stopwatch.mjs \
  https://m5burner-cdn.m5stack.com/firmware/05a2cf9f8c71e46dbd0dca65137ad0c1.bin \
  6ed1ed429c275504c1cc32a1535f8f961b16509e87b231fd1dfd15fcff1fc21a \
  output/stopwatch-es8311/after 8
node --test tools/platform-config-test.mjs tools/platform-runtime-test.mjs tools/platform-input-test.mjs tools/platform-view-test.mjs
```

浏览器按平台技能通过 `espTester` 选择 `m5stack-stopwatch-xiaozhi`，启动后检查
上述串口标记并调用 `capture()`；保持 trial 标签。不需要清除用户存储，也未做实机验证。

发布前在 `cdf96d4e` 基线上补跑两项 Clippy、完整 release 工作区测试（含 ignored、
排除 external_）、八个标准 WASM 场景、124 项平台检查、真实录制回放和
StopWatch WASM 场景，全部通过。首次完整测试的相对 ROM 路径在 classic-camera
用例中无法解析，改用绝对路径后通过，未修改测试或更新 golden。
