# M5_Hardware Products 目录与近半年变更核查

截至 **2026-10-26 15:12:44 UTC**，固定到 [`M5_Hardware@e29c139`](https://github.com/m5stack/M5_Hardware/tree/e29c139cbc7c80b5ccc5557e5ab8025a7a19a109/Products)，`Products/` 有 **270 个直接产品目录**。按对应官网产品正文与 SKU 核对，其中 **83 个非 Unit、以 ESP32 系列为主控的产品目录**，**154 个属于官网 Unit 分类的目录**。这两项采用不同维度：后者中又确认 **13 个 Unit 自带 ESP32**。

这里的“产品目录”包含硬件版本、引脚形式和套件差异；不是 270 个独立硬件设计，也不是 M5Stack 在售产品总数。仓库的结构文件覆盖范围不等于官网全部产品，更不代表这些设备均已可在模拟器运行。

## 统计快照与证据

| 项目 | 固定值 |
| --- | --- |
| 仓库 | [m5stack/M5_Hardware](https://github.com/m5stack/M5_Hardware) |
| 分支及完整 HEAD | `master` / `e29c139cbc7c80b5ccc5557e5ab8025a7a19a109` |
| HEAD 提交时间 | `2026-10-23T06:10:33Z` |
| Products 树对象 | `6ab969d6aae8f077c1478d8396de807fa613cfd3` |
| 统计窗口 | `2026-04-26T00:00:00Z` 至 `2026-10-26T15:12:44Z` |
| 期初基准提交 | [`d56a09d16e3a1e03b051420351d0237440907f93`](https://github.com/m5stack/M5_Hardware/commit/d56a09d16e3a1e03b051420351d0237440907f93)，即窗口开始前最后一个可达提交 |
| 基准提交时间 | `2026-04-10T04:19:08Z` |
| 官网核实时间 | `2026-10-26T15:19:16.724246Z` 至 `2026-10-26T15:19:46.038745Z` |

时间筛选使用 Git 的 **committer time**；该窗口没有合并提交。目录统计保留原始大小写及 Unicode 字符，不把连字符相似的名称擅自合并。官网使用当次下载的产品正文，不从导航菜单中的“ESP32”字样推断芯片。270 个目录均找到相应官网正文及 SKU；网页可能继续更新，所以每页记录了 URL、抓取 UTC、标题、SKU 和实际 HTML 的 SHA256。官网抓取时间晚于仓库快照，技术说明不被当作历史上市证据。

可复核文件：

- [inventory.json](m5-hardware-inventory-2026-10-26/inventory.json)：270 个目录的分类、芯片证据、版本标签、变更归属、固定版本文件链接和 Git blob 对象 ID。
- [inventory.csv](m5-hardware-inventory-2026-10-26/inventory.csv)：便于筛选的同一目录表。
- [sources.json](m5-hardware-inventory-2026-10-26/sources.json)：270 份官网来源的下载回执；只保存少量必要芯片摘录，不整页转载。
- [commits.json](m5-hardware-inventory-2026-10-26/commits.json)：58 个提交、文件级 A/M/D、逐提交目录增删改，以及期初/期末集合。
- [新增及更新完整清单](m5-hardware-inventory-2026-10-26/changes.md)：ESP32 主控、其他计算设备、Unit 分别列出；包括官方链接、芯片核实状态、模型格式和最后修改时间。
- [verify.py](m5-hardware-inventory-2026-10-26/verify.py)：离线一致性检查，可进一步对临时裸仓库和下载 HTML 逐项复核。

## 当前设备分类

本报告的“ESP32 主控产品”是非 Unit 的主控分类，包括完整主机、可编程控制器、核心板和确实包含主控的套件。`Stamp` 核心板也在这一口径内；单纯扩展板不算主机。官网 `/unit/` 中的 M5Camera、TimerCamera 等仍保留 Unit 分类，不因为它们能独立运行固件就转入主机计数。

| 互斥分类 | 当前目录数 | 判定依据 |
| --- | ---: | --- |
| 非 Unit 的 ESP32 主控产品，含核心板及带主机套件 | **83** | 官网主控/SoC 参数及产品定位 |
| 其他主控计算设备 | **5** | CM4Stack、CoreMP135、LLM630 Compute Kit、AI Pyramid、AI Pyramid-Pro |
| 官网 Unit 分类 | **154** | 对应 `/en/unit/` 产品正文与 SKU，不依赖 U 前缀 |
| 其他配件、底座、Hat、Cap、非主控模块 | **28** | 对应官网分类及产品说明 |
| 合计 | **270** | 每个当前目录只出现一次 |

5 个非 ESP32 主控设备中，CM4Stack 主控为 BCM2711，CoreMP135 为 STM32MP135DAE7，LLM630 为 AX630C，AI Pyramid / Pro 为 AX8850。**LLM630 同时有 ESP32-C6 网卡，但不能因此称为 ESP32 主机**。[CM4Stack 官方说明](https://docs.m5stack.com/en/core/CM4Stack)、[CoreMP135 官方说明](https://docs.m5stack.com/en/core/CoreMP135)、[LLM630 官方说明](https://docs.m5stack.com/en/core/LLM630_Compute_Kit)、[AI Pyramid](https://docs.m5stack.com/en/ai_hardware/AI_Pyramid)、[AI Pyramid-Pro](https://docs.m5stack.com/en/ai_hardware/AI_Pyramid-Pro)。

83 个 ESP32 主控目录按主控系列拆分：

| 主控系列 | 目录数 |
| --- | ---: |
| 经典 ESP32，含 PICO-D4、D0WDQ6 等 | 25 |
| ESP32-S3 | 47 |
| ESP32-C3 | 4 |
| ESP32-C6 | 3 |
| ESP32-P4 | 2 |
| ESP32-H2 | 1 |
| ESP32-C5 | 1 |
| 合计 | **83** |

每条芯片值及其官网参数摘录见 `inventory.json` 的 `chipEvidence`。芯片系列不同必须分别评估模拟后端；上表不表示现有 ESP32-S3 运行能力自动覆盖 C3/C5/C6/H2/P4。

前缀仅供检索：`A=7、AI=2、C=23、D=1、DK=1、K=58、M=4、S=11、U=163`。U 前缀的 163 项实际包含 154 个官网 Unit、8 个 Hat 和 1 个 Cap；另一个 Hat 是 K036-B RoverC-Pro。K009-F Watch 属于配件。因此不能用 U 前缀直接回答“有多少 Unit”。

## Unit 与 ESP32 的交叉

本轮明确确认 **至少 13 个 Unit 含 ESP32**：

| 官网 Unit 产品 | 本轮确认的 ESP32 芯片 |
| --- | --- |
| [M5Camera-X](https://docs.m5stack.com/en/unit/M5Camera-X) | ESP32，官网未在引用处细分料号 |
| [TimerCamera-F](https://docs.m5stack.com/en/unit/TimerCamera-F) | ESP32-D0WDQ6-V3 |
| [TimerCamera-X](https://docs.m5stack.com/en/unit/TimerCamera-X) | ESP32-D0WDQ6-V3 |
| [Unit LCD](https://docs.m5stack.com/en/unit/Unit_LCD) | ESP32-PICO，官网引用处未细分后缀 |
| [Unit PoE CAM-W v1.1](https://docs.m5stack.com/en/unit/Unit_PoE_CAM-W_v1.1) | ESP32-D0WDQ6-V3 |
| [Unit PoESP32](https://docs.m5stack.com/en/unit/Unit_PoESP32) | ESP32-WROOM-32U |
| [Unit ACSSR](https://docs.m5stack.com/en/unit/Unit_ACSSR) | ESP32-C3 |
| [Unit DCSSR](https://docs.m5stack.com/en/unit/Unit_DCSSR) | ESP32-C3 |
| [Unit Thermal2](https://docs.m5stack.com/en/unit/Unit_Thermal2) | ESP32-PICO-D4 |
| [Unit CamS3-5MP](https://docs.m5stack.com/en/unit/Unit_CamS3-5MP) | ESP32-S3-WROOM-1-N16R8 |
| [Unit Gateway H2](https://docs.m5stack.com/en/unit/Unit_Gateway_H2) | ESP32-H2-MINI-1-N2 |
| [Unit C6L](https://docs.m5stack.com/en/unit/Unit_C6L) | ESP32-C6 |
| [Unit PoE-P4](https://docs.m5stack.com/en/unit/Unit_PoE-P4) | ESP32-P4NRW32 |

其余 **141 个 Unit 的 ESP32 存在性没有在本轮被证实**，不能据此断言全部不含 ESP32。其中 8 项另有已确认的非 ESP 主处理芯片：GPS v1.1 / GPS SMA 为 AT6668，2 项 UnitV 为 K210，3 项 UnitV2 为 SSD202D，当前 Unit KMeter-ISO v1.1 为 STM32F030；剩余 133 项没有逐项保留完整主控/传感芯片认定。这是保守的存在性下界，而非完整 BOM 审计。

尤其不能把文档历史或使用示例中的 ESP32 当作当前内置芯片：Unit Angle 提到的是外部主机的 ADC；Unit KMeter-ISO 历史说明其主控在 2023 年已从 ESP32-C3 改为 STM32F030。[Unit Angle](https://docs.m5stack.com/en/unit/Unit_Angle)、[Unit KMeter-ISO](https://docs.m5stack.com/en/unit/Unit_KMeter-ISO)。

由此可证明该树至少 **97 个目录所对应的产品内含 ESP32**：83 个 ESP32 主控产品 + 13 个 Unit + 1 个 LLM630 的 ESP32-C6 网卡。这是另一种“含 ESP32”的口径，不能拿它替代 83 个 ESP32 主控产品。

## 半年内变化

窗口内全仓库 **58 次提交**，这 **58 次均涉及 Products**，共触及 **248 个不同的直接目录名称**。期初有 53 个目录，期末有 270 个。

### 期初与期末对比：互斥口径

| 分类 | 新增目录 | 既有目录内容变化 | 既有目录不变 | 当前合计 |
| --- | ---: | ---: | ---: | ---: |
| ESP32 主控产品 | 63 | 13 | 7 | 83 |
| 非 ESP32 主控计算设备 | 5 | 0 | 0 | 5 |
| Unit | 137 | 16 | 1 | 154 |
| 其他配件/模块 | 12 | 0 | 16 | 28 |
| 合计 | **217** | **29** | **24** | **270** |

期初存在、期末消失的目录为 **0**。这里“既有目录内容变化”按目录树对象比较，不以 commit message 中的 `add/update` 用词计数。53 + 217 − 0 = 270。

### 逐提交事件：可重叠口径

逐个提交与其父提交比较并对目录名去重，可见 **219 个名称发生新增事件、133 个名称发生修改事件、2 个名称发生删除事件**。这三组可以重叠：窗口内新增的目录可能随后又被修改，不能相加当作产品总数。

两个中途消失的名称均有明确迁移线索：

- `K151_stackchan` 后改为 `K151_StackChan`。两次提交都在 2026-05-25；删除旧路径与增加新路径发生于[同一次提交](https://github.com/m5stack/M5_Hardware/commit/70fb127275ef0e2da21218d1f01cd305754d35fa)。完整 SHA 以 `commits.json` 为准。
- `U209_Unit_Heart` 后由 `U029_Unit_Heart` 路径承接，2026-07-20 的提交删除旧路径并加入新路径；官网 Unit Heart 的 SKU 是 U029。这是目录纠正线索，不据此编造新产品或停产记录。见 [Unit Heart 官方说明](https://docs.m5stack.com/en/unit/Unit_Heart)及 [对应提交](https://github.com/m5stack/M5_Hardware/commit/9eecd0075e26e94c2f5a19bed81f9789a93e6544)的文件变更。

大量提交是在 6–8 月批量补充已有产品结构文件，所以 **“新增 217 个目录”不等于“半年推出 217 款产品”**。全部 63 个新增 ESP32 主控目录、13 个既有主控更新、137 个新增 Unit 目录和16 个既有 Unit 更新，详见[完整清单](m5-hardware-inventory-2026-10-26/changes.md)。

## 版本与主产品如何区分

主计数保留目录/SKU，不擅自折叠硬件版本。例如 Core2、Core2 v1.1、Core2 v1.3 各算一个目录；Core2 For AWS 及其 v1.3 也分别保留。Atom-Matrix、Capsule、Dial、Air Quality、Cardputer、DinMeter 的基础款与明确版本同样分开。

数据另提供 `explicitRevision` 和 `revisionNameGroup`：**只去掉官网名称末尾的 `vN` 标签**，83 个 ESP32 主控目录会成为 **73 个名称组**，154 个 Unit 会成为 **152 个名称组**。这一辅助结果仍不是独立设计数：Lite/Adv、S3/S3A、引脚形式、套装和射频频段都保留。例如 Stamp-S3 与 Stamp-S3A 没有因为 SKU 中带版本字段就被合并，PaperMono 与 PaperMono-Lite 也保持独立。

若产品库未来需要真正的“主产品 / revision / variant / bundle”关系，应在逐项核实兼容性后显式建立关系，不能只用前缀或名字裁剪自动生成。

## StopWatch、PaperMono 与 GPS

| 产品目录 | 主控/处理芯片 | 窗口内资料变化（UTC 日期） | 当前仓库内三维格式 |
| --- | --- | --- | --- |
| `C152_StopWatch` | ESP32-S3R8 | 2026-06-22 新增；2026-07-22 更新 | `Structures/StopWatch.stl` |
| `C153_PaperMono` | ESP32-S3R8 | 2026-10-22 新增 | `Structures/PaperMono.stl` |
| `C153-Lite_PaperMono-Lite` | ESP32-S3R8 | 2026-10-22 新增 | `Structures/PaperMono-Lite.stl` |
| `U032_V11_Unit_GPS_v1_1` | AT6668 / ATGM336H-6N | 2026-08-14 新增 | `Structures/Unit_GPS_v1.1.stl` |
| `U190_Unit_GPS_SMA` | AT6668 | 2026-08-14 新增 | `Structures/Unit_GPS_SMA.stl` |

芯片依据分别来自 [StopWatch](https://docs.m5stack.com/en/core/StopWatch)、[PaperMono](https://docs.m5stack.com/en/core/PaperMono)、[PaperMono-Lite](https://docs.m5stack.com/en/core/PaperMono-Lite)、[GPS v1.1](https://docs.m5stack.com/en/unit/Unit_GPS_v1.1)、[GPS SMA](https://docs.m5stack.com/en/unit/Unit_GPS_SMA) 官网正文；对应固定版本模型路径及 blob ID 见 `inventory.json`。

这些目录在本次固定树中均未提供 STEP/STP。旧款 U032 GPS 的 AT6558 版本在 GPS v1.1 文档对比表里出现，但该固定树没有它单独的产品目录；不能把 GPS v1.1 模型或配置默认套到用户未确认版本的硬件上。

## 模型覆盖与格式

`Products` 当前共有 **324 个文件**，其中 264 个产品目录至少含 STL、STP、STEP、OBJ 四种三维格式之一。

| 格式 | 文件数 | 含该格式的产品目录数 |
| --- | ---: | ---: |
| STL | 282 | 260 |
| STP | 5 | 5 |
| STEP | 1 | 1 |
| OBJ | 6 | 6 |

目录覆盖有交叉，不能把右列相加当作总数。另外还有 PcbDoc 10、PcbLib 5、SchLib 5、SchDoc 4、DWG 3、DXF 2、CDR 1。未发现 README / Markdown / TXT 产品说明；芯片核实因此使用官网，而非根据结构文件名称猜测。

STP/STEP 仅出现在这 6 个目录：

- `K009-F_Watch` → `Watch_Bottom.stp`
- `K025_Base15` → `Base15_DB9_01.stp`
- `K026_Base26` → `Base26_Main.stp`
- `K037_BaseX` → `BaseX_DB9_01.stp`
- `K143_LLM630_Compute_Kit` → `LLM630_Compute_Kit.stp`
- `S012_Stamp_C6LoRa` → `Stamp_C6LoRa_SMD.step`

其余 6 个没有上述三维格式的目录是 Faces Kit、Atom Hub Proto、Module Bus、Stamp-P4、Stamp-C5、Stamp UWB；这些目录有二维图或电路设计文件。格式覆盖只按文件树核实，没有把每个模型导入 CAD 检查完整性、单位、部件层次或可用许可，因此不把“有文件”表述为“有完整装配模型”。模型条目保留 Git blob SHA1 身份，不伪装为已下载模型的 SHA256。

## 复核步骤与边界

仅数据检查：

```sh
python3 docs/platform/research/m5-hardware-inventory-2026-10-26/verify.py
```

需要复核 Git 时，在主项目之外或忽略目录中创建临时裸仓库；`--filter=blob:none` 避免批量取得模型内容。以下命令不会 checkout、提交或修改本项目分支：

```sh
git clone --bare --filter=blob:none --single-branch --branch master \
  https://github.com/m5stack/M5_Hardware.git /tmp/m5-hardware-inventory.git
python3 docs/platform/research/m5-hardware-inventory-2026-10-26/verify.py \
  --git-dir /tmp/m5-hardware-inventory.git
```

脚本固定使用记录的 SHA，而非执行时的新 HEAD。下载 HTML 若保存在调用者提供的缓存目录，并按 URL SHA256 的前 16 个字符加 `.html` 命名，可追加 `--html-cache <目录>`；它会同时检查 HTML 哈希、正文标题、SKU 和每条芯片摘录。官网后续更新造成哈希变化时，需要重新审阅来源，不能静默覆盖本次回执。

本轮实际执行了完整 Git 及 270 页 HTML 检查，均通过；原始 HTML 留在临时缓存，不随研究文档整页提交。没有下载或保存固件二进制，没有向上游发送消息或提交变更。未完整审计 270 个产品的 BOM，也未证明每个 Unit 均不含其他协处理器；未知项已保留为 `not-established`。
