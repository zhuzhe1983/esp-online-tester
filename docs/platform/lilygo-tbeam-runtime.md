# T-Beam V1.2 SX1276：已验证操作与模型边界

本条目限定原版 **T-Beam V1.2、AXP2101、SX1276 868/915 MHz**，不包括 V1.1/AXP192、SX1262、SUPREME、1W 或其他预装固件版本。未安装选配 OLED。板级支持为 `partial`，不是整机兼容声明。

固定官方出厂镜像在原生执行器和浏览器 WASM worker 中均通过普通 ROM 启动，无 app-direct、ROM stub、固件补丁或自动对端回包。实测 GPIO38 短按切换发送/接收，发送完成经 DIO0 回到固件；显式注入 `07000000` 后，固件打印 `COUNT:7`。错误信道被拒绝。AXP2101 电源键短按触发 IRQ35 并进入固件处理函数，UART0 的 `freq:915.0` 确实修改 SX1276 寄存器。浏览器中实际点击 3D 电源键使短按处理次数从 1 增至 2，实际点击用户键产生 `currentFrames : 3`。

原生回归运行 48.77 客体秒、8,052,638,319 条指令，只有一次 `setupBoards`。浏览器功能检查在 188.06 客体秒完成，3D 按键检查延续至 270.68 秒。浏览器等待与人工检查时长不构成性能测量。输入、数值和来源分别见 [运行记录](evidence/lilygo-tbeam-runtime/receipt.json) 与 [来源记录](evidence/lilygo-tbeam-runtime/sources.json)。

## 板级模型

- AXP2101：I2C `0x34`、SDA21/SCL22，两个控制器共享同一寄存器状态；只读 ID、通道配置、IRQ 使能、状态 W1C、短/长按状态及固定电源 ADC 档案。电源档案明确为 USB 5V、无电池，不能解释为真实电池电压或充电仿真。短按数字过滤下限为模型的 10 ms 近似；长按阈值来自寄存器 0x27。关机请求只记录并切断无线模块的数字供电，不模拟 ESP32 物理断电。
- SX1276：严格 SPI 引脚、CS 分段事务、复位/供电门控、FIFO、LoRa 参数、事件空中时间、IRQ/W1C、DIO0/1、发送记录和显式数字对端接收。公共对端 API 沿用已有编码，因此将 SX1276 的频率字、带宽和同步字转换为公共格式。最多保留 16 条发送记录。
- 没有 RF 传播、天线、碰撞、射频灵敏度、真实 RSSI/SNR、电流或模拟电源。FSK/OOK、CAD 和跳频未实现；不因 RadioLib 初始化成功而声称支持这些模式。
- 没有 GNSS 模型、NMEA/UBX 自动回答或真实 UART 定时注入。官方固件会探测 GNSS 超时后继续。可选 OLED 和本产品的 8 MiB PSRAM 仍缺失；固件会报告显示器未找到及 PSRAM 禁用。硬件知识中的 8 MiB 是实物容量，不是当前模拟容量。上游新增的经典 ESP32 PSRAM 仅支持可选 4 MiB；WASM 对本条目的 8 MiB 请求保持无 PSRAM，不能据此提升本产品的兼容状态。
- 长按睡眠/唤醒、物理电源断开、3D 复位键和 Wi-Fi 网络交互未验收；可使用平台运行控制重置。串口出现 SoftAP 日志不证明真实无线服务可用。

寄存器定义以厂家固定提交中的 XPowersLib、RadioLib 和 V1.2 原理图核对；镜像自身记录 ESP-IDF v4.4.7。AXP2101 和 SX1276 模型留在本板目录，没有新增共享寄存器块或 `Board` 方法。公开 PR 检查中，#311 的 ESP32 板注册和 #262 的 C6 工作与此板不重叠。WASM 改动只把已有显式 LoRa API 从 S3 专用入口转到通用板入口，不增加热路径工作，因此未做 CPU 性能声明。

## 固件与输入时机

