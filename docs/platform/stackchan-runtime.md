# StackChan K151 硬件模拟

StackChan 使用独立板型 `stackchan`，保留 K151/K151-R 的 16 MiB Flash、8 MiB PSRAM 身份。K151-R 套装中的遥控器不属于本板型。生产硬件版次仍未知；不能将当前驱动源码视为所有批次的实机证明。

## 电气路径

| 部件 | 真实客体路径 | 可观察结果 |
| --- | --- | --- |
| 显示屏 | SPI：CS3、CLK36、MOSI37、D/C35；AW9523 P1_1 复位、AXP2101 DLDO1 背光 | 320×240 固件 framebuffer |
| 屏幕触摸 | SDA12/SCL11，FT6336U `0x38` | 固件读取接触数和坐标；与顶部触摸分开 |
| 顶部三块触摸板 | 同一 I²C 总线，Si12T `0x68` 的输出寄存器 `0x10` | 前/中/后分别为位 5:4、3:2、1:0 |
| 十二颗 RGB | PY32 `0x6f`，RGB565 小端 RAM `0x30 + 2×index`，`0x24` 位 6 刷新，经 IO14 输出 | `observe().leds`；只写 RAM 不刷新不改变灯光 |
| 双轴反馈舵机 | UART1，TX6/RX7，1 Mbaud，SCSCL 数据包；PY32 IO1 控制 VM_EN | 固件命令和反馈位置；`observe().servos`、`servoSupply` |

PY32 的默认地址是 `0x6f`，官方还列出由地址选择引脚决定的 `0x71`。默认板型只装配默认地址。LED 不接 ESP32 的 RMT 引脚；采用其他产品的 RMT 灯带路径会绕过实际控制器。

VM_EN 的资料存在冲突：厂家 Power/Adapter 原理图显示 Q4/Q3 切断舵机唯一的 VM 电源，三线接口只有 SIG、VM、GND；小智上游 StackChan README 则称关闭 VM_EN 后仍可读位置、仅停止运动。当前模拟采用不考虑寄生供电的理想电路，断开 VM 后不再答复。UART 转换器仍有独立 3.3 V 供电，但是否能沿信号线反向供电、使实机舵机逻辑继续工作，尚无实测依据。诊断中的断电无答复只验证此模型约定，不是“所有实机必定无答复”的结论；后续实测应保留并更新这个冲突。

SCSCL 数据字为大端，校验和覆盖 ID、长度、指令和参数。BSP 默认水平舵机 ID=1、零位 raw=460；俯仰舵机 ID=2、零位 raw=620；一步对应 0.3125°。固件可在 NVS 中保存不同零位；API 中默认参考角不能代替实机校准。位置随客体时间推进。无负载的理想位置模型不能预测转矩、堵转、回差、碰撞或实际转速。PWM 连续旋转模式只保留固件设置的模式和输出值，尚未模拟其连续旋转位移；没有经过校准的 PWM 到转速映射，不能把模式设置成功当作旋转功能通过。

## 操作与验收

顶部控制 ID 为 `touch-pad-front`、`touch-pad-middle`、`touch-pad-back`。它们使用普通按下/释放事件并写入 Si12T 接触状态，不占用或伪造 GPIO。可组合按下；录制保存同一语义事件。屏幕仍使用 `input({type:'touch',x,y,down})`。舵机观察值由客体串口协议产生，界面不提供覆盖客体反馈的角度动画。

```js
await espTester.input({type:'button',id:'touch-pad-front',pressed:true});
const state = await espTester.observe();
// input 的确认只表示送达；检查新的固件串口、像素、LED 和舵机反馈。
await espTester.input({type:'button',id:'touch-pad-front',pressed:false});
```

内部源代码诊断及复现步骤见 [examples/platform-stackchan](../../examples/platform-stackchan/README.md)。它检查错误 TX 路由、VM_EN 断电/恢复、舵机写入与读回，并以固件实际读取的顶部触摸驱动 RGB、舵机和画面。

本次[真实 WASM worker 回执](evidence/m5stack-k151/runtime/fixture-wasm.json)六组检查通过：ROM 启动与供电/错误引脚门控；三块触摸分别使客体改变十二颗 RGB、舵机反馈和 framebuffer；组合按压独立释放；屏幕触摸与顶部触摸分离；平台重启释放输入并重新产生输出；录制回放由客体再次消费。诊断 app SHA-256 为 `cdcf8226d986f2c05408e1cdff2830a47b30c43e61974188455c79a75aa53c79`；WASM 为 `7710be02c1e470413fcd54260c59c8ad62a26c5b3dfeb49d4bc72d867697f8b6`。回执保留初始帧 hash、客体检查结果和事件时序；回放不宣称逐像素位级一致。

## 官方固件及来源

默认固件仍为官方出厂 V1.5.1：SHA-256 `411578a2ebca2cfe3541fdc32aeddbc4703cf912a5daa7e1253f69306d6e1d87`，12,783,789 字节。应用描述符为 `stack-chan` / `1.5.1` / ESP-IDF `v5.5.4`，编译日期为 2026-07-31。镜像仅从厂商远程地址读取，不镜像到本站或 Git。当前官方仓库源码晚于此版本，不声称两者逐字节对应。

[原生执行回执](evidence/m5stack-k151/runtime/factory-native.json)记录官方镜像经过 ROM 启动显示 Welcome，触摸 Skip 后进入 AI.AGENT 启动菜单；横向选择和触摸能打开 AVATAR、DANCE。AVATAR 随后进入联网配置，DANCE 停留在启动 BLE 服务，均未验收远端交互。回执保留命令、客体时间、双核指令数、显示写入量、画面 hash 和未通过范围；截图及原始日志留在本地，不将生成的 MAC、SSID 等带入 Git。此结果与源代码诊断互为补充，官方目录仍为 `trial`，不标记整机功能兼容。

