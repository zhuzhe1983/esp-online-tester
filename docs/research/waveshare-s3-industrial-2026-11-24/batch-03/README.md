# S3 工业及基础板第三组：5 个产品家族，19 个精确 SKU

本提交提供精确板型、源资料、原厂固件链接、知识记录和可重建机械模型。中央产品目录、固件目录、board registry 以及原始 BIN 的 Worker 运行观测由集成批次另行完成。这里的通过记录只覆盖明确列出的数字行为和几何检查，没有把原厂完整应用、WebGL 浏览器交互或实物验收标为通过。

| 家族 | SKU 数 | 本次区别 |
|---|---:|---|
| ESP32-S3-ETH-8DI-8RO | 2 | 原版 RS485，普通网络与 PoE；不混入带 CAN 的 C 版本 |
| ESP32-S3-ETH | 8 | 普通 / CAM / PoE / PoE-CAM，各自无排针与 M 排针版本 |
| ESP32-S3-LCD-Driver-Board | 4 | 裸板，以及 2.1 / 2.8 / 4 英寸 RGB 套件；目录中的暂不可售选项也保留 |
| ESP32-S3-Tiny | 4 | FH4R2 与 PICO1-N8R8；各自裸板和 Adapter 套件 |
| ESP32-S3-Relay-6CH | 1 | 默认 WROOM-1U-N16，16 MiB Flash、0 MiB PSRAM |

精确 SKU、产品 ID、运行时 profile 和模型路径列在 `manifest.json` 及 `tools/waveshare-batches/s3-industrial/<listingId>.json`。这五个 fragment 含 19 个产品、19 个知识记录、18 个固件记录及 18 个 manual watchlist 条目。没有共享或借用其他设备的硬件验收结论。

## 原始资料和固件身份

各家族的 `sources.json` 记录原始 URL、抓取时间、字节数、SHA-256，以及派生文本 / 选定硬件声明的独立哈希。原始商城会话和样例 Wi-Fi 凭据没有复制进证据目录。LCD 仓库历史只保留 commit / tree / 日期，省略个人作者身份；仓库日期不作为设备发布时间。

- 老 8DI8RO 当前 Wiki 返回 403。保留制造商撰写的 Wiki PDF 镜像并明确标注镜像主机，以原厂商城、下载 ZIP、原始 GPIO / I²C / SD 驱动交叉核对。
- ETH 原厂指南称 `Factory_Testing.bin` 为板载测试程序，并给出 `0x0` 写入位置。已取得文本没有明确说明它就是预装出厂程序，所以它是 **official project trial**。未解决的出厂程序仍保留为 reference，产品显式默认选择板载测试 trial。`ETH_Web_CAM.bin` 只绑定四个含摄像头 SKU。
- Relay6 和老 8DI8RO 的原始出厂 BIN 有原厂指南依据。Relay6 官方 MicroPython 单列为 project trial。
- Tiny 没有找到精确出厂镜像。文档中的通用 4 MB MicroPython 包没有被替代为 N8R8 镜像。
- LCD 历史 RGB ZIP 是 **541,714,255 字节、26,972 条目、2,329,844,170 字节展开大小**，同时超过现有 256 MiB / 16,384 条目 / 1 GiB 的浏览器 ZIP 边界。完整原包用于原理图和 RGB BSP 调查；没有修改上限，也没有把分离的 bootloader / partition / app 重新合并或重编译成“原厂”镜像。当前固定 GitHub 提交中的 SPI 示例不作为本组 RGB 套件的接线依据。

`firmware-image-audit.json` 中的五个原始 BIN 均通过生产 ZIP decoder 的容器 SHA、entry CRC、BIN SHA、ESP32-S3 chip ID 和 16 MiB merged flash layout 检查。BIN 内容没有修改；解码成功不代表程序已完整运行。UIFlow、小智及 M5Burner 的固定来源没有发现这些精确 SKU 的可替代镜像，保留 reference 和人工更新规则。

## 已实现和验证的数字行为

`runtime-integration.log` 有 12 个真实 LX7 客体测试，`runtime-tca-regression.log` 有 8 个既有 RGB 板型回归测试；`runtime-clippy.log` 是对应代码的 Clippy 检查。

