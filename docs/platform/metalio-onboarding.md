# 无名科技 / Metalio INK4 与 INK4 Plus

2026-10-27 入库：两个产品系列、三个官方固件版本、两个按型号隔离的 CloudZao watchlist。厂家筛选为“无名科技（Metalio）”；可搜索 INK4、Metalio、CloudZao、无名、Nologo。

| 产品 ID | 官方源码主控 | 显示与触摸 | 默认官方固件 | 其他渠道 |
| --- | --- | --- | --- | --- |
| `metalio-ink4` | ESP32-S3 / 16 MB Flash | SSD1677，原生800×480 / 竖屏480×800，CST816S | 2.0.53 | 2.0.63 测试版 |
| `metalio-ink4-plus` | ESP32-S31 / 16 MB Flash | ED047TC2_1216，并行EPDiy + TPS65185，原生1216×684 / 竖屏684×1216，FT6336U | 2.0.68 | 本轮未另收旧测试版 |

官方来源：[INK4源码](https://github.com/CloudZao/Metalio-E-INK4/tree/d12f481f4a42420634bb56a928f3959a977367d4)、[Plus源码](https://github.com/CloudZao/Metalio-E-INK4-Plus/tree/ad8edf4150a688737c0d7c00cae4413079b262ef)、[CloudZao烧录站](https://burner.cloudzao.com/zh/registry)。已抓取来源的URL、UTC时间、字节数、SHA-256在两份 `web/platform/knowledge/metalio-*.json`；实际公共API原文含无关账户字段，只保留哈希收据，不复制到站点或证据文件。

两款均 **尚未完成板级适配**，工作台与mini隐藏运行按钮并显示红色提示。INK4不能直接复用PaperMono的引脚与电源模型；Plus的ESP32-S31是独立RISC-V平台，当前内核尚不支持。未运行官方固件、未连接硬件。公开图片有4G标识，源码与手册涉及不同配置，故本轮只建立两个系列，不虚构4G子型号。PSRAM已确认Octal接口/频率，但实际容量未确认，保留null。

## 固件发布规则

公开接口为 `https://admin.metalio.world/xiaozhi/api/firmwares?page=1&limit=1&hardwareModelId=<ID>&sourceType=official`，INK4 ID为38，Plus ID为44。还必须匹配完整固件ID、`chipName`、`sourceType=official`和发布状态；不能只靠名称。`versionStatus=test`表示测试版，即使版本号没有beta/rc字样；`latest`才进入稳定渠道。官方来源不等于证明实机出厂版本，当前固件为 `role: official`、官方分组、默认选中，不冒称 `factory`。

`tools/firmware-source-cloudzao.mjs` 接入既有发布前/Action元信息同步，不向此接口传GitHub令牌，不保留作者、头像、账户或评论。默认候选规则仅接受该型号记录中的官方CDN BIN。API没有SHA-256：已收录三个版本经一次RAM内审计固定hash，后续新候选缺hash则保留现有固件并报告 `candidate-missing-sha256`。自动更新不下载固件正文，也不自动声称新版本布局兼容。

[固件收据](evidence/metalio-onboarding/firmware-receipts.json)记录整包SHA-256、分区、ESP段校验与附加digest。两款的app均在0x80000；INK4 bootloader在0，Plus在0x2000（整包起始0–0x1fff为填充，整包仍从0刷写）。芯片ID分别9/32。审计不执行固件，BIN未写磁盘/Git。源CDN在浏览器HEAD检查可跨域读取、HTTP200、读取正文0字节；不代表未来永不发生CORS变化。浏览器缓存按平台原规则保留。

## 厂家 CAD 与精度边界

已通过浏览器下载无名科技Nologo发布的两个原始模型：[INK4 STL](https://makerworld.com.cn/zh/models/2978913-metalio-e-lnk4-aimo-shui-ping-yue-du-qi-mo-xing)、[INK4 Plus STEP](https://makerworld.com.cn/zh/models/3093684-metalio-e-ink4-plus47ying-cun-mo-shui-ping-yue-du)。初次无登录态访问遇到403，后续正常登录态下载成功。文件保存在各产品资产目录，来源、下载时间、字节数、SHA-256及原始文件名留在 `model-plan.json`；不留临时下载地址或账户信息。之前的照片估算外壳已替换。

| 型号 | 原始组件 | 主壳 CAD 尺寸（宽×高×厚，mm） | 转换 |
| --- | --- | --- | --- |
| INK4 | 8 | 63.6657 × 106.9859 × 6.8000 | 原始STL三角形，全组件保留 |
| Plus | 12 | 70.7326 × 122.1900 × 7.7000 | STEP以0.05mm / 0.2rad离散，全组件保留 |

尺寸范围不包括按钮/盖板突出部分。只做绕X轴+90°旋转和居中平移，缩放为1，没有单独移动零件。STL本身不声明单位，毫米解释依据标称屏幕与产品照片的一致性；STEP由原生读取器换算为毫米。导出float32的最大坐标舍入误差低于0.0001mm，逐组件检查保存在 `geometry-checks.json`。

两份MakerWorld模型标注 **CC BY-NC 4.0**，作者、链接、许可和改动说明见各目录 `NOTICE.md`。原始CAD及派生几何不套用代码仓库MIT许可。参考照片保留各自来源，不另作许可声明。

- CAD里的外框、盖板、侧键与不明细节分别保留。屏幕平面按标称对角线及照片定位，底部印刷标记是独立透明矢量图，没有把灰色盖板或固件界面当成贴纸。
- 原厂色、金色、紫色半透明、正反侧视和UNKNOWN检查图均使用最终CAD。改色只影响外框；盖板、屏幕、印刷和侧键材质独立。
- 厂家外观CAD不能证明完整PCB布局；内部仍使用明确标记的 `UNKNOWN` 空白占位。不明CAD细节不冒称电气元件。Plus缺少可靠背面照片，没有复制INK4背面图案。
- CAD转换保真检查通过；[照片对照](evidence/metalio-onboarding/photo-comparison.json)按屏幕四角将正面渲染投影到官方照片，报告外廓虚拟矩形角点误差。没有独立尺寸图、实物测量和完整六面照片，**完整1:1实物验收仍未通过**。不能用转换误差代替实物精度。

重建命令：`python tools/build-metalio-models.py --output <scratch>`（依赖 `tools/requirements-model.txt`）。仅读取已提交、hash匹配的原始CAD与计划；没有在线下载或私有目录依赖。已对比独立重建的geometry、model JSON、印刷SVG及几何收据。

复核页面：`platform/review-lane-01.html?id=metalio-ink4` 或 `id=metalio-ink4-plus`。照片对照：`python tools/verify-metalio-photo-models.py --evidence docs/platform/evidence/metalio-onboarding`。

## ESP32-S31 实现路径

当前项目已有 `riscv-rv32` 与双核 `esp32p4` 框架，但没有S31 SoC定义。S31的内存/中断/启动行为必须单独按ESP-IDF目标头文件和ROM核对，不能直接把产品配置指向P4或S3。

2026-10-27核对：[乐鑫esp-emulator](https://github.com/espressif/esp-emulator)已列S31并提供[浏览器WASM版本](https://github.com/espressif/esp-emulator/blob/main/docs/guides/BROWSER.md)，但标为early bring-up。公开仓库主要分发程序、文档与工具，未提供完整内核源码，因此不把它描述为可以直接移植的S31源码。其固件启动结果可作为独立对照；是否适合接入我们的板级模型，还需核对发布包接口与授权。

建议分阶段验收：固定ROM/IDF与最小串口固件 → S31指令/双核/中断/Flash/PSRAM并启动FreeRTOS → Plus板级I2C/TCA9555/FT6336U及并行墨水屏 → microSD与持久化 → 无线/音频和其余传感器。每阶段检查客体日志或framebuffer，不将3D预览当作固件运行。现有Rust内核继续编译为WASM即可保持静态部署；接入真实外网仍是另一项浏览器网络桥接能力。

## 验证

- 目录schema：86产品、6配件、114固件、46 watchlist；知识库90条通过引用/时间/未知项检查。
- Node平台76项通过（包括新增CloudZao身份、渠道、恶意链接、令牌隔离、隐私与无hash保留规则）。仅修改目录/前端元信息，无Rust/WASM变更。
- 真实headless浏览器：品牌选择、两款默认官方固件、测试版分组、3D加载、不可执行提示、mini分享配置均通过。没有发起固件下载或执行。
- 限制：未实现两款客体运行、4G、云端账户、音频、SD、传感器、真实硬件刷机或1:1几何验收。
