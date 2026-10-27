# 产品板级适配队列

基准日期：2026-10-27。目录共 83 项，开始本轮时 6 项使用 4 个板级模型；待处理 77 项。AtomS3、AtomS3R 与 Dial K130 已完成本轮部分适配交付：现有 9 项部分适配；加入 Unit C6L U202 与 Metalio INK4 / Plus 后目录为86项，77项尚无产品板级模型。`partial` 不代表整机或任意固件已通过。

采用逐台交付：查官方原理图/驱动 → 实现真实引脚与器件 → 源码构建定向固件 → WASM 输出/输入检查 → 浏览器模型检查 → 记录缺项。每一步失败都保留原因；不能把 `none` 通用芯片模型改名后算作适配。

## 并发分工

总共三条工作线：主线程和两个子代理。主线程没有额外任务时，主线程也参与适配，形成两路控制器、一路配件；主线程处理产品功能、修复或用户问题时，两个子代理分别负责控制器和配件。每路逐项交付，先保存检查点再换任务，不额外增加第四条工作线。

本轮控制器线先补 PaperMono 完整版 NFC 通信与板型识别，再补 LoRa 收发；之后接续已保存工作的 AtomS3-Lite C124、DinMeter K134。AtomS3R 官方 v0.3 的 CPU1 启动异常已完成三种执行路径的复现与源码定位，尚缺 S3 实机行为依据；保持已知失败和试运行入口，不以猜测修改底层行为。

配件线依次适配 Unit RGB U003、Unit Angle U005，先逐项核对实际电路、供电与主机路由，再实现客体输出或输入。Unit Button U027 → StopWatch Port A 已实现实际 GPIO 行为并合并；功能回归通过，C6 hello 本机 CPU 中位数约 +1.875% 的偏差保留记录。Dial K130 运行时与两版 PCB 占位修复已合并；K130-V11 尚无板级模型。

每项按可独立验收的阶段提交，主线程审查、整合并更新本表；子任务交付阶段后接续下一阶段或下一项资料核对，遇到外部依赖时保留检查点并报告。未完成的研究工作树保留，不能把排队、已抓资料或菜单出现算作已适配。

## 首台与接续顺序