源码仓库为 [LilyGo-LoRa-Series 固定提交](https://github.com/Xinyuan-LilyGO/LilyGo-LoRa-Series/tree/1991b1ed70915d4f246b841f99f6027166ea7330)，镜像名 `tbeam-v1.x-all-band-sx1276-factory-20260817_0x0.bin`，1004784 字节，SHA-256 `2867c0cdfe47a9ad70cb69504a7fbe757ccaf73b044fc2345a2250d93284a4b6`。完整镜像写入偏移 0，经典 ESP32 ROM 读取 0x1000 引导器；第三方 BIN 不入库。

固件启动时的 GNSS 探测会等待约 32 客体秒。`SoftAP started` 早于余下启动延时及初始同步发送完成；原生与 worker 回归在 42 客体秒开始按键。GPIO38 的 AceButton 单击会拒绝过长按压，阻塞发送期间也可能漏过短按。测试发送真实按下/释放边沿、等待固件结果，并有界地尝试短按；没有直接调用固件菜单函数。

固件进入 RX 时可能因遗留的发送回调标志先打印 `COUNT:0 / RSSI:-157 / SNR:0`。这不是模拟器注入数据：显式输入之前无线模型的 `rxCount` 为 0。验收要求显式对端输入后 `rxCount=1` 且输出 `COUNT:7`，避免把这条固件日志误认作接收通过。

## 外观：局部照片模型，未通过 1:1 验收

厂家公开了 `dimensions/T-Beam-V1X.7z`，但 STEP 内部身份为 `T22_GPS_V1-1-PCB`，因此没有作为 V1.2 精确 CAD 使用。照片来自 LILYGO 商店；其中射频标签分别为 923 MHz 和 433/470 MHz，只用于共用外形、侧键与电池座参照，不作为本条目射频/GNSS 元件布局依据。

当前模型只有标明 **UNKNOWN** 的空白 PCB、三个侧键和估算电池座。32.89 × 100.13 mm 取自旧 V1.x 尺寸图作为比例估计；1.6 mm 板厚及电池座厚度均未经实物确认。USB、SMA、GNSS 接头、安装孔、焊盘、丝印与内部器件没有伪造。裸板没有可改色外壳，也没有线缆插接锚点。

实际通用渲染器检查了正面、背面和侧面，UNKNOWN 清晰可见，两个有效按键能驱动同一运行实例。正/背截图因核验页面高度被截去底部；侧面可见。未完成同视角叠图、轮廓 IoU、孔位误差和准确厚度验证，不能据此宣称精确还原。照片 hash、来源时间、估算方法与限制保留在 `web/platform/assets/lilygo-t-beam-v1-2/model-plan.json`。可在独立目录重建并对照输出 hash：

```sh
python3 tools/build-tbeam-model.py --output target/tbeam-model-rebuild
```

## 复现与检查

先按项目文档构建 WASM、获取 ROM，下载上述固定镜像并核对 SHA-256。以下变量由调用者提供：

```sh
export TBEAM_FACTORY_BIN=/path/to/tbeam-v1.x-all-band-sx1276-factory-20260817_0x0.bin
export ESP32SIM_ROM_DIR=/path/to/roms
cargo test -p esp32 --lib tbeam
cargo test --release -p esp32 --test tbeam_factory -- --ignored --nocapture
node tools/tbeam-factory-test.mjs
cargo clippy -p esp32 --all-targets -- -D warnings
cargo clippy --release --target wasm32-unknown-unknown -p esp32sim-wasm --features jit-tests -- -D warnings
```

浏览器通过平台公开 API 配置本产品、加载默认官方源并启动；对应固定 SHA 下载成功。等待启动后短按用户键进入 TX、再进入 RX，调用 `receiveLoRa` 注入测试包，短按电源键，最后通过 `input({type:'serial',port:'uart0',data:'freq:915.0\n'})` 改频。自动化操作之后实际点击 3D 两个按键，检查新增 UART 输出。创建的无头浏览器会话已关闭、服务器已停止，未触及其他任务会话。

定向 Rust 单元测试 8 项、官方原生固件回归、worker 回归、板级与 WASM Clippy 均通过；平台配置、输入、运行、视图、LoRa、知识及 LILYGO 目录检查通过。全工作区 CI 由集成步骤统一运行。没有更改 golden 输出，也未刷写实机。原始日志与截图仅在本地 scratch 保留，提交记录保留 hash 与结果谓词，删除本机路径、会话标识和完整串口日志；未上传这些原始文件。
