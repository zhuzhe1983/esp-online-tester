# Seeed Studio 显示设备适配记录

本批在 Seeed Studio HMI/Display 范围核对并加入 **48 个产品身份：23 个独立商品或商业版本，加上 25 个官方套件的明确板屏配置**。它们不是 48 个独立制造商 SKU。按有依据的发布时间优先新设备、优先带屏设备；发布日期未知时明确保留未知。四款 EE 裸主控板也单独收录，物理上没有屏幕。完整范围、父 SKU、板 SKU、屏 SKU 与排除理由见 [范围核对](research/seeed-scope-2026-11-22.md)。

目录中的 47 个身份为 `partial`，表示有可执行的数字板级子集；不表示全部外设、云服务、低功耗、对应出厂固件或实体硬件均已验证。D1001 为 `unavailable`，保留模型与原厂恢复镜像，但 P4 运行仍有明确缺口。已有 SKU 的执行结果不会自动继承给共用外壳或运行板的新商品。

用户给出的商店链接依赖动态库存筛选，本轮未取得完整静态结果；范围同时与官方显示索引、具体商品页、PCB 资料和固定提交的官方 Hub 板屏目录交叉核对。暂时缺货和已停产但仍有商品身份的设备没有被漏掉。

仓库 `skills/esp-product-onboarding` 是本批使用的 onboarding skill。它要求优先官方 CAD、调查每台设备的原厂固件、保留未知信息，并以客体固件输出验证输入。设备的完整来源、抓取时间、摘要和型号差异在 `web/platform/knowledge/seeed-*.json`；原厂、UIFlow 和小智调查见 [逐机调查记录](research/seeed-display-factory-audit-2026-11-22.json)。

## 设备范围与验证层次

### 新增 35 个身份

| 设备组 | 数量 | 适配与验证范围 |
| --- | ---: | --- |
| EE02、EE03、EE04、EE05 裸主控板 | 4 | 独立按键、供电、电池分压和 LED 接线；无屏板不会响应屏幕 SPI、驱动 BUSY 或产生假帧。 |
| EE02 + 13.3-inch Spectra 6 | 1 | T133A01、1200×1600、双片选；完整 SenseCraft 1.1.5 原厂四段镜像。 |
| EE03 + 10.3-inch 单色屏 | 1 | IT8951 原生横向 1872×1404；不能套用 E1003 的整机竖屏安装方向。 |
| EE04 + 官方面板选项 | 12 | 7.3-inch 六色，以及 1.54/2.13/2.9/4.2/4.26/5.83/7.5-inch 的明确单色、四色或柔性选项。 |
| EE05 + 官方面板选项 | 11 | 对应 11 个有依据的选项，保留区别于 EE04 的第三按键和 ADC_EN39 接线。 |
| Watcher W1-A 与 XiaoZhi 商业版 | 2 | W1-A 透明壳与 W1-B 白壳分别建模；XiaoZhi 使用独立官方恢复镜像和执行证据。 |
| Indicator D1S、D1L、D1Pro、Meshtastic 版 | 4 | 商业 SKU、传感器/LoRa 配置与固件身份分开；三个带 SX1262 的版本使用独立板型，经真实扩展器和 SPI 接线提供数字无线子集。RP2040、环境传感器和实体 RF 未模拟。 |

25 个板屏配置中，17 个取得完整 SenseCraft 1.1.5 四段固件。实际 WASM 第一轮全部进入原厂应用并完成 ePaper 初始化，六秒内均 `NO_FRAME`；EE04/EE05 7.5-inch 延长至十五秒后仍在 Wi-Fi 初始化且无首帧。它们不是“17 台显示已通过”。EE05 实际报告电量 0%，EE04 报告 63%，与已保留的 ADC_EN39/6 接线冲突吻合。余下 8 个配置有官方选型资料但未取得完整可启动镜像，保留资料入口，不用别的面板固件填充。详见 [EE 板级与原厂执行记录](seeed-ee-runtime.md)。