- AtomS3 C123：本轮 GC9107、GPIO41 按键、MPU6886 轮询子集。证据见 [AtomS3 记录](atoms3-runtime.md)。
- AtomS3R C126：本轮 ST7735 批次、LP5562白光直控、GPIO41、BMI270轮询子集与8MB Octal PSRAM。BMM150 AUX、RGB/IR、旧GC9107批次及物理姿态校准暂缺。证据见 [AtomS3R记录](atoms3r-runtime.md)。
- AtomS3R 官方 v0.3：已复现屏幕初始化前 CPU1 启动异常；内部定向固件通过不代表官方固件通过，修复前保持 trial。
- PaperMono 完整版：NFC 探测缺失导致 UserDemo 自动选择 Lite 界面，隐藏 NFC/LoRa 测试；后续先补板型识别所依赖的 NFC 通信，再验证 LoRa 收发。见 [实际判断依据](papermono-runtime.md#为什么完整版没有显示-lora-测试)。
- 下一台 AtomS3-Lite C124：复用 S3 执行、GPIO 和 RMT 解码；按官方图补 GPIO41 按键、WS2812C-2020 灯（当前 C124 官方资料，G35）以及 IR/电源约束。先检查软件 GPIO 与 RMT 两种灯驱动，3D 灯颜色必须来自客体输出。
- Dial K130 / v1.1：GC9A01 可复用 DCS 框架；FT3267 触摸、旋转编码器、RTC/RFID 各自按来源建模，版本引脚分别核对。先交付屏幕/触摸/编码器，RTC/RFID 明示阶段缺项。
- DinMeter K134 / v1.1：ST7789 复用可行，接续电源管理、编码器和 RTC；不同屏幕批次与引脚分别核对。
- Stamp-S3 系列：实现真实按键/地址灯及供电控制；基础版通过后逐 SKU 核对封装引脚和固件配置。
- 经典 ESP32 Atom 与 C6 产品随后处理；ESP32-H2/C5 需要先补芯片级执行与外设平台，不用现有 S3/C6 模型冒充。

## 77 项工作清单

下表的“缺项/入口”是待核对范围，不是已确认的完整 BOM。官方入口链接与目录现有来源绑定；实现前还须读取原理图和固定版本源码。顺序兼顾可复用程度，芯片平台缺失项排后。

| 顺序 | 产品 | 芯片 / 屏幕 | 当前状态 | 缺项/入口 |
| --- | --- | --- | --- | --- |
| 1 | [M5Stack AtomS3 · C123](https://docs.m5stack.com/en/core/AtomS3) · `m5stack-c123` | esp32s3 / GC9107 | 本轮已完成 partial：GC9107 / 按键 / IMU 子集 | 背光、IR、IMU FIFO/时序、屏幕批次 |
| 2 | [M5Stack AtomS3R](https://docs.m5stack.com/en/core/AtomS3R) · `m5stack-c126` | esp32s3 / ST7735 | 本轮已完成 partial：屏幕 / 背光 / 按键 / BMI270 / PSRAM | BMM150 AUX、RGB/IR、旧GC9107批次、IMU时序/物理坐标 |
| 3 | [M5Stack AtomS3-Lite](https://docs.m5stack.com/en/core/AtomS3-Lite) · `m5stack-c124` | esp32s3 / 无屏幕 | 待实现 | 按键 / LED / 供电 / 专用传感器与音频等 BOM 核对 |
| 4 | [M5Stack Dial · K130](https://docs.m5stack.com/en/core/Dial) · `m5stack-k130` | esp32s3 / GC9A01 | 本轮已完成 partial：GC9A01 / 触摸 / 按压 / 编码器 | RTC/RFID、音频、电源与背光尚未实现 |
| 5 | [M5Stack Dial v1.1](https://docs.m5stack.com/en/core/Dial_v1.1) · `m5stack-k130-v11` | esp32s3 / GC9A01 | 待实现 | GC9A01 引脚/窗口/批次，按键/触摸，供电及板载外设 |
| 6 | [M5Stack DinMeter](https://docs.m5stack.com/en/core/DinMeter) · `m5stack-k134` | esp32s3 / ST7789V2 | 待实现 | ST7789V2 引脚/窗口/批次，按键/触摸，供电及板载外设 |
| 7 | [M5Stack DinMeter v1.1 · K134-V11](https://docs.m5stack.com/en/core/DinMeter_v1.1) · `m5stack-k134-v11` | esp32s3 / ST7789P3 | 待实现 | ST7789P3 引脚/窗口/批次，按键/触摸，供电及板载外设 |
| 8 | [M5Stack Stamp-S3 · S007](https://docs.m5stack.com/en/core/Stamp-S3) · `m5stack-s007` | esp32s3 / 无屏幕 | 待实现 | 按键 / LED / 供电 / 专用传感器与音频等 BOM 核对 |
| 9 | [M5Stack Stamp-S3A · S007-V033](https://docs.m5stack.com/en/core/Stamp-S3A) · `m5stack-s007-v033` | esp32s3 / 无屏幕 | 待实现 | 按键 / LED / 供电 / 专用传感器与音频等 BOM 核对 |
| 10 | [M5Stack Atom-Lite · C008](https://docs.m5stack.com/en/core/Atom-Lite) · `m5stack-c008` | esp32 / 无屏幕 | 待实现 | 按键 / LED / 供电 / 专用传感器与音频等 BOM 核对 |
| 11 | [M5Stack Atom-Matrix](https://docs.m5stack.com/en/core/Atom-Matrix) · `m5stack-c008-b` | esp32 / 无屏幕 | 待实现 | 按键 / LED / 供电 / 专用传感器与音频等 BOM 核对 |
| 12 | [M5Stack NanoC6 · C125](https://docs.m5stack.com/en/core/NanoC6) · `m5stack-c125` | esp32c6 / 无屏幕 | 待实现 | 按键 / LED / 供电 / 专用传感器与音频等 BOM 核对 |
| 13 | [Arduino Nesso N1 · DK001](https://docs.m5stack.com/en/core/Arduino_Nesso_N1) · `m5stack-dk001` | esp32c6 / ST7789 | 待实现 | ST7789 引脚/窗口/批次，按键/触摸，供电及板载外设 |
| 14 | [M5Stack Air Quality](https://docs.m5stack.com/en/core/Air_Quality) · `m5stack-k131` | esp32s3 / GDEY0154D67 | 待实现 | GDEY0154D67 刷新通道、存储、触摸/按键、电源 |
| 15 | [M5Stack Air Quality v1.1 · K131-V11](https://docs.m5stack.com/en/core/Air_Quality_v1.1) · `m5stack-k131-v11` | esp32s3 / GDEY0154D67 | 待实现 | GDEY0154D67 刷新通道、存储、触摸/按键、电源 |
| 16 | [M5Stack Atom Voice](https://docs.m5stack.com/en/atom/Atom_Voice) · `m5stack-c008-c` | esp32 / 无屏幕 | 待实现 | 按键 / LED / 供电 / 专用传感器与音频等 BOM 核对 |
| 17 | [M5Stack Atom VoiceS3R](https://docs.m5stack.com/en/core/Atom_VoiceS3R) · `m5stack-c126-echo` | esp32s3 / 无屏幕 | 待实现 | 按键 / LED / 供电 / 专用传感器与音频等 BOM 核对 |
| 18 | [M5Stack Atom-Matrix v1.1 · C008-B-V11](https://docs.m5stack.com/en/core/Atom-Matrix_v1.1) · `m5stack-c008-b-v11` | esp32 / 无屏幕 | 待实现 | 按键 / LED / 供电 / 专用传感器与音频等 BOM 核对 |
| 19 | [M5Stack AtomS3R AI Chatbot Kit · K147](https://docs.m5stack.com/en/core/AtomS3R-AI_Chatbot) · `m5stack-k147` | esp32s3 / GC9107 | 待实现 | GC9107 引脚/窗口/批次，按键/触摸，供电及板载外设 |
| 20 | [M5Stack AtomS3R-CAM AI Chatbot · K147-CAM](https://docs.m5stack.com/en/core/AtomS3R-CAM_AI_Chatbot) · `m5stack-k147-cam` | esp32s3 / 无屏幕 | 待实现 | 按键 / LED / 供电 / 专用传感器与音频等 BOM 核对 |
| 21 | [M5Stack AtomS3R-CAM · C126-CAM](https://docs.m5stack.com/en/core/AtomS3R-CAM) · `m5stack-c126-cam` | esp32s3 / 无屏幕 | 待实现 | 按键 / LED / 供电 / 专用传感器与音频等 BOM 核对 |
| 22 | [M5Stack AtomS3R-Ext · C126-Ext](https://docs.m5stack.com/en/core/AtomS3R-Ext) · `m5stack-c126-ext` | esp32s3 / 无屏幕 | 待实现 | 按键 / LED / 供电 / 专用传感器与音频等 BOM 核对 |
| 23 | [M5Stack AtomS3R-M12](https://docs.m5stack.com/en/core/AtomS3R-M12) · `m5stack-c126-m12` | esp32s3 / 无屏幕 | 待实现 | 按键 / LED / 供电 / 专用传感器与音频等 BOM 核对 |
| 24 | [M5Stack AtomS3R-M12 Volcengine Kit · D062-M12](https://docs.m5stack.com/en/core/AtomS3R-M12_Volcengine_Kit) · `m5stack-d062-m12` | esp32s3 / 无屏幕 | 待实现 | 按键 / LED / 供电 / 专用传感器与音频等 BOM 核对 |
| 25 | [M5Stack AtomS3U · K125](https://docs.m5stack.com/en/core/AtomS3U) · `m5stack-k125` | esp32s3 / 无屏幕 | 待实现 | 按键 / LED / 供电 / 专用传感器与音频等 BOM 核对 |
| 26 | [M5Stack Basic](https://docs.m5stack.com/en/core/Basic) · `m5stack-k001` | esp32 / ILI9342C | 待实现 | ILI9342C 引脚/窗口/批次，按键/触摸，供电及板载外设 |
| 27 | [M5Stack Basic v2.7](https://docs.m5stack.com/en/core/Basic_v2.7) · `m5stack-k001-v27` | esp32 / ILI9342C | 待实现 | ILI9342C 引脚/窗口/批次，按键/触摸，供电及板载外设 |
| 28 | [M5Stack Capsule](https://docs.m5stack.com/en/core/Capsule) · `m5stack-k129` | esp32s3 / 无屏幕 | 待实现 | 按键 / LED / 供电 / 专用传感器与音频等 BOM 核对 |
| 29 | [M5Stack Capsule v1.1 · K129-V11](https://docs.m5stack.com/en/core/Capsule_v1.1) · `m5stack-k129-v11` | esp32s3 / 无屏幕 | 待实现 | 按键 / LED / 供电 / 专用传感器与音频等 BOM 核对 |
| 30 | [M5Stack Cardputer Mesh Kit](https://docs.m5stack.com/en/core/Cardputer_Mesh_Kit) · `m5stack-k152` | esp32s3 / ST7789V2 | 待实现 | ST7789V2 引脚/窗口/批次，按键/触摸，供电及板载外设 |
| 31 | [M5Stack Core2](https://docs.m5stack.com/en/core/Core2) · `m5stack-k010` | esp32 / ILI9342C / ILI9342E | 待实现 | ILI9342C / ILI9342E 引脚/窗口/批次，按键/触摸，供电及板载外设 |
| 32 | [M5Stack Core2 For AWS](https://docs.m5stack.com/en/core/Core2_For_AWS) · `m5stack-k010-aws` | esp32 / ILI9342C / ILI9342E | 待实现 | ILI9342C / ILI9342E 引脚/窗口/批次，按键/触摸，供电及板载外设 |
| 33 | [M5Stack Core2 for AWS v1.3 · K010-AWS-V13](https://docs.m5stack.com/en/core/Core2_For_AWS_v1.3) · `m5stack-k010-aws-v13` | esp32 / ILI9342C / ILI9342E | 待实现 | ILI9342C / ILI9342E 引脚/窗口/批次，按键/触摸，供电及板载外设 |
| 34 | [M5Stack Core2 v1.1](https://docs.m5stack.com/en/core/Core2_v1.1) · `m5stack-k010-v11` | esp32 / ILI9342C / ILI9342E | 待实现 | ILI9342C / ILI9342E 引脚/窗口/批次，按键/触摸，供电及板载外设 |
| 35 | [M5Stack Core2 v1.3](https://docs.m5stack.com/en/core/Core2_v1.3) · `m5stack-k010-v13` | esp32 / ILI9342C / ILI9342E | 待实现 | ILI9342C / ILI9342E 引脚/窗口/批次，按键/触摸，供电及板载外设 |
| 36 | [M5Stack CoreInk](https://docs.m5stack.com/en/core/CoreInk) · `m5stack-k048` | esp32 / GDEW0154M09 | 待实现 | GDEW0154M09 刷新通道、存储、触摸/按键、电源 |
| 37 | [M5Stack CoreS3](https://docs.m5stack.com/en/core/CoreS3) · `m5stack-k128` | esp32s3 / ILI9342C / ILI9342E | 待实现 | ILI9342C / ILI9342E 引脚/窗口/批次，按键/触摸，供电及板载外设 |
| 38 | [M5Stack CoreS3 Thread BR](https://docs.m5stack.com/en/core/CoreS3_Thread_BR) · `m5stack-k149` | esp32s3 / ILI9342C / ILI9342E | 待实现 | ILI9342C / ILI9342E 引脚/窗口/批次，按键/触摸，供电及板载外设 |
| 39 | [M5Stack CoreS3-Lite](https://docs.m5stack.com/en/core/CoreS3-Lite) · `m5stack-k128-lite` | esp32s3 / ILI9342C / ILI9342E | 待实现 | ILI9342C / ILI9342E 引脚/窗口/批次，按键/触摸，供电及板载外设 |
| 40 | [M5Stack CoreS3-SE · K128-SE](https://docs.m5stack.com/en/core/CoreS3-SE) · `m5stack-k128-se` | esp32s3 / ILI9342C | 待实现 | ILI9342C 引脚/窗口/批次，按键/触摸，供电及板载外设 |
| 41 | [M5Stack Faces Kit](https://docs.m5stack.com/en/core/Faces_Kit) · `m5stack-k005` | esp32 / ILI9342C | 待实现 | ILI9342C 引脚/窗口/批次，按键/触摸，供电及板载外设 |
| 42 | [M5Stack FIRE v2.7](https://docs.m5stack.com/en/core/Fire_v2.7) · `m5stack-k007-v27` | esp32 / ILI9342C | 待实现 | ILI9342C 引脚/窗口/批次，按键/触摸，供电及板载外设 |
| 43 | [M5Stack M5GO IoT Kit v2.7](https://docs.m5stack.com/en/core/M5GO_IoT_Kit_v2.7) · `m5stack-k006-v27` | esp32 / ILI9342C | 待实现 | ILI9342C 引脚/窗口/批次，按键/触摸，供电及板载外设 |
| 44 | [M5Stack Paper v1.1](https://docs.m5stack.com/en/core/Paper_v1.1) · `m5stack-k049-b` | esp32 / IT8951E | 待实现 | IT8951E 刷新通道、存储、触摸/按键、电源 |
| 45 | [M5Stack PaperColor · C151](https://docs.m5stack.com/en/core/PaperColor) · `m5stack-c151` | esp32s3 / ED2208-DOA / EL040EF1 | 待实现 | ED2208-DOA / EL040EF1 刷新通道、存储、触摸/按键、电源 |
| 46 | [M5Stack PaperDIY Complete Kit · K159-B](https://docs.m5stack.com/en/core/PaperDIY_Complete_Kit) · `m5stack-k159-b` | esp32s3 / ET047TC1 | 待实现 | ET047TC1 刷新通道、存储、触摸/按键、电源 |
| 47 | [M5Stack PaperS3](https://docs.m5stack.com/en/core/PaperS3) · `m5stack-c139` | esp32s3 / ET047TC1 | 待实现 | ET047TC1 刷新通道、存储、触摸/按键、电源 |
| 48 | [M5Stack PowerHub](https://docs.m5stack.com/en/core/PowerHub) · `m5stack-c148` | esp32s3 / 无屏幕 | 待实现 | 按键 / LED / 供电 / 专用传感器与音频等 BOM 核对 |
| 49 | [M5Stack StackChan](https://docs.m5stack.com/en/StackChan) · `m5stack-k151` | esp32s3 / ILI9342C / ILI9342E | 待实现 | ILI9342C / ILI9342E 引脚/窗口/批次，按键/触摸，供电及板载外设 |
| 50 | [M5Stack Stamp-C3](https://docs.m5stack.com/en/core/Stamp-C3) · `m5stack-c056-b` | esp32c3 / 无屏幕 | 待实现 | 按键 / LED / 供电 / 专用传感器与音频等 BOM 核对 |
| 51 | [M5Stack Stamp-C3 Mate · K056](https://docs.m5stack.com/en/core/Stamp-C3_Mate) · `m5stack-k056` | esp32c3 / 无屏幕 | 待实现 | 按键 / LED / 供电 / 专用传感器与音频等 BOM 核对 |
| 52 | [M5Stack Stamp-C3U](https://docs.m5stack.com/en/core/Stamp-C3U) · `m5stack-c122-b` | esp32c3 / 无屏幕 | 待实现 | 按键 / LED / 供电 / 专用传感器与音频等 BOM 核对 |
| 53 | [M5Stack Stamp-C3U Mate](https://docs.m5stack.com/en/core/Stamp-C3U_Mate) · `m5stack-k122` | esp32c3 / 无屏幕 | 待实现 | 按键 / LED / 供电 / 专用传感器与音频等 BOM 核对 |
| 54 | [M5Stack Stamp-C6LoRa](https://docs.m5stack.com/en/core/Stamp_C6LoRa) · `m5stack-s012` | esp32c6 / 无屏幕 | 待实现 | 按键 / LED / 供电 / 专用传感器与音频等 BOM 核对 |
| 55 | [M5Stack Stamp-P4](https://docs.m5stack.com/en/core/Stamp-P4) · `m5stack-s013` | esp32p4 / 无屏幕 | 待实现 | 按键 / LED / 供电 / 专用传感器与音频等 BOM 核对 |
| 56 | [M5Stack Stamp-Pico DIY Kit · K051-B](https://docs.m5stack.com/en/core/Stamp-Pico_DIY_Kit) · `m5stack-k051-b` | esp32 / 无屏幕 | 待实现 | 按键 / LED / 供电 / 专用传感器与音频等 BOM 核对 |
| 57 | [M5Stack Stamp-Pico Mate](https://docs.m5stack.com/en/core/Stamp-Pico_Mate) · `m5stack-k051` | esp32 / 无屏幕 | 待实现 | 按键 / LED / 供电 / 专用传感器与音频等 BOM 核对 |
| 58 | [M5Stack Stamp-Pico · C050-B](https://docs.m5stack.com/en/core/Stamp-Pico) · `m5stack-c050-b` | esp32 / 无屏幕 | 待实现 | 按键 / LED / 供电 / 专用传感器与音频等 BOM 核对 |
| 59 | [M5Stack Stamp-S3 PIN1.27](https://docs.m5stack.com/en/core/Stamp-S3_PIN1.27) · `m5stack-s007-pin127` | esp32s3 / 无屏幕 | 待实现 | 按键 / LED / 供电 / 专用传感器与音频等 BOM 核对 |
| 60 | [M5Stack Stamp-S3 PIN2.54 · S007-PIN254](https://docs.m5stack.com/en/core/Stamp-S3_PIN2.54) · `m5stack-s007-pin254` | esp32s3 / 无屏幕 | 待实现 | 按键 / LED / 供电 / 专用传感器与音频等 BOM 核对 |
| 61 | [M5Stack Stamp-S3A PIN2.54](https://docs.m5stack.com/en/core/Stamp-S3A_PIN2.54) · `m5stack-s007-pin254-v033` | esp32s3 / 无屏幕 | 待实现 | 按键 / LED / 供电 / 专用传感器与音频等 BOM 核对 |
| 62 | [M5Stack Stamp-S3Bat DIP](https://docs.m5stack.com/en/core/Stamp-S3Bat_DIP) · `m5stack-s015-dip` | esp32s3 / 无屏幕 | 待实现 | 按键 / LED / 供电 / 专用传感器与音频等 BOM 核对 |
| 63 | [M5Stack Stamp-S3Bat · S015](https://docs.m5stack.com/en/core/Stamp-S3Bat) · `m5stack-s015` | esp32s3 / 无屏幕 | 待实现 | 按键 / LED / 供电 / 专用传感器与音频等 BOM 核对 |
| 64 | [M5Stack StampFly](https://docs.m5stack.com/en/app/StampFly) · `m5stack-k138` | esp32s3 / 无屏幕 | 待实现 | 按键 / LED / 供电 / 专用传感器与音频等 BOM 核对 |
| 65 | [M5Stack StamPLC · K141](https://docs.m5stack.com/en/core/StamPLC) · `m5stack-k141` | esp32s3 / ST7789v2 | 待实现 | ST7789v2 引脚/窗口/批次，按键/触摸，供电及板载外设 |
| 66 | [M5Stack Station-485 · K123](https://docs.m5stack.com/en/core/Station-485) · `m5stack-k123` | esp32 / ST7789V2 | 待实现 | ST7789V2 引脚/窗口/批次，按键/触摸，供电及板载外设 |
| 67 | [M5Stack Station-Bat](https://docs.m5stack.com/en/core/Station-Bat) · `m5stack-k124-b` | esp32 / ST7789V2 | 待实现 | ST7789V2 引脚/窗口/批次，按键/触摸，供电及板载外设 |
| 68 | [M5Stack StickC · K016-C](https://docs.m5stack.com/en/core/StickC) · `m5stack-k016-c` | esp32 / ST7735SV | 待实现 | ST7735SV 引脚/窗口/批次，按键/触摸，供电及板载外设 |
| 69 | [M5Stack StickC-Plus](https://docs.m5stack.com/en/core/StickC-Plus) · `m5stack-k016-p` | esp32 / ST7789V2 | 待实现 | ST7789V2 引脚/窗口/批次，按键/触摸，供电及板载外设 |
| 70 | [M5Stack StickS3 · K150](https://docs.m5stack.com/en/core/StickS3) · `m5stack-k150` | esp32s3 / ST7789P3 | 待实现 | ST7789P3 引脚/窗口/批次，按键/触摸，供电及板载外设 |
| 71 | [M5Stack StickT2](https://docs.m5stack.com/en/core/StickT2) · `m5stack-k016-t2` | esp32 / ST7789 | 待实现 | ST7789 引脚/窗口/批次，按键/触摸，供电及板载外设 |
| 72 | [M5Stack Tab5 · C145](https://docs.m5stack.com/en/core/Tab5) · `m5stack-c145` | esp32p4 / ILI9881C / ST7123 / ST7121（按批次核对） | 待实现 | ILI9881C / ST7123 / ST7121（按批次核对） 引脚/窗口/批次，按键/触摸，供电及板载外设 |
| 73 | [M5Stack Tough](https://docs.m5stack.com/en/core/Tough) · `m5stack-k034` | esp32 / ILI9342C | 待实现 | ILI9342C 引脚/窗口/批次，按键/触摸，供电及板载外设 |
| 74 | [M5Stack VAMeter](https://docs.m5stack.com/en/core/VAMeter) · `m5stack-k136` | esp32s3 / ST7789 | 待实现 | ST7789 引脚/窗口/批次，按键/触摸，供电及板载外设 |
| 75 | [M5Stack Xiaozhi Card Kit · K146](https://docs.m5stack.com/en/core/Xiaozhi_Card_Kit) · `m5stack-k146` | esp32s3 / GDEY027T91 | 待实现 | GDEY027T91 刷新通道、存储、触摸/按键、电源 |
| 76 | [M5Stack NanoH2](https://docs.m5stack.com/en/core/NanoH2) · `m5stack-c149` | esp32h2 / 无屏幕 | 待实现 | 先实现对应芯片平台，再核对产品外设 |
| 77 | [M5Stack Stamp-C5](https://docs.m5stack.com/en/core/Stamp-C5) · `m5stack-s016` | esp32c5 / 无屏幕 | 待实现 | 先实现对应芯片平台，再核对产品外设 |

## 新增双角色设备

- Unit C6L U202：控制器与配件共用资料/模型，板级运行尚未实现。后续核对ESP32-C6、SSD1306 SPI、PI4IOE5V6408、SX1262、WS2812C与蜂鸣器；与Stamp-C6LoRa的封装/引脚差异分别记录。Meshtastic稳定版及Alpha已按准确板型加入watchlist，不能以固件已收录视作模拟器就绪。见[固件记录](meshtastic-watchlist.md)。

## 无名科技 / Metalio 新增系列

- `metalio-ink4`：ESP32-S3 / SSD1677 / CST816S / TCA9555，需独立引脚、电源、SDMMC与传感器接入，不直接复用PaperMono板名。
- `metalio-ink4-plus`：ESP32-S31 / 并行EPDiy / TPS65185 / FT6336U，先补S31芯片级支持再实现板级；不能作为ESP32-S3运行。

[产品、官方固件与模型边界](metalio-onboarding.md)已记录；两款当前均为 unavailable，不以外观入库计作适配完成。
