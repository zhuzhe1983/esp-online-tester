# PaperColor C151：官方固件运行范围

PaperColor 使用独立板型 `papercolor`。固定官方出厂 v1.0.1 完整镜像已在真实 WASM 和浏览器中从 ROM 启动：第一次显示彩色引导图、写入 NVS 后请求关机；点击平台重启保留该标志，进入官方 AP 配置二维码页面。3D 顶部按键 C（G1）长按能触发新的 `AP re-enabled by long press`，随后由客体刷新屏幕并输出 RGB。目录启用为部分适配，官方出厂固件保持默认第一项。

## 硬件和来源

[来源收据](evidence/papercolor/sources.json) 固定官方产品页、使用说明、V0.5 原理图、M5PM1 v1.9、Sensirion SHT4x v7.3。产品为 ESP32-S3R8、16 MB Flash / 8 MB OPI PSRAM，400×600 Spectra 6 ED2208/EL040EF1 彩色纸屏。不能复用 PaperMono 的 SSD1677 或 PaperDIY 的并行接口。

官方 UserDemo 源码固定 `1ff998e0cf3b9ce916f2e0319561d043db21cc4a`，M5GFX 子模块 `5268353cffabd2925b87ed9bcc05ded4e22cc3d1`、M5Unified `46aa9ede24016a70f397884e8fb3c0ccc3a05269`。已检查开放 PR，没有 ED2208/PaperColor/SHT4x 的重复实现；相邻 GP-SPI service #312、SPI1 #313 不在本次修改范围。

| 部件 | 实际接线与实现 |
| --- | --- |
| ED2208 | G15 CLK、G13 MOSI、G44 CS、G43 DC、G11 BUSY 低有效、G12 RESET；M5PM1 PYG0 供电。固定 120,000 字节 4bpp RAM、有限参数、更新/休眠/复位；断电保留可见画面。 |
| 内部 I²C | SDA3/SCL2，控制器和位操作共享总线；M5PM1 0x6e、RX8130 0x32、SHT40 0x44。 |
| SHT40 | FD/F6/E0 测量、89 序列号、94 复位；转换延迟、CRC8、忙碌 NACK 和单次读取。环境固定为合成的 25°C / 50% RH；不模拟加热器、模拟噪声。 |
| RX8130 | 仅寄存器 RAM，满足识别和固件标志读写；不模拟持续走时、闹钟或电池域。 |
| 按键 | 下键 G9 / BtnA，上键 G10 / BtnB，顶键 G1 / BtnC。实体、键盘和 agent 使用同一路由；无触摸屏，拖动屏幕旋转模型。 |
| RGB | G21 两颗 WS2812，客体 RMT 驱动；M5PM1 LDO3V3_EN（PWR_CFG 位 2、HOLD_CFG 位 5）控制供电。断电丢弃旧锁存颜色。 |
| TF | SPI G15/G13/G14/CS47，PMIC PYG1 检测、PYG3 供电、PYG4 检测开关。当前模型明确返回无卡。 |

M5GFX 配置中出现 `pin_rst=43`，但此前已显式复位 G12；物理连接采用原理图 G12，不把 DC 当 RESET。屏幕色码为黑 0、白 1、黄 2、红 3、蓝 5、绿 6；颜色取厂家驱动的理想 RGB 表。BUSY 使用有界的数字完成延迟（上电 1 ms、刷新 20 ms），没有校准实物 15–30 秒的光学刷新或温度依赖。

## 固件差异和实际验收

固定远程出厂镜像 16,777,216 字节，SHA-256 为 `0f9c9c0f429cfae2c4ab39ca842bbddf7b1e6f8eb29bbfd2826cb9903f7ee37e`，IDF 5.5.1。应用描述为 `48a1057-dirty`，不能把当前同名 v1.0.1 源码认作逐字节对应版本。当前源码把长按 AP 操作写在 `M5.BtnA`；最初据此测试 G10/G9，没有观察到预期白灯与 AP 动作。分别检查三个实际输入后，发行镜像在 G1 顶键触发 AP；官方使用说明也明确顶键 C 长按。保留这个差异，不通过交换 GPIO 来迁就源码猜测。

[真实客体收据](evidence/papercolor/factory-wasm.json) 记录未修改的镜像、WASM/ROM hash、首帧和重启后帧 hash、NVS 改变页数、输入实际投递时间、RMT 输出以及相关串口行。无网络连接时 A/B 的翻图行为尚未验证；仅核对物理位置、GPIO 和库映射。二维码出现和 guest HTTP server-ready 不代表浏览器能访问该门户。

[浏览器收据](evidence/papercolor/browser.json) 验证了真实下载、首启和重启、顶部 CAD 命中长按的新串口响应、2D 400×600 帧，以及共享 mini 页面保留设备、固件和材质后启动。截图与完整串口只留本地检查，避免提交二维码、合成 Wi-Fi 标识或浏览器会话信息。不是以 API 接受输入或模型亮色代替固件响应。

[模型重建收据](evidence/papercolor/model.json) 的 7 个厂家源文件、几何和贴图逐字节一致；修正上下按钮语义，两个灯罩绑定真实 RGB 链。金色外壳不改变键帽、屏幕、纸贴和灯罩；紫色半透明背面能读到 UNKNOWN 主板占位。没有 PCB 布局，左右灯孔与串联索引的物理次序仍未校准；当前官方镜像同时输出两灯相同颜色，不能据此证明灯序。保留原 CAD 和照片的尺寸/贴图验收限制，不宣称内部 1:1 还原。

## 回归与复现

[检查收据](evidence/papercolor/checks.json)：最新 stable 1.99.1、双 Clippy 零警告、1,439 Rust 测试 / 132 组（含 ignored，跳过 external）、11 个真实 WASM 场景、100 个平台合同、18 事件录制回放帧一致、目录 schema、timing/BLE/worker、111,126 JIT 差分 + 24 GPIO 波形、profiling 构建、保留性能收据合同均通过。没有重生成黄金输出。

```sh
tools/wasm-build.sh
node tools/probe-papercolor-factory.mjs --url <目录中的固定官方镜像URL> --sha256 0f9c9c0f429cfae2c4ab39ca842bbddf7b1e6f8eb29bbfd2826cb9903f7ee37e
python3 tools/rebuild-product-model.py build m5stack-c151 --scratch output/papercolor-rebuild
python3 docs/platform/evidence/papercolor/compare-cpu.py --base <固定基线bin目录> --candidate target/release --root .
```

CPU 收据见 [三组 hello 五对对照](evidence/papercolor/cpu.json)。这是冷路径板型回归，不是新的 CPU 加速实验；未修改 S3 核心或 JIT，所以不要求 Pocket Tank 核心性能实验。参考源码为固定 main `84e67571f0d1aecddf77d2432678fec81baabf72`，其既有 S3/C3/C6 生产路径与本次基线 `f14d5634` 逐字节相同。S3 CPU 中位数不变、C3 +0.877%、C6 +1.290%；全部串口 hash 和停止指令数一致。短样本有重叠和随时间漂移，保留实测偏差，不把它包装成严格零开销。

## 尚未覆盖

没有实际硬件验证；没有 Wi-Fi RF、宿主网络桥接或 HTTP 门户交换，没有 TF、音频、可选 Stamp1262 LoRa、物理 PMIC 电源键、RTC 闹钟/唤醒及真实电源域。首次出厂引导后的关机请求不会让宿主网页关机，需点击平台重启。SHT40 的协议单测通过，不代表已验证官方 HTTP 温湿度接口。UIFlow2 仍为可尝试运行的未验证固件，不继承本次固定出厂镜像的验收。
