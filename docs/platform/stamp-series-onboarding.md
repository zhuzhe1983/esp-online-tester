# Stamp 系列适配清单

本轮按用户确认的范围完成 **22 个 Stamp 主控和套件**的独立目录、模型与受限板级运行适配。StampFly 与 v1.1 分别建档；专用扩展板不在实施范围内。下表是集成结果；文末保留 `49a27787` 的原始盘点，描述的是修正前状态。

厂家 CAD 的原始三角形和装配来源保留，裸露 PCB 按对应 SKU 照片校正颜色和贴图，未知层保留 `UNKNOWN`。缺失紧固件按可核验来源补全，隐藏长度、照片推导的几何及标准件装配估算均写明。纸贴按实际承载面、孔和高度裁切；真实纸张跨孔与错误贴图悬空分别记录。相关经验已写入[导入技能](../../skills/esp-product-onboarding/SKILL.md)及[模型验证规则](../../skills/esp-product-onboarding/references/model-verification.md)。

所有条目仍如实保留 `partial` 支持范围。模型完成不代表所有官方固件兼容，模拟器测试也不等同实机验证。原厂镜像留在发布方；源码构建的测试程序与正式固件目录分开。

| 产品 | 板型与运行证据 | 已验证行为及主要限制 | 模型证据 |
| --- | --- | --- | --- |
| StampFly · `m5stack-k138` | [`stampfly`](stampfly-runtime.md) | 原厂 ROM 启动及 G0 软件复位；BMI270、INA3221、双 ToF 初始化、四路 PWM；无飞行动力学或遥控射频 | [尺寸、照片与资产 hash](evidence/m5stack-k138/verification.json) |
| StampFly v1.1 · `m5stack-k138-v11` | [`stampfly-v11`](stampfly-v11-onboarding.md) | 独立硬件版本与原厂镜像；包含 S3A G38 门控；同样不宣称飞行能力 | [尺寸、照片与资产 hash](evidence/m5stack-k138-v11/verification.json) |
| Stamp-S3 · S007 · `m5stack-s007` | [`stamps3`](stamps3-runtime.md) | G0 按键与 G21 RGB；真实指针驱动固件输出；UIFlow2 v2.5.4 保留 trial | [尺寸、照片与资产 hash](evidence/m5stack-s007/verification.json) |
| Stamp-S3 PIN1.27 · `m5stack-s007-pin127` | [`stamps3`](stamps3-runtime.md) | G0 按键与 G21 RGB；真实指针驱动固件输出；UIFlow2 v2.5.4 保留 trial | [尺寸、照片与资产 hash](evidence/m5stack-s007-pin127/verification.json) |
| Stamp-S3 PIN2.54 · S007-PIN254 · `m5stack-s007-pin254` | [`stamps3`](stamps3-runtime.md) | G0 按键与 G21 RGB；真实指针驱动固件输出；UIFlow2 v2.5.4 保留 trial | [尺寸、照片与资产 hash](evidence/m5stack-s007-pin254/verification.json) |
| Stamp-S3A · S007-V033 · `m5stack-s007-v033` | [`stamps3a`](stamps3a-runtime.md) | G0、G21 RGB 与 G38 供电门控；三种排针装配分别验收 | [尺寸、照片与资产 hash](evidence/m5stack-s007-v033/verification.json) |
| Stamp-S3A PIN1.27 · S007-PIN127-V033 · `m5stack-s007-pin127-v033` | [`stamps3a`](stamps3a-runtime.md) | G0、G21 RGB 与 G38 供电门控；三种排针装配分别验收 | [尺寸、照片与资产 hash](evidence/m5stack-s007-pin127-v033/verification.json) |
| Stamp-S3A PIN2.54 · `m5stack-s007-pin254-v033` | [`stamps3a`](stamps3a-runtime.md) | G0、G21 RGB 与 G38 供电门控；三种排针装配分别验收 | [尺寸、照片与资产 hash](evidence/m5stack-s007-pin254-v033/verification.json) |
| Stamp-S3Bat · S015 · `m5stack-s015` | [`stamps3bat`](stamps3bat-runtime.md) | 8 MB OPI PSRAM、M5PM1 按键读取与 RGB 刷新；不模拟真实电源关机 | [尺寸、照片与资产 hash](evidence/m5stack-s015/verification.json) |
| Stamp-S3Bat DIP · `m5stack-s015-dip` | [`stamps3bat`](stamps3bat-runtime.md) | 8 MB OPI PSRAM、M5PM1 按键读取与 RGB 刷新；不模拟真实电源关机 | [尺寸、照片与资产 hash](evidence/m5stack-s015-dip/verification.json) |
| Stamp-C3 · `m5stack-c056-b` | [`stampc3`](stampc3-runtime.md) | G3 按键、G2 RGB；无 CH9102 / EN 电气时序 | [尺寸、照片与资产 hash](evidence/m5stack-c056-b/verification.json) |
| Stamp-C3 Mate · K056 · `m5stack-k056` | [`stampc3`](stampc3-runtime.md) | G3 按键、G2 RGB；无 CH9102 / EN 电气时序 | [尺寸、照片与资产 hash](evidence/m5stack-k056/verification.json) |
| Stamp-C3U · `m5stack-c122-b` | [`stampc3u`](stampc3-runtime.md) | G9 按键、G2 RGB；无原生 USB 电气行为 | [尺寸、照片与资产 hash](evidence/m5stack-c122-b/verification.json) |
| Stamp-C3U Mate · `m5stack-k122` | [`stampc3u`](stampc3-runtime.md) | G9 按键、G2 RGB；无原生 USB 电气行为 | [尺寸、照片与资产 hash](evidence/m5stack-k122/verification.json) |
| Stamp-Pico · C050-B · `m5stack-c050-b` | [`stamppico`](stamppico-runtime.md) | G39 按键、G27 RGB、PICO-D4 身份；UIFlow2 缺 M5Things 的失败结果保留 | [尺寸、照片与资产 hash](evidence/m5stack-c050-b/verification.json) |
| Stamp-Pico Mate · `m5stack-k051` | [`stamppico`](stamppico-runtime.md) | G39 按键、G27 RGB、PICO-D4 身份；UIFlow2 缺 M5Things 的失败结果保留 | [尺寸、照片与资产 hash](evidence/m5stack-k051/verification.json) |
| Stamp-Pico DIY Kit · K051-B · `m5stack-k051-b` | [`stamppico`](stamppico-runtime.md) | G39 按键、G27 RGB、PICO-D4 身份；UIFlow2 缺 M5Things 的失败结果保留 | [尺寸、照片与资产 hash](evidence/m5stack-k051-b/verification.json) |
| Stamp-C6LoRa · `m5stack-s012` | [`stamp-c6lora`](stamp-c6lora-runtime.md) | PI4IO / SPI / SX1262 数字协议与明确注入的对端报文；无 RF 传播模拟 | [尺寸、照片与资产 hash](evidence/m5stack-s012/verification.json) |
| Stamp-P4 · `m5stack-s013` | [`stamp-p4`](stamp-p4-runtime.md) | 16 MB Flash / 32 MB PSRAM，实际 IDF 6.0.2 测试镜像；不含 C6 SDIO 扩展 | [尺寸、照片与资产 hash](evidence/m5stack-s013/verification.json) |
| Stamp-C5 · `m5stack-s016` | [`stamp-c5`](stamp-c5-runtime.md) | 独立 C5 内核，仅显式 SRAM app-direct；UART0 与 G28 蓝灯；不支持原厂 ROM / 通用 IDF 启动 | [尺寸、照片与资产 hash](evidence/m5stack-s016/verification.json) |
| Stamp-C5 DIP · `m5stack-s016-dip` | [`stamp-c5-dip`](stamp-c5-runtime.md) | 同一受限 C5 运行范围；独立 FPC / 散装排针装配与浏览器验收 | [尺寸、照片与资产 hash](evidence/m5stack-s016-dip/verification.json) |
| StamPLC · K141 · `m5stack-k141` | [`stamplc`](stamplc-runtime.md) | 原厂 v0.2 Dashboard / Launcher / LogMonitor，按键、LCD、继电器 GPIO；SD / RTC / 温度等仍有限制 | [尺寸、照片与资产 hash](evidence/m5stack-k141/verification.json) |