浏览器还使用相同的官方镜像完成 Welcome → 点击 Skip → AI.AGENT 菜单，确认真实 320×240 framebuffer。在诊断固件下，工作台键盘 `Digit1` 驱动红灯和水平 +40°，点击模型顶部中区驱动绿灯和俯仰 +60°；独立 mini 分享恢复后，后区驱动蓝灯和水平 −40°。iframe 通过来源及会话绑定的 RPC 触发同一中区输入，客体产生相同绿灯与 +60°反馈。主动阻断 CAD 元数据后，页面回退到 2D，面板触摸板输入仍由客体消费。紧凑回执见 [browser.json](evidence/m5stack-k151/runtime/browser.json)，原始画面保留在忽略目录 `output/playwright/`。

UIFlow2 的官方 M5Burner 项目 `1897200000000002716` 精确映射 `STACKCHAN`，当前目录固定 v2.5.5 / 版本 `2116097822314586114`。本次匿名下载返回 HTTP 400，未获取该版本镜像，不据元数据宣称启动成功。它保留 `trial` 状态及原有 watchlist。

官方文档明确出厂 AI Agent 使用小智，并区分 StackChan World 与其他小智固件的绑定方式。小智上游 `78/xiaozhi-esp32` 的固定提交 `8dc6d4588368a2091bf4f577e8b03a491c28fd71` 已有 `main/boards/m5stack/stackchan`，README 明确对应 K151/K151-R，配置为 ESP32-S3 Quad PSRAM。本轮核对的 v2.5.2 发布清单没有 StackChan 镜像，因此增加精确型号的源码参考和 `xiaozhi-stackchan-k151` watchlist，保留 `source:null`，待有唯一匹配的可启动镜像再建立下载规则。不拿 CoreS3 镜像或其他社区硬件移植冒充，出厂固件始终优先。

逐来源 URL、最终 URL、UTC 抓取时间、媒体类型、字节数和 hash 见 [2026-11-01 来源回执](research/stackchan-runtime-sources-2026-11-01.json)。知识库保留旧资料身份并追加本轮依据。官方顶部照片只说明外壳区域，原理图只说明灯的电气链序；三块隐藏电极的边界、十二颗灯的三维顺序以及机械零位均不能据此宣称实机校准。摄像头、完整音频、NFC、IR、无线服务、供电模拟与机械负载均需独立验收；本轮未实机验证。

## 回归检查

2026-11-01 在 macOS arm64、Rust stable 1.99.1、Node 26.8.2、Python 3.14.7 上完成。`ROM_DIR` 须为绝对路径；诊断程序使用 ESP-IDF 5.4.3。以下是正确性检查，未据并行构建时的耗时作性能结论。

| 检查 | 结果 |
| --- | --- |
| `cargo build --release --workspace` | 通过 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 通过 |
| `cargo clippy --release --target wasm32-unknown-unknown -p esp32sim-wasm --features jit-tests -- -D warnings` | 通过 |
| `ESP32SIM_ROM_DIR=ROM_DIR cargo test --release --workspace -- --include-ignored --skip external_` | 1,738 项通过；未改 golden |
| `ESP32SIM_STACKCHAN_FIXTURE=MERGED_BIN ESP32SIM_ROM_DIR=ROM_DIR cargo test --release -p esp32s3 --test stackchan -- --include-ignored` | 真实诊断固件集成测试通过 |
| `ESP32SIM_VQ_NATIVE=1 cargo test --release -p esp32s3 --test machine --test virtual_stops` | 通过 |
| `tools/wasm-build.sh`、`node tools/check-wasm-sections.mjs web/wasm/esp32sim.wasm` | 通过；生产 WASM hash 见上 |
| `ESP32SIM_ROM_DIR=ROM_DIR node --test tools/platform-*-test.mjs` | 199 项通过 |
| `node tools/probe-stackchan.mjs --build BUILD_DIR` | 真实 worker 的六组检查通过 |
| `ESP32SIM_ROM_DIR=ROM_DIR node tools/platform-recording-test.mjs --real` | 通过；18 个输入事件完整回放 |
| `node tools/wasm-test.mjs`，参数使用 CI 的 13 个 demo，并另跑 `demo-stopwatch` | 全部通过 |
| `tools/wasm-jit-test.sh` | 111,126 个差分用例、24 个 GPIO 波形用例通过 |
| CI 的 network-helper / network-relay、timing-api / ble-api / ble-connection、worker-pacing / worker-channel 检查 | 全部通过 |
| CI 的 touch-input / stopwatch-ui / wasm-build-policy / browser-benchmark 检查 | 28 项 Node、15 项 Python 测试通过 |
| CI 的三组保留 x5 回执检查及验证器 | 全部通过 |
| 生产 flags 下构建 `jit-profile,cpu-profile` | 通过 |
| `python tools/validate-platform-catalog.py` | 213 个产品、10 个配件、166 个固件、112 个 watchlist；通过 |
| `node tools/check-evidence-privacy.mjs`、人工检查新增来源与回执 | 通过；原始日志、截图、第三方固件不入 Git |
| 无界面 Chromium：工作台、mini、iframe、2D/3D、模型失败回退 | 通过；任务浏览器和本地服务器已关闭 |

完整平台测试初次发现知识库 UTC 时间格式不符合既有约定，修正为秒精度 UTC 后 199 项全部通过。没有改变测试条件或跳过失败项。