### 第一阶段 13 个带屏设备

| 设备 | 有依据的发布时间 | 屏幕 | 本批运行范围与已知限制 |
| --- | --- | --- | --- |
| XIAO 1.47-inch IPS Touch Display | 2026-09-15 | 172×320，JD9853A | 原厂 ZIP 的完整 ROM 启动、软件 SPI 显示、按键/背光及 AXS5106L 单点触摸；未模拟 IMU、PDM 麦克风和 SD 传输。 |
| XIAO 1.14-inch IPS Display | 2026-09-15 | 135×240，ST7789 | 原厂 ROM 启动、硬件 SPI、三个按键及背光；第三键引起的页面标题变化有实际固件证据。 |
| XIAO 0.96-inch IPS Display | 2026-09-15 | 80×160，ST7789 | 原厂 ROM 启动、SPI、双按键及背光；仅验证当前原厂使用的 rotation-2 窗口，其他扫描方向仍有限制。 |
| reTerminal Sticky | 2026-07-30 | 800×480，3.97-inch 电子纸 | SSD1677 数字命令、按键、SPI TF 和 GT911 单点输入。SSD2677 量产变体不冒充同一控制器；原厂 1.2.0 的 LittleFS/字体加载仍失败，未验证正常 UI。 |
| reTerminal E1003 | 2026-03-26 | 1404×1872，10.3-inch | IT8951 数字协议、按键、SPI TF；最终 WASM 已输出原厂低电量页面。GT911、PMIC 和实际电子纸波形未实现。控制器原生传输是 1872×1404。 |
| reTerminal D1001 | 2026-03-19 | 800×1280，8-inch MIPI DSI | 官方 CAD、完整 P4 原厂镜像和精确引脚资料已入库。实际 ROM 启动首先阻塞于 P4 ADC 熵源；DSI、DW_GDMA 和硬件 I2C0 也是后续缺口，不注册伪 SPI 运行板。 |
| reTerminal E1004 | 2026-02-05 | 1200×1600，13.3-inch Spectra 6 | T133A01 双片选、屏幕供电、按键、SPI TF、受 GPIO 控制的标称电池分压；native 显式 ADC=0 条件下已观察到原厂低电量画面，默认 WASM 首屏仍未验证。 |
| reTerminal E1001 | 2025-09-04 | 800×480，7.5-inch 单色 | UC8179、按键、SPI TF、受 GPIO 控制的标称电池分压；native 显式 ADC=0 条件下已观察到原厂低电量画面，默认 WASM 首屏仍未验证。 |
| reTerminal E1002 | 2025-09-04 | 800×480，7.3-inch Spectra 6 | 按官方 Seeed_GFX2 的 ED2208/GDEP073E01 建模，含数字六色像素、按键和 SPI TF；native ADC=0 原厂低电量画面已观察到，默认 WASM 首屏未验证。不将未核实的 JD79660 名称等同于该型号。 |
| TRMNL 7.5-inch OG DIY Kit | 2025-07-21 | 800×480，UC8179，ESP32-S3 Plus | 原始 TRMNL 1.8.19 已显示配网页面。GPIO43/U0TXD 的实际供电线路、ADC_EN6 分压和按键分别建模。 |
| XIAO 7.5-inch ePaper Panel | 2025-04，仅月份已知 | 800×480，UC8179，ESP32-C3 | 真实 GPIO/SPI/BUSY 路径及 MMIO 测试通过；TRMNL 1.8.19 在 C3 轻睡眠路径阻塞并发生 RTCWDT 重启，未验证首屏。 |
| SenseCAP Watcher W1-B White / AI Vision | 2024-09-12，众筹发布 | 412×412，SPD2010 QSPI | 独立于同名 Xiaozhi 聊天版。板级显示、输入及执行结果见 SenseCAP 专项报告；Himax、相机、SenseCraft 身份和全部网络功能不等同于已模拟。 |
| SenseCAP Indicator D1 | 至少于 2023-04-21 已发布 | 480×480，RGB | 原厂 GX 显示版本的 ROM 启动与 Wi-Fi 设置页已观察到；不将 D1S/D1Pro 的传感器或 D1L/D1Pro 的 LoRa 当作 D1 标配。 |