最终联合 WASM 为 `bd1c688cff2cbd2ac608761dbe122d0c2346f65ea3d0a6e53d3034ee170ba3ef`（5,000,313 字节）。[两款 StampFly 最终回归](evidence/stampfly-runtime/final-regression/summary.json)记录同一 worker 内 G0 触发软件复位后的新输出；[C5 浏览器回执](evidence/stamp-c5-runtime/platform-browser.json)记录本地上传和 iframe RPC，[独立 C5 复核](evidence/stamp-c5-runtime/independent-review.json)记录边界与负例。各型号更早的指针、照片及固件回执保留原测试 hash；仅说明文字变化的延续关系有独立差异证明，不把旧回执重标为新测试。

尺寸与照片的偏差没有被改成通过：StampFly 的源装配外形与官方尺寸冲突、S3 的 USB 外伸范围、部分排针版本的拍摄角度差异仍有记录。StamPLC 的正反面对照 IoU 未达到 1:1 阈值，属于保留偏差的模型；S3A 的实体尺寸另行排除非实体贴图与交互标记，旧显示边界也继续保留。

[最终集成检查](evidence/stamp-series-final/checks.json)记录 Rust、平台、WASM 与 CI 等价命令及结果；[独立审查](evidence/stamp-series-final/independent-review.json)覆盖 22 款实际加载、13 款拆分/精确复位和两款窄屏嵌入页；性能比较另存回执。