- ETH：一颗 GPIO21 WS2812，GRB 线序；SD 使用 SPI 的 7 / 6 / 5 / 4 引脚。八种 SKU 均执行真实 512 字节 SD 读写，并验证错误 MOSI / MISO、拔插和中途取消写入。原厂通用 RGB 样例中的 60 像素配置没有被当作实板的 60 颗灯。W5500 网络和 OV5640 SCCB / DVP 尚未实现，仅观察原厂连接的 GPIO 和 SPI。
- Tiny：精确 GPIO38 RGB。FH4R2 为 4F2P，N8R8 为 8F8P。GRB 线序依据厂家指定 Arduino 2.0.11 的 `neopixelWrite` 实现；旧示例中的 GPIO14 和通用 Zero 引脚没有覆盖精确原理图。实体 USB、BOOT / RESET 只出现在 Adapter 套件。
- Relay6：六路继电器线圈 GPIO1 / 2 / 41 / 42 / 45 / 46，蜂鸣器 GPIO21，GPIO38 RGB 的实际线序。没有把可定制的 R8 模组当作默认 N16 SKU，也没有从其他可选示例补入 RTC。负载、触点、电气隔离、RS485 对端和声音未建模。
- 老 8DI8RO：TCA9554、PCF85063、八个数字输入、RGB / 蜂鸣器驱动和原厂 1-bit SDMMC 接线；使用已有真实客体 SD DMA 读写检查。普通和 PoE 两款均不带 C 版本的 CAN 终端。
- LCD Driver：8F8P。裸板只有精确 TCA9554，未附加显示屏、触控、RTC、IMU 或 SD。三款套件使用真实 RGB DMA 和 ST7701S SPI 初始化；CS 是 MCU GPIO42，TCA P0 / P1 / P2 分别为触控复位 / 背光使能 / LCD 复位。2.1 寸为 CST820，2.8 / 4 寸为 GT911，保留地址 strap、ACK 和复位行为。GPIO6 使用 AP3032 反馈 PWM，反向占空比是有来源的归一化显示策略，未声称模拟实际电流或标定亮度。

这些客体测试没有通过宿主直接写 framebuffer 来代替 LCD DMA，也没有以宿主直接设置灯色来代替 GPIO / RMT 行为。

## 机械模型和可核查边界

19 个模型全部使用精确来源尺寸与照片重建，没有找到可用于这些具体设备的原生整板 / 整壳 CAD。完整计划及原始照片 / 面板 PDF 保存在各模型目录；`model-plan.json` 可由公共 builder 重建。sourceSearch 描述了实际检查的资源和范围，不声称穷尽厂家所有仓库。

有外壳的三个选项使用独立壳壁、背板和盖板，具有真实空腔、螺孔和端口开口。未知内部板保持空白并明确显示 UNKNOWN，拆分检查显示标签后能恢复原装配位置；没有凭猜测设置穿壳按钮。Relay6 保留 145 × 90 × 40 mm 原厂尺寸，不套用其他型号的外壳。

ETH 的 USB / SD / BOOT / RGB 在反面；PoE 和相机只出现在包含它们的 SKU，M 版本才有焊接排针。相机镜头轴向依据确切 CAM 照片。PoE 隐藏电路、实际间距和接线仍未知。Tiny N8R8 使用自己的大封装与照片，不把 FH4R2 纹理投射到新版本上。适配器和独立 RGB 屏按套件内容分开展示；未提供长度、弯折或装配姿态的 FPC 没有被伪造为已安装连接。

4 英寸面板手册给出 71.86 × 69.51 mm 有效区，但数字图像为 480 × 480。模型明确使用有效区内 69.51 mm 方形图像，保留纵横比，光学像素比例没有标定。2.1 英寸手册通用表的“无触控”与触控接头 / 套件 BSP 的冲突原样记录；未据此猜测触控盖玻璃厚度。

验证文件包括：

- `loader-checks.json`：19 个生产 Three.js 模型的尺寸、源文件哈希、材质隔离、RGB 绑定、按钮正反面遮挡和屏幕光线检查。
- `loader-expanded-checks.json`：12 个存在独立组件的模型展开 / 恢复检查。
- `additional-geometry-checks.json`：裸板 UNKNOWN 标签可见性、三个外壳真实空腔和 USB 开口、10 个 RJ45 内腔光线检查。
- `review/`、`review-expanded/`：31 张四视图，来自实际生产 loader mesh；屏幕统一使用明确标为 review swatch 的颜色。
- `review-photo/`：19 个模型与保留原图的平面对照，单独说明透视、未知高度和估测误差。
- `visual-review.json`：图像人工检查记录及文件哈希。没有把这些 CPU 几何图当作 WebGL、字体 DOM 可读性或物理装配验收。

## 复核入口

```sh
cargo test --release -p esp32s3 --test waveshare_extended --test waveshare_industrial_controls
cargo test --release -p esp32s3 --lib waveshare_tca_rgb -- --skip external_
cargo clippy -p esp32s3 --test waveshare_extended --test waveshare_industrial_controls -- -D warnings
python tools/waveshare-batches/s3-industrial/validate_third.py
node tools/waveshare-batches/s3-industrial/check_third_geometry.mjs
```

模型构建、生产 loader 和 review 工具的使用版本 SHA-256 记录在 `tool-versions.json`。模型计划和固件来源可复查，但未知硬件部分及明确未实现的接口仍是适配范围边界。