发布时间来自各知识记录的官方公告或文档。仅有月份或“最迟已发布”证据的型号不伪造精确日期。未做实体硬件刷写或电气验证。

## 3D 模型

48 个身份都有本地预览资产和可复现构建依据。第一阶段 13 款中，11 款使用官方 CAD，E1004 基于官方正反面照片和公布尺寸重建，TRMNL 裸屏套件使用明确标注的屏幕调试模型。新增四款 EE 裸板依据官方 PCB 的板边、孔位、厚度和元件坐标；25 个套件各自组合对应主控板与按官方尺寸图构建的屏幕调试面板，不虚构固定机壳、排线姿态或装配位置。六个 SenseCAP 商业版本只复用有来源的同壳几何，保留各自颜色、SKU 和未知附加内部结构。

照片重建、PCB 元件高度近似和屏幕调试面板不冒充制造用 CAD。没有内部电子结构证据的部分显示为 `UNKNOWN`；没有集成屏幕的裸板不放置测试图。共享来源文件保留可验证的原始与脱敏后摘要，每个产品仍有自己的模型定义与几何资产。

原厂 CAD 只做有记录的刚体变换，不缩放整个装配体来凑尺寸。保留原始组件编号、被省略的微小器件清单、坐标变换、尺寸对比、许可说明及源文件摘要。几何数据按产品按需加载，正常预览不下载原始 CAD 档案。Seeed 原始 CAD/PCB 在 Git 中保留，相同来源通过经过路径和摘要验证的明确引用复用；Pages 发布运行几何、纹理、模型定义和来源收据。已有两个其他品牌模型审核页的原始 STL 下载入口仍保留对应文件。发布前检查实际文件总量和模型依赖，避免超过站点容量。

[第一阶段模型报告](evidence/seeed-models/README.md)和 [扩展模型报告](evidence/seeed-expanded-models/README.md)包含正、背、斜向软件深度渲染、屏幕射线遮挡检查、逐件尺寸和源摘要。软件渲染使用测试图案，不能冒充固件截图或真实浏览器验收。只有有来源的实体按钮绑定交互；例如 Sticky 两枚尚未确定 Up/Down 顺序的侧键保留几何但不猜测 3D 按键身份。

## 原厂固件及加载链路

每台设备保留自己的原厂调查记录。固定原厂条目排在该机固件列表首位；TRMNL DIY 和 XIAO 7.5 的原始出厂批次版本或三页演示恢复镜像仍未知，因此明确选择已固定的 TRMNL 1.8.19 项目镜像作为替代默认，原厂条目的角色和未知信息不改变。

本批 Seeed 设备固件只记录远程地址、版本、布局和 SHA-256，不新增托管其 BIN 或镜像。调查时下载的镜像保存在临时验证目录；浏览器可以按 URL 与摘要持久缓存，并在读取时复核。本站既有公共 demo、ROM、Linux 和 Pocket 等固定依赖按原有发布规则保留。