## 原始盘点记录（修正前）

以下内容固定描述基线 `49a27787`，其中“未接板型”“待新增”和“芯片后端缺失”等不是当前状态；当前结果以本页上表和对应型号的运行、模型回执为准。

本清单记录 `49a27787` 的基线，核对日期为 2026-11-04。初始范围是目录中 **18 个 `m5-stamp` 条目，加 StamPLC K141，再新增 StampFly v1.1 K138-V11，共 20 个产品**。核对当前官方索引后，又确认缺少 **Stamp-S3A PIN1.27 S007-PIN127-V033** 和 **Stamp-C5 DIP S016-DIP**，因此主队列为 **22 个产品**；Stamp 专用扩展另见下表，不能把现有目录当作官方全集。K141 当前归在 `m5-other`，这里纳入工作范围，不据名称自动改动目录分类。每个 SKU 保留独立产品身份，不因芯片或盖板相似而继承运行验证。用户已确认本轮只处理这 22 个 Stamp 主控与套件；专用扩展板仅保留盘点，不进入实施队列。

本轮盘点读取了目录、19 份模型计划与历史验收资料，检查了现存来源文件的 SHA-256，并目视复核了各类产品的已有照片对照。没有重新渲染浏览器、运行固件或测量实物；下列缺陷与待查项是后续逐台验收的输入。[结构化回执](evidence/stamp-series-audit-2026-11-04/receipt.json) 保存输入哈希、源文件可用性、旧验收状态与当前目录状态。

## 基线：顺序和产品边界

先完成原版 StampFly，并把 v1.1 作为单独型号验收；之后按下表顺序逐台推进。模型、资料与板级实现可以并行准备，最终验收仍逐产品记录。表中“未接板型”表示 `runtime.board=null`，不代表对应芯片完全没有模拟器；“trial”也不代表固件已运行。

