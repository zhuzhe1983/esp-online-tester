# Dial K130 板级模拟

原版 M5Stack Dial K130 使用独立 `dial` 板。当前实现屏幕、单点触摸、前部按压和旋转编码器，目录为 `partial`。K130-V11 没有借用本板，仍须单独适配。

官方 [Dial 资料](https://docs.m5stack.com/en/core/Dial)、[原理图](https://m5stack-doc.oss-cn-shenzhen.aliyuncs.com/499/Sch_M5Dial.pdf)、固定版本 [M5GFX](https://github.com/m5stack/M5GFX/blob/65c816a38386062d2c3dc47d41237a9083acbc75/src/M5GFX.cpp)、[M5Unified](https://github.com/m5stack/M5Unified/blob/368ad60807161625aa65baad37a4a3af2a7eccea/src/M5Unified.inl) 及 [M5Dial](https://github.com/m5stack/M5Dial/blob/533c9d0198e72621d55a4d35f726d3d53a7e1431/src/M5Dial.h) 是引脚依据。
每次抓取的完整 URL、重定向地址、实际 UTC 秒时间、类型、字节数、SHA-256 见 [来源收据](evidence/dial/sources.json)，知识 JSON 保留相同记录。

| 部件 | 已实现 | 边界 |
| --- | --- | --- |
| ESP32-S3FN8 | 8 MiB Flash、模拟配置不提供 PSRAM，沿用 S3 ROM/CPU | 无物理样机验收，不宣称网络或任意完整固件兼容 |
| GC9A01 | 240×240，DC4/MOSI5/SCLK6/CS7/RST8，读 ID `00 9a 01`、DCS RGB565 窗口、MADCTL、睡眠/开关/复位 | 不含 PWM 背光、gamma、模拟面板电压/色彩校准 |
| FT3267 | I2C 0x38，SDA11/SCL12，IRQ14；FT5x06 通用单点报告子集，硬件 I2C0/1 和软件 I2C 共享状态 | 复用 FT6336G 的通用寄存器报告；未知芯片/固件 ID 返回零；无多点手势或触控采样周期 |
| 前部按压 | GPIO42 低有效；3D 橙色前框、屏幕下方按钮、键盘 A 同一输入 | 触屏轻触与机械按压分别命中，机械行程/抖动不建模 |
| 旋转 | A41/B40，复用既有 `BoardModel::encoder()` 和 Machine 四相 GPIO 队列 | 16 格/圈；方向为平台约定，未校准真实轴方向；不模拟接点抖动 |
| RTC/RFID/蜂鸣器/电源 | 来源和接口仍可查 | BM8563、WS1850S、音频、电源保持/深睡均未实现 |

官方出厂来源仍默认排第一；当前只有官方源码/EasyLoader EXE 的资料入口，未定位可固定校验值的完整远程 BIN，故保留 `reference`，不把 EXE 标成固件。完整本地构建或用户提供的完整远程镜像可经相同板级模型尝试运行。本轮没有遍历或认证官方固件。

## 编码器合同

```js
const input = (await espTester.capabilities()).input;
await espTester.input({type:'encoder', id:'encoder', steps:1});
```

`steps` 是 -64…64 的非零整数，单位为一格。正数依次输出 A 下降、B 下降、A 上升、B 上升；负数顺序反转。四相每相占用 2 客体毫秒，随后留出 8 毫秒空闲；每格共占 16 毫秒，连续消息不重叠。官方 Encoder 解码器对正数一格计数 -4，反向为 +4；这不是绝对角度或累计位置的写入。API 回执表示进入队列，必须再检查客体结果。

3D 灰色握环的顺时针拖动映射为正数。该顺时针约定尚未实机校准，不作为机械方向证据。握环蓝色、前框绿色、触屏红色光晕来自当前可见命中，外壳改色不改变提示。2D 仍可用 agent 输入编码器，屏幕只发送触摸。

录制保存 `{at,type:'encoder',id,steps}`，无虚构按住/释放事件；分享与 iframe 的 `input` 走同一路径。回放按客体时间排队，仍不保证周期或帧完全一致。

## 验证与模型

[源码 fixture](../../tools/fixtures/dial-firmware/README.md) 用 ESP-IDF 5.5.1 的 SPI/I2C/GPIO 驱动执行真实客体操作。它没有应用级跳转或宿主替代执行；生成文件只在忽略的 `output/`，不进入网站或固件库。

[WASM 收据](evidence/dial/fixture.json) 验证：ROM 启动；240×240 每个 RGBA 像素左128列红、右112列绿；GPIO42 按下/释放；FT3267 坐标和 IRQ；旋转 `+1/-2/+1` 经客体 GPIO 中断计数 `-4/+4/0`；非法步数被拒绝；`+2/-1` 的真实录制和回放均被客体读到。最终帧相等只是此 fixture 的结果，不是通用回放保证。

模型只给既有厂家 CAD 的 `front-bezel` 增加 `controlId`、`grip-ring` 增加 `encoderId`，并接入真实 framebuffer。更改同时写入 `model-plan.json`，公开 `rebuild-product-model.py plan/build` 已在独立 scratch 重建。[重建收据](evidence/dial/model-rebuild.json) 的几何、纹理和源文件逐字节一致；[语义修订](evidence/dial/model-semantic-revision.json) 保留前后模型 hash。没有覆盖既有照片对照结论：官方 51×51×32.3 mm 与选定塑料件约50.82×50.82×30.32 mm 的偏差仍未解决，内部为 UNKNOWN PCB，不能称作整机 1:1 验收完成。

本板新增实现不修改 SoC bus、`device_set!`、中断路由或通用 per-tick 路径；没有新增 CPU 性能实验。未做物理硬件测试。

浏览器的 UI、悬停、材质和 mini RPC 证据见 [browser.json](evidence/dial/browser.json)。最终检查见 [checks.json](evidence/dial/checks.json)：双 Clippy、1361 个 Rust 测试、58 个 Node 测试、目录 schema、五个既有 WASM 场景及本板真实 fixture 均通过。