- 三款新 XIAO IPS 原厂 ZIP 来自 [Display-Gadgets 固定提交](https://github.com/Seeed-Projects/Display-Gadgets/tree/ba7f69f223fb90b1766175afee4d26a045663a97)。完整布局为 bootloader `0x0`、分区表 `0x8000`、boot_app0 `0xe000`、应用 `0x10000`。
- E1001–E1004 固定 [fw-2026.11.20 发布](https://github.com/Seeed-Projects/OSHW-reTerminal-Series-E-D/releases/tag/fw-2026.11.20)中的 SenseCraft HMI 1.1.5，应用偏移是 **`0x90000`**。不得套用普通 Arduino 的 `0x10000`。
- Watcher 是 ESP32-S3 六段恢复布局，未覆盖设备独有的 `0x9000` 工厂身份分区。Himax 固件属于另一颗芯片。
- Indicator 的 ESP32-S3 完整镜像与 RP2040 UF2 分开；D1001 的 P4 完整镜像与 C6 镜像分开，不将不同芯片的二进制拼入一份 Flash。
- Indicator D1L / D1Pro 各有同名板型的独立清单，仍使用已核验的原官方 merged 镜像、`0x0` 偏移和原摘要；不能借相同 ESP32-S3 芯片绕过板型检查。Meshtastic 版固定 `2.5.11.8e2a3e5` 的三个 ZIP 成员：完整镜像 `0x0`、S3 BLE OTA `0x260000`、LittleFS `0x300000`；这是匹配文档版本的官方恢复来源，精确出厂批次仍未知。
- 两款 7.5-inch TRMNL 镜像按 [v1.8.19 官方发布脚本](https://github.com/usetrmnl/trmnl-firmware/blob/450f9ca2c690ef8bf7d9f5a219b71fd49301325d/.github/workflows/publish-release-firmware.yml)分别固定 S3 和 C3 构建。相同屏幕尺寸不产生固件兼容别名。

### ZIP 内精确成员

为保持原厂完整启动链，manifest 的每个 `flash_at` 段可以声明 `archive: {format: "zip-entry", entry: "精确根目录文件名.bin", sha256: "整个 ZIP 的 SHA-256"}`；段自身的 `sha256` 是提取后 BIN 的摘要。先校验容器，再核验成员与最终 Flash 布局；不是把 ZIP 中首个 BIN 当作整机固件。

实现限制 ZIP 容器为 64 MiB、成员不超过 64 个、声明总解压尺寸不超过 128 MiB；只解压精确选中的成员，成员仍受板型容量限制。拒绝路径穿越、重复名称、加密、ZIP64、符号链接及不一致的局部/中央头、CRC 或数据描述符。一次加载最多暂存一个已校验容器，Meshtastic 的三个段实测只读取一次 62,741,576 字节 ZIP。跨加载的成员缓存仍按 URL、精确成员与摘要区分，并重新核验成员内容。

硬件刷写规划也接受全部段都明确列在 `flash_at` 的完整清单。它仍验证 bootloader、分区表、应用、芯片、容量、重叠和擦除范围；写入原始分段，不把验证时补齐的空洞当作额外写入。单独 app、缺分区表或错误芯片不会被这个路径放行。本批仅验证刷写计划，未执行实机刷写。

## 关键数字协议边界

### Indicator SX1262

D1L、D1Pro 和 Meshtastic 版各使用自己的精确板名。SX1262 的 SCK41、MOSI48、MISO47 经真实 GPIO 矩阵连接；NSS、RESET、BUSY、DIO1 位于扩展器 P0–P3，扩展器中断为 GPIO42。扩展器寄存器读写、片选边沿、数字忙期限和中断确认共同驱动器件，不能用虚拟 Arduino GPIO64 当作 ESP32 实体引脚。D1 / D1S 没有增加无线器件。

实际原始 Meshtastic 镜像已通过无线初始化。最终 WASM 在约 8 秒注入的显式四字节数字测试包使接收计数从 0 变为 1，客体进入 RadioIf 处理，记录 `Region unset` 与 `Packet received (noise?)` 并清除无线 IRQ；这不证明应用接收成功或已经读取全部载荷。在独立的较长观察中，约 16 秒注入的同包仍到达器件并触发客体扩展器中断，但后续 RadioIf 处理未完成，IRQ 保持为 2。两种结果分别保留，没有发送包、组网或物理 RF 成功结论。D1L / D1Pro 的各自原厂 UI 与触摸已经分别实跑，但该原厂路径仍将无线器件保持在复位，不能继承 Meshtastic 的无线结果。来源和 GPIO 位序见 [Indicator LoRa 专项报告](seeed-indicator-lora-runtime.md)，最终两个时间窗口见 [原始固件执行收据](evidence/seeed-meshtastic-final-20261122/README.md)。

### RGB 扫描与帧边界

原始 Meshtastic 启动暴露了两处共享 S3 LCD 模型问题：DMA 欠供时未产生 VSYNC，客体无法进入自己的恢复 ISR；批量取数跨过一帧后又保留下一帧字节，与 ISR 的 34 字节预取补偿重复，造成水平漂移和奇数字节错色。修复按真实 LCD 时钟生成欠供扫描的同步中断，丢弃不完整扫描；正常取数在本帧边界停止，RGB565 仅消费完整像素，以有界宽整数保存未消费的时间额度。没有替客体选择描述符或补画面。

最终生产 WASM 的较长观察达到 20.36 客体秒，七次完整 RGBA 捕获完全相同，颜色为固定固件定义的绿色。方向与固件自身 `setRotation(2)` 一致，没有额外翻转。默认应用仍停在启动画面，未观察到退出 logo 或短按处理标记；显示稳定不代表整个 Meshtastic 应用已兼容。早先的黑屏、漂移及颜色错误证据均保留。修复有意纠正了旧的批量 RGB 时序，完整回归与主机 CPU 对比另见最终集成记录；协议依据与失败→成功回归见 [帧边界修复](evidence/seeed-lcd-vsync-20261122/boundary-review.md)。

### 小尺寸电子纸与独立板屏配置

新增 SSD1680/SSD1683、JD79676/JD79667 兼容命令和 UC8151D 的有界数字模型，以及 UC8179 的 648×480 配置。2.13-inch 面板的 122 可见列与 128 存储列分别处理；客体提供的分辨率不能重新分配任意大小显存。SSD1681/SSD1677 的供电丢失清除易失 RAM，但保留已显示的光学图像。

UC8151D 数据手册规定三字节 TRES、七字节局部窗口；固定 GFX2 版本发送的九字节局部窗口差异被保留，不在模拟器里改成另一颗芯片的协议。四色 JD 驱动名称与实际硅片型号有来源冲突，目录明确区分。电子纸 LUT、温度、残影和高压波形不在数字模型范围。EE04/EE05 的 4.26-inch Y 方向尚无原厂首帧校准证据，不能称作完整方向验收通过。

### UC8179 与供电

UC8179 的旧/新显存、局部窗口、刷新和 BUSY 有界实现。TRMNL 使用的 bb_epaper 会先写显存，再硬复位并初始化后刷新，因此 `reset()` 保留显存；只有板级供电丢失才清除易失 RAM。已有可见电子纸图像保留。此处不声称模拟 LUT、光学残影或温度相关刷新时长。

TRMNL 套件原理图把屏幕电源开关 EN 接至 GPIO43/U0TXD。板模型支持实际 GPIO 高电平和正确 UART0 TX 路由的空闲高电平，并明确不模拟串口位引起的电源纹波。E1001/E1002/E1004 的 ADC_EN21 与 TRMNL 的 ADC_EN6 分别控制各自电池分压；标称电压不是测量值，显式宿主电压或原始 ADC 注入继续优先。

### E1003 / IT8951

协议依据 [Seeed_GFX2 固定驱动](https://github.com/Seeed-Studio/Seeed_GFX2/tree/556df7cb0b4a097d9b81026a26fd5511f9c934d6/src/driver/epaper)、原厂依赖的 [Seeed_GFX Tcon.cpp](https://github.com/Seeed-Studio/Seeed_GFX/blob/24c5094cbde79933e93565ee68119e214c48fb36/Extensions/Tcon.cpp)与 ITE 原始 [IT8951 For D v0.2.4.3 数据手册镜像](https://download.kamami.pl/p586054-IT8951_D_V0.2.4.3_20170728.pdf)。文档的 SPI 时序和 Fig 7-17 分别给出读 dummy word 与打包像素位序。

`0x6000`、`0x0000`、`0x1000` 分别是命令、数据和读取前导。实际 `transfer16` 是全双工，发送读取前导的时钟不能提前消耗之后的 dummy 或返回值。little-endian 像素位于 SPI 字的低位，不能额外交换两个字节，否则单色图会出现每 8 像素的切片。BGVR 高字节是背景、低字节是前景；bit 1 选择前景。

模型覆盖关键寄存器、4/8bpp 装载、1bpp 位图、区域更新与 BUSY。缓冲区地址、温度、VCOM 和刷新时间明确为合成数字值。板层按实际原生 X 方向与安装方向映射至目录的竖屏，不根据截图动态旋转。官方低电量页面使用默认横向 rotation 0，在竖向模型中侧向显示是该固件与安装方向的组合。

### 存储和触摸

所有声明 TF/microSD 的设备默认准备 64 MiB FAT32 卡；只有实现真实 SD 传输的板型才挂给客体。五款 reTerminal 的 SPI TF 分别检查自己的供电、片选、MISO 和总线冲突。XIAO 1.47 的卡镜像准备不代表该板已模拟 SD 传输。

物理 `display.touch` 与可用 `runtime.touch` 分开。显式 `runtime.touch: false` 优先于硬件规格，避免向未实现的控制器发送伪触摸。Sticky 的 GT911 选择电源/复位绑带对应的 0x14 或 0x5d 地址；E1003 的 GT911 仍未实现。

## 复现与证据

- [XIAO IPS 专项报告](seeed-xiao-ips-runtime.md)：三机原厂启动、背光变化/关闭、第三键和触摸/释放共 12 个 native 场景；MMIO 测试同时验证错误引脚不会产生有效输入。
- [7.5-inch 专项报告](seeed-75-runtime.md)：S3 原始 TRMNL 配网画面、C3 轻睡眠阻塞、供电原理图与显存复位差异。
- [D1001 阻塞报告](seeed-d1001-blockers.md)：实际观察到的第一阻塞与从源码推导的后续 DSI、DMA、I2C 缺口分开记录。
- [Watcher XiaoZhi 专项报告](seeed-xiaozhi-runtime.md)：独立官方 2.0.4 恢复镜像、真实音量回调、短按应用状态，以及 2 ms/20 ms 编码器观察。
- [十二机共享 WASM 检查点](seeed-final-wasm-runtime.md)：`1002b80d…` 产物上的 12 台原有设备、16 个输入、29 次捕获，分别保留与历史相同的 26 个真实帧和 3 个 `NO_FRAME`；后续产物以最终集成记录为准。
- [最终生产 WASM 的十二机回归](evidence/seeed-publish-wasm-20261122/README.md)：`a3f0c90c…` 产物重新 ROM 启动，26 个实际 RGBA 均与上述检查点逐字节相同，3 个 `NO_FRAME` 也保留；没有把白屏或未完成流程升级为通过。
- [最终生产 WASM 的 D1L、D1Pro 与 XiaoZhi](evidence/seeed-publish-extra-products-20261122/README.md)：各商品独立启动和输入；XiaoZhi 启动标题的有限像素差异与后续三帧完全相同分别记录。
- [最终生产 WASM 的 D1S 与 Watcher W1-A](evidence/seeed-publish-remaining-skus-20261122/README.md)：分别使用各自商品 ID 独立 ROM 启动。D1S 触摸打开密码键盘；W1-A 输入已接受但画面未改变，不继承其他版本的导航结论；底层官方家族恢复来源如实共享。
- [最终 Meshtastic 两个接收窗口](evidence/seeed-meshtastic-final-20261122/README.md)：九份完整画面稳定一致；保留早期 RadioIf 处理与晚期 IRQ 未清除的差别，状态仍为 `partial`。
- [Indicator LoRa 专项报告](seeed-indicator-lora-runtime.md)：三个独立板型、真实扩展器/SPI/IRQ 路径、数字无线边界与实际 RGB GPIO 位序。
- [SenseCAP 专项报告](seeed-sensecap-runtime.md)：Watcher QSPI、扩展器及输入线路，Indicator GX 显示和触摸，以及未实现的相机、身份与音频范围。
- [第一阶段七机 WASM 验证](seeed-wasm-runtime.md)：7 次 ROM 启动、16 个输入动作、24 帧真实捕获；保留成功与未改变画面的输入结果。
- [第一阶段五款 reTerminal 观察](seeed-reterminal-runtime.md)：实际默认 WASM、独立 native 低电量条件及五机 TF 挂载，明确区分各自结论。
- [前端与 WASM 接口回归收据](evidence/seeed-node-gates/receipt.json)：全平台合同、真实录制、StopWatch、worker、触摸、timing 和 BLE；唯一新增翻译失败已定向修复并复测。
- [最终生产 WASM 的 13 组网页回归](evidence/seeed-publish-node-gates/receipt.json)：438 项平台测试及全部列出的 ABI、worker、真实录制和既有 demo 检查通过。
- [RGB 时序引起的四份 panel_sid golden 审查](evidence/seeed-panel-golden-20261122/README.md)：完整画面相同、音频精确提前一个采样，控制台与计数变化逐项保留；有意更新后定向测试通过。
- [第一阶段集成检查](evidence/seeed-integration-20261122/README.md)：2,274 项 native release 测试、Clippy、目录与来源校验，以及发布和浏览器验证的实际状态。
- [最终集成记录](evidence/seeed-final-integration-20261122/README.md)：后续源码与产物、独立检查点、完整平台回归、发布目录容量和当前交付状态。
- [最终主机 CPU 对照](evidence/seeed-performance-final-20261122/README.md)：30 次交替基线/候选运行的工作量、输出和逐来源 IRQ 完全一致；所有样本、正负增幅与较早负结果保留，不宣称零成本或提速。
- `tools/probe-seeed-runtime.mjs` 使用实际平台 WASM worker 加载目录默认固件，并记录真实 RGBA、指令数、固件摘要和输出。`NO_FRAME` 是失败观察，不用静态图片填补。
- [模型审核](evidence/seeed-models/README.md)单独记录几何、来源、可复现构建与屏幕可见性。

```sh
tools/fetch-rom-elfs.sh target/roms
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy --release --target wasm32-unknown-unknown -p esp32sim-wasm --features jit-tests -- -D warnings
ESP32SIM_ROM_DIR=target/roms cargo test --release --workspace -- --include-ignored --skip external_
tools/wasm-build.sh
ESP32SIM_ROM_DIR=target/roms node --test tools/platform-*-test.mjs
ESP32SIM_ROM_DIR=target/roms node tools/platform-recording-test.mjs --real
ESP32SIM_ROM_DIR=target/roms node tools/wasm-test.mjs demo-stopwatch
python tools/validate-platform-catalog.py
node tools/platform-knowledge-test.mjs
node tools/platform-seeed-catalog-test.mjs
python docs/platform/evidence/seeed-models/verify-models.py --catalog web/platform/catalog/products.json
node tools/check-evidence-privacy.mjs docs/evidence 'docs/platform/evidence/seeed*'
```

本地 Chromium 无法创建其所需 socket，云浏览器也不能访问工作区回环地址，因此本地软件渲染和 Node WASM 检查不写成浏览器通过。实际浏览器验收在部署后的公开 HTTPS 页面上单独进行，部署与统一检查结果随集成证据记录。