| 顺序 | 产品 / ID | 芯片；当前板型 | 当前固件目录 | 模型重点 |
| --- | --- | --- | --- | --- |
| 1 | StampFly K138 · `m5stack-k138` | ESP32-S3；未接板型 | 出厂 v1.1.0，trial | 当前只有原厂电池保护小件和粗略机架；升级整机分件，核对机架 PCB、桨、电机、螺丝和支撑件 |
| 2 | StampFly v1.1 K138-V11 · `m5stack-k138-v11`（待新增） | ESP32-S3 / Stamp-S3A；待独立定义 | 待按 v1.1 来源映射 | 核对 S3A、320 mAh 电池和实际尺寸；不直接把原版网格改名为精确 v1.1 |
| 3 | Stamp-S3 S007 · `m5stack-s007` | ESP32-S3；`stamps3` partial | UIFlow2 v2.5.4，trial；官方 LED 源码 reference | 保留已有机械 CAD；裸板照片、正确板色、背面丝印与 UNKNOWN 面 |
| 4 | Stamp-S3 PIN1.27 · `m5stack-s007-pin127` | ESP32-S3；未接板型 | UIFlow2 教程 reference | 裸露背板灰绿占位，缺真实板面；排针、孔与紧固件逐项核对 |
| 5 | Stamp-S3 PIN2.54 · `m5stack-s007-pin254` | ESP32-S3；未接板型 | 无条目 | 保留已有排针；补背板外观，核对排针节距/数量和 USB 开口 |
| 6 | Stamp-S3A S007-V033 · `m5stack-s007-v033` | ESP32-S3；`stamps3a` partial | UIFlow2 v2.5.4，trial | 仅盖板 CAD 与派生 USB/天线；补裸板外观、中心紧固件和按键版本细节 |
| 7 | Stamp-S3A PIN1.27 · `m5stack-s007-pin127-v033`（待新增） | ESP32-S3FN8，8MB；待独立定义 | 待调查；官方提供 UIFlow2 / Arduino 教程 | 1.27mm 预焊排针，S3A RGB 供电差异；结构页链接 S3 STEP，须检查变体缺件 |
| 8 | Stamp-S3A PIN2.54 · `m5stack-s007-pin254-v033` | ESP32-S3；未接板型 | UIFlow2 教程 reference | 盖板孔内缺中心螺丝，USB/天线近景粗略；排针与背面 PCB 独立核验 |
| 9 | Stamp-S3Bat S015 · `m5stack-s015` | ESP32-S3；未接板型 | UIFlow2 v2.5.4，trial | 盖板与整机尺寸不同；补裸露 PCB 色/贴图，核对正面纸贴承载和电池插口 |
| 10 | Stamp-S3Bat DIP · `m5stack-s015-dip` | ESP32-S3；未接板型 | UIFlow2 v2.5.4，trial | 已有估算 DIP 排针；纸贴缺失，UNKNOWN 从盖板镂空处露出；核对电池插座和实际高度 |
| 11 | Stamp-C3 C056-B · `m5stack-c056-b` | ESP32-C3；未接板型 | 无条目 | 有隔热贴与 M2 螺丝头；检查贴纸边缘/孔、背板外观和螺柱，勿重复加螺丝头 |
| 12 | Stamp-C3 Mate K056 · `m5stack-k056` | ESP32-C3；未接板型 | 无条目 | 同上，保持 Mate 套件身份；未安装的散装接头不能凭包装清单强行装上 |
| 13 | Stamp-C3U C122-B · `m5stack-c122-b` | ESP32-C3；未接板型 | 无条目 | 白贴纸已挖中心孔，但模型缺螺丝；背板、USB 周边与贴纸边界需修 |
| 14 | Stamp-C3U Mate K122 · `m5stack-k122` | ESP32-C3；未接板型 | 无条目 | 同样缺中心螺丝和裸露背板外观；旧轮廓 pass 不代表这些细节通过 |
| 15 | Stamp-Pico C050-B · `m5stack-c050-b` | ESP32；未接板型 | UIFlow2 v2.5.4 / UIFlow1 v1.15.5，trial | 纸贴有效区铺满矩形，需按盖板缺口校正；补中心紧固件与裸板外观 |
| 16 | Stamp-Pico Mate K051 · `m5stack-k051` | ESP32；未接板型 | UIFlow1 / UIFlow2 教程 reference | 中心螺丝和背板细节缺失，盖板厚度与整机厚度须分开；保留套件接头身份 |
| 17 | Stamp-Pico DIY Kit K051-B · `m5stack-k051-b` | ESP32；未接板型 | 无条目 | 纸贴矩形边界和孔位需修，补紧固件与裸板；下载器/套件散件单独记录 |
| 18 | Stamp-C6LoRa S012 · `m5stack-s012` | ESP32-C6；未接板型 | 无条目 | 已有真实 PCB/屏蔽盖 CAD；补外露丝印，不换占位板；辨识被排除的离体圆柱 |
| 19 | Stamp-P4 S013 · `m5stack-s013` | ESP32-P4；未接板型 | UIFlow2 v2.5.4，trial | 已有板级 CAD，但绿色板面与近黑色产品照不同；补背面丝印及有依据的孔内紧固件 |
| 20 | Stamp-C5 S016 · `m5stack-s016` | ESP32-C5；芯片后端缺失 | UIFlow2 v2.5.4，trial | 已有红色板级 CAD，RF 屏蔽盖缺失；补背面外观并核对 USB 与屏蔽盖装配 |
| 21 | Stamp-C5 DIP S016-DIP · `m5stack-s016-dip`（待新增） | ESP32-C5HF4，4MB；芯片后端缺失 | 待调查；官方 Arduino 教程 | 与 C5 不同：已焊 FPC 座、随附 2×2.54-7P 排针；核对成品装配、PCB 版本与共用 CAD |
| 22 | StamPLC K141 · `m5stack-k141` | ESP32-S3；未接板型 | 出厂 v0.2、UIFlow2 v2.5.4，trial | Stamp 纸贴承载边界、接线端螺丝与壳体紧固件；内部 PCB 未知层保持独立 |

官方出厂调查见 [2026-10-27 审计](research/factory-firmware-audit-2026-10-27.json)。基线中只有 K138 与 K141 固定了有出厂用途依据的镜像，其余 17 个现有型号为 `not-located`，不能用 UIFlow 或 LED 示例替代这个结论。表中版本是本次目录快照，不是对上游最新版本的再次确认。当前没有这些产品的精确小智固件目录映射；每台开始实施时仍须按技能重新查官方出厂、UIFlow 与小智来源。

## 官方索引发现的目录缺项

当前[官方产品索引](https://docs.m5stack.com/en/products)和[发布记录](https://docs.m5stack.com/en/history)均已重新读取。发布记录已包含 2026-10-30 的 Stamp Timer Power2；旧的 `product_release_timeline` 页面截至 2025 年，不能单独用来证明当前全集。官网比较器的服务端 HTML 没有产品行，本次没有将它的空结果当作缺货或缺项依据。

两个缺失主控已加入上方顺序：官方 [S3A PIN1.27](https://docs.m5stack.com/en/core/Stamp-S3A_PIN1.27)给出 SKU `S007-PIN127-V033`，与原版 `S007-PIN127` 分开；[C5 DIP](https://docs.m5stack.com/en/core/Stamp-C5_DIP)给出 `S016-DIP`，不能沿用裸焊盘 C5 的连接器装配结论。

下列 **22 个扩展/套件 SKU（20 个官方页面）**均未出现在基线 `products.json` 或 `accessories.json` 中。用户已明确将它们排除在本轮实施范围之外；本次只保留盘点，没有创建资产或宣称支持。尤其 A172 内含 ESP32-C6，官网将其列在 Controllers，但用途仍是 Stamp-P4 的 SDIO 扩展，不能拿通用 C6 启动测试代替组合运行。

| 官方产品 / 独立 SKU | 角色与所需行为 | 模型与来源核查重点 |
| --- | --- | --- |
| [Stamp-AddOn C6 For P4](https://docs.m5stack.com/en/stamp/Stamp-AddOn_C6_For_P4) · **A172** | ESP32-C6-MINI-1-N4，4MB；与 P4 的 SDIO 通讯，需单独固件映射 | 官方列 M1.2×3 mm 帽头螺丝；BTB 插合、天线及 PCB 必须检查 |
| [Stamp IO](https://docs.m5stack.com/en/stamp/Stamp_IO) · **S002** | STM32F030 的 I²C IO/ADC/PWM/RGB 扩展 | 盖板中心螺丝、裸板、可选 Grove 接头；不新增虚假的 ESP32 固件 |
| [Stamp PWR485](https://docs.m5stack.com/en/stamp/Stamp_PWR485) · **S001** | RS485 收发与供电，不是独立 ESP 主控 | 裸板、端子及盖板装配；声明串行电气时序边界 |
| [Stamp ISP](https://docs.m5stack.com/en/stamp/Stamp_ISP) · **S006** | CH9102F USB-UART 下载器 | Type-C、指示灯、直角/直排针属于可选装配，勿同时强制装上 |
| [Stamp CatM](https://docs.m5stack.com/en/stamp/Stamp_CatM) · **S003** | SIM7080G UART AT 模组 | 天线接口、SIM 槽、盖板和裸 PCB；命令响应不等于蜂窝射频仿真 |
| [Stamp Cat1](https://docs.m5stack.com/en/stamp/Stamp_Cat1) · **S004** | SIM-A7680C UART AT 模组 | SMA 接口、SIM 槽与 PCB；保持 Cat1 与 CatM 独立 |
| [Stamp Timer Power](https://docs.m5stack.com/en/stamp/Stamp_Timer_Power) · **S005** | BM8563 定时唤醒、充电、升降压供电 | 电池接口、盖板/裸板、各电源域；需电源状态行为 |
| [Stamp Timer Power2](https://docs.m5stack.com/en/stamp/Stamp_Timer_Power2) · **S021** | M5PM1 电源管理、I²C、GPIO/ADC/PWM | 官方有 PcbDoc、KiCad footprint/STEP；与 S005 完全分开，保留真实板级 CAD |
| [Stamp LoRa-1262](https://docs.m5stack.com/en/stamp/Stamp_LoRa-1262) · **S014**、**S014-I**、**S014-IF** | SX1262 SPI 模块，三个独立连接器变体 | IPEX/FPC 是否预装；I/IF 随附天线延长线、垫圈和螺母，按实际装配建模 |
| [Stamp UWB](https://docs.m5stack.com/en/stamp/Stamp_UWB) · **S017** | QM33120W SPI 模块 | 官方 PCB/STEP；裸板、天线区、焊盘，不套用 S3 FPC 引脚 |
| [Stamp UWB F](https://docs.m5stack.com/en/stamp/Stamp_UWB_F) · **S017-F** | 同类 UWB 独立 FPC 变体 | 预焊 FPC 座和随附排线需独立核对，不简单改名 |
| [Stamp-AddOn Cam0308](https://docs.m5stack.com/en/stamp/Stamp-AddOn_Cam0308) · **A178** | GC0308，DVP/SCCB，与 S3Bat 组合 | 镜头、L 形 FPC、24P BTB、侧安装孔；来源只有照片/尺寸时保持派生范围 |
| [Stamp-AddOn Cam3660](https://docs.m5stack.com/en/stamp/Stamp-AddOn_Cam3660) · **A182** | OV3660，DVP/SCCB，与 S3Bat 组合 | 单独相机外形及寄存器，不用 A178 镜头尺寸代替 |
| [Stamp-S3 BreakOut](https://docs.m5stack.com/en/stamp/Stamp-S3_BreakOut) · **A129** | 被动引脚转换、Grove、G0/EN 按键 | 插座/排针、固定孔、PCB；不把单独购买的 S3 画成标配 |
| [Stamp-S3 GroveBreakOut](https://docs.m5stack.com/en/accessory/Stamp-S3_GroveBreakOut) · **A144** | 六个 Grove 端口和电池供电 | 电池架、电池接口、开关和主板独立装配 |
| [1.27-Socket Kit for Stamp-S3](https://docs.m5stack.com/en/accessory/1.27-Socket_Kit_for_Stamp-S3) · **A138** | 无固件的插座套件 | 精确节距、排数和高度，套装数量不等于装配数量 |
| [StampFly Accessory Kit](https://docs.m5stack.com/en/accessory/StampFly_Accessory_Kit) · **A147** | 维修备件，无独立固件 | 原厂电机/桨/紧固件与 Fly 实际装配核对，作为缺件来源 |
| [StamPLC AC](https://docs.m5stack.com/en/stamplc/StamPLC_AC) · **A160** | I²C IO 扩展控制继电器/RGB 与供电 | 有官方结构文件；接线端、紧固件和内部未知板面分开 |
| [StamPLC IO](https://docs.m5stack.com/en/stamplc/StamPLC_IO) · **A176** | STM32G030 I²C 隔离 IO/采集/继电器 | 独立壳体与端子；官方有内部固件，但非 ESP32 主固件 |
| [StamPLC PoE](https://docs.m5stack.com/en/stamplc/StamPLC_PoE) · **A165** | W5500 SPI 网络与 PoE 供电 | RJ45、插接与壳体；不能仅以网络 API 代替片上 SPI 验证 |

[官方索引回执](evidence/stamp-series-audit-2026-11-04/official-inventory.json)保存上述页面的抓取时间、字节数、SHA-256、精确 SKU 和页面内公开 CAD 链接，不保存整页副本。这里的“官方有 CAD 链接”仅证明链接存在，尚未下载或确认装配完整性。Cardputer、Capsule、DinMeter 等使用 Stamp 主控的整机仍属于它们各自的产品系列；本清单不因内部使用 Stamp 就自动把所有这些整机加入 Stamp 专用扩展队列。

扩展实施建议接在相关主控验收之后：S3/S3A → A129/A144/A138；S3Bat → A178/A182；P4 → A172；K141 → A160/A176/A165；其余接口模块按电源、IO、串行、无线顺序推进。无线模块的协议与客体响应应单独验收，展示距离、信号强度或动画不是物理射频的实测证据。

## 基线：裸露 PCB、紧固件和贴纸的问题

- **裸板占位集中在 S3/S3A、S3Bat、C3/C3U、Pico。** 多数既有模型用统一灰绿底和双面 UNKNOWN，未利用对应型号的外露板面照片。按新规则补可见面纹理和正确底色，另一未知面保留 UNKNOWN；照片无法证明元件高度、层叠或电气布局。原厂板级 CAD 已知的部分继续保留。
- **S3 PIN1.27、S3A 及 C3U 的已有背面对照清楚显示问题。** 产品照是带焊盘与丝印的近黑 PCB，模型却是空白灰绿面。需在相应 SKU 的原图上校正、遮罩，不能借同系列另一版本的丝印。
- **C3U 两款和 Pico 系列的中心紧固件需要补查。** C3U 的纸贴已明确排除金属螺丝区域，但模型列表没有对应螺丝，现有对照孔内是占位色。Pico 和 S3A PIN2.54 也需先确认头型、孔径与螺柱长度，再补几何。C3 两款已有 `cover-m2-head`，应核对而非重复添加。
- **Pico 纸贴需要承载面级别的裁切。** K051-B 与 C050-B 的有效 alpha 包围盒都是完整 18×24 mm 矩形；这只与盖板总包围盒近似一致，不能保证落在有缺口和起伏的真实承载面内。已有斜拍照片与正向模型的对照相机不一致，应重新校准视角后检查纸边、通孔和侧视悬空，不能用旧 IoU 直接判定修复量。
- **StamPLC 的 Stamp 纸贴存在明确的承载边界风险。** `stamp-printed-paper` 有效 alpha 投影为 x=[10.136,33.603]、y=[2.924,20.888] mm；对应 `stamp-cover` CAD 的 XY 范围为 x=[9.800,33.800]、y=[4.000,22.000] mm。纸贴下边超出该盖板约 **1.076 mm**。这是对实际 PNG alpha 与模型参数的筛查结果，需进一步检查局部承载面与相机，不能把大外壳包围盒当成这张贴纸的承载区域。
- **C5/P4/C6LoRa 不适用空白板替换。** C5 来自 KiCad footprint 内嵌 STEP，已含真实板级器件与 USB 开放曲面；缺 RF 屏蔽盖。P4 含 213 个实体及 3 个独立 USB 曲面，当前 `pcb-main` 色为 `#247832`，和已有产品照片明显不符。C6LoRa 已有 PCB、元件与屏蔽盖，但印字没有还原；源中索引 384 的离体圆柱被有理由排除，先辨识其身份，不凭圆柱形状将它当作螺柱移入。
- **S3Bat 两款必须把盖板尺寸与整机尺寸分开。** DIP 的估算排针与插座不能使厂家盖板自身的宽高/厚度自动变成整机验证通过。S015-DIP 当前无纸贴，镂空处可见 UNKNOWN；先确定可见板面和开孔，再决定纹理层与未知标识所在面。

以上基于保留的对照图、模型计划和几何元数据；未给出实物确认或精确制造公差。所有新增螺丝、螺柱和纸贴承载面应有来源与局部坐标，无法确认的隐藏长度保留为估算。

## 基线：运行适配的起点

[Stamp-S3](stamps3-runtime.md) 与 [Stamp-S3A](stamps3a-runtime.md) 已有部分板型：G0 按键、G21 RMT RGB；S3A 另有 G38 数字供电门控。已有官方 UIFlow2 **v2.5.3** ROM 启动蓝灯及内部 fixture 的输入/输出证据。当前目录已更新到 **v2.5.4**，仍为 `trial / simulationVerified:false`，新版本不能继承旧版结果；PIN 版本也不能不核对就继承基础板型。

仓库包含 ESP32、S3、C3、C6、P4 的芯片实现及 WASM 分派，Stamp 对应板型仍须逐一接入。**没有 ESP32-C5 芯片后端**；Stamp-C5 的模型/资料修正可以先完成，运行适配需要独立芯片工作，不能把 C5 镜像改标为 C3/C6 试跑。P4 已有芯片后端，所以旧模型说明中的“P4 运行内核尚未接入”应更新为“Stamp-P4 产品板型未接入”，并核对具体芯片修订和 ROM。

StampFly 还需要其 I²C/SPI 传感器、PWM 电机输出、遥控协议和飞行反馈闭环。现有通用 Stamp-S3 GPIO/RMT 并不覆盖这些功能。S3Bat 的电池供电与按键、StamPLC 的显示/继电器/输入，以及 C6LoRa 的 SX1262 也须分别建立行为边界。启动日志、前端颜色或旋翼动画都不替代客体读数和输出。

## 基线：当时的来源与工具边界

19 个现有产品均可执行：

```sh
python3 tools/rebuild-product-model.py plan <product-id>
python3 tools/rebuild-product-model.py build <product-id> --scratch <fresh-scratch>
```

本次 `plan` 全部成功；17 个产品的必需来源在仓库中可找到且 SHA-256 匹配。K056 缺 `product-photo-2849521917`、C056-B 缺 `product-photo-12e9471163`，都是既有贴图源照片，需按固定 URL/hash 显式加 `--allow-network` 恢复；没有发现现存来源 hash 不符。详见[模型复现说明](model-reproduction.md)，不要调用依赖旧私有队列的 `prepare/publish` 作为公开重建入口。

构建器 `tools/onboard-lane-01.py` 已支持明确 alpha 多边形/圆角矩形/孔洞排除、四点透视校正、厂家三角形上的 XY 正背面投影，以及独立派生面。当前 renderer 的 `box`、`cylinder`、`lathe`、`torus` 可以表达有依据的简单紧固件；精确螺纹和特殊头型优先导入相应 CAD，不把这些几何原语称为已核实标准件。

通用 renderer 没有自动判定纸贴是否完全落在实际承载面内；CAD 贴图依赖法向、深度和指定平面筛选，矩形 label 依赖作者提供正确遮罩与位置。因此纸贴边界、孔洞、异高面、缺失实体支撑和细小螺柱仍需逐件近景验收。原始占位板代码将灰绿材质和双面 UNKNOWN 写死；本轮新增的 `part.color` 与 `geometry.labelSide` 约定见[模型验证指南](../../skills/esp-product-onboarding/references/model-verification.md)，其他产品不应未经照片核实批量改色或隐藏标识。

S007、S007-V033 的早期 `verification.json` 所记录模型 hash 与当前模型不同；后续 [S3 模型回执](evidence/stamps3/model.json)和 [S3A 模型回执](evidence/stamps3a/model.json)与当前 hash 一致。旧对照图仍可定位历史问题，但不得当成本轮最终资产验收。每台修复后应生成对应最终 hash 的尺寸、照片、材质和交互证据。
