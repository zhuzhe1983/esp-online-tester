# LILYGO 产品系列与适配进度

产品列表对照 LILYGO Spark 的 `product_list`，以系列 `id` 和产品 `product_id` 保留上游身份。本站纳入其中 21 个分组、127 个明确包含 ESP32 的 LILYGO 商品条目；非 ESP32、未明确主控的附件和通用 Espressif 开发板另记排除理由。清单没有单独的产品 `uuid` 字段，不生成虚构 UUID。此范围对应已固定的 Spark 清单，不等同于厂家全部在售产品。

工作台可按厂商、系列和具体型号选择，也可跨系列搜索。T-Deck 是系列，包含原版、Plus、Pro 及各自的 Meshtastic 预装商品，共 6 项；预装版身份不代表硬件已逐项验证。系列本身不能选择为可执行设备。所有产品沿用同一个场景、分享和嵌入协议。

以下 6 款继续作为优先适配对象，保留原有本地产品 ID、精确硬件配置、出厂默认项和模型。其余新增条目先建立来源可查的产品列表，容量、屏幕和引脚未核验的部分保持未知，不借用同系列型号的板级模型。

[Spark 系列与产品清单](research/lilygo-spark-product-inventory-2026-10-28.md)记录完整标识映射和排除项。[产品顺序与后续队列](research/lilygo-priority-2026-10-28.md)记录了关注度来源；[目录与固件审计](research/lilygo-catalog-audit-2026-10-28.json)保留每款产品的出厂固件调查和校验数据。

| 具体产品 | 本轮已验证 | 尚未覆盖 |
| --- | --- | --- |
| [TTGO T-Display 原版](../evidence/classic-light-sleep-tdisplay/README.md) | 原厂 ROM、屏幕、GPIO0/35、轻睡眠、深睡眠与唤醒 | 真实电池、射频和实机验证 |
| [T-Display-S3 非触摸版](lilygo-tdisplay-s3-runtime.md) | 官方 TFT 示例、GPIO i8080、LEDC 背光、GPIO0/14 | 原厂/LVGL 镜像仍卡在 DMA；整机保持 partial |
| [T-Deck 原版](lilygo-t-deck-runtime.md) | 原厂 ROM、屏幕、GT911、35键、轨迹球、实际 CAD 点击 | 音频、SD、无线、电源及完整第二 MCU 执行 |
| [T-Embed CC1101 V1.0](lilygo-tembed-runtime.md) | 原厂菜单、屏幕、按键、编码器、RMT8、CAD 交互 | 硬件检测仍 Mixed/Unknown；电池、NFC、RF、SD、音频、睡眠 |
| [T-Beam V1.2 SX1276](lilygo-tbeam-runtime.md) | 原厂 ROM、GPIO38、PMU 短按、SX1276 数字收发、串口改频 | PSRAM、GNSS、OLED、真实 RF 和完整电源；3D 仅局部照片估算 |
| T-Watch S3 原版 | 已有目录、出厂固件和来源 | 按暂停范围保留后续队列，未开始板级适配 |
| T5 4.7 英寸 e-Paper | 已确认后续具体产品 `t5-4-7-inch-e-paper-v2-3` | 按暂停范围保留后续队列，未开始板级适配 |

2026-10-28 收尾范围为前五款；主任务与三个并行任务均停止扩展。以上均未实机验证，partial 不代表所有外设或任意固件兼容。

产品事实、来源抓取时间、源文件摘要、版次冲突和未知项保存在 `web/platform/knowledge/lilygo-*.json`。官方 CAD 的来源、变换和验证限制随 `web/platform/assets/lilygo-*` 资产保存；视觉预览不等于 1:1 全部验收通过，更不代表固件兼容。

[四款 CAD 模型报告](lilygo-models.md)记录尺寸偏差、缺失部件、材料和许可状态。T-Embed 内部 PCB 使用明确的 `UNKNOWN` 占位；没有把猜测的元件布局当成厂家设计。

## 固件入口

已核验出厂来源的 6 款设备，固件库默认选择对应的官方出厂镜像；仅完成列表登记的新条目不虚构默认固件。二进制由浏览器直连固定的官方远程地址，本站只保存元信息。布局、芯片、容量和 SHA-256 核对通过不代表全部外设已经模拟；未验证镜像保留试运行状态，没有板级模型的设备显示“尚未完成适配”。

在“远程地址”的来源选择框中选择 **LILYGO Spark**，位置与 M5Stack 的 Burner 相同。按当前产品的 Spark `product_id` 搜索项目和选择版本，不用名称前缀或系列名匹配。已核对到具体硬件的 BIN 使用原有下载、验证、运行和历史记录流程。新增条目可以查看 Spark 标记的固件资料，但镜像尚未核对的条目仅作参考；只有 ZIP 的条目也保留来源说明，不把压缩包当作完整镜像运行。

Spark 官方配置中的 OSS 端点本轮未提供本站所需的 CORS 响应头，因此使用其官方 GitHub 仓库内可跨域读取的清单。界面展示该清单的日期；它可能落后于桌面 Spark 的线上数据。上游标签中存在跨型号混挂，本地匹配还会核对具体产品、仓库路径或文件名。没有匹配项时保留手动 HTTPS 和本地固件入口。

[Spark 来源与 CORS 审计](research/lilygo-spark-cors-2026-10-28.md)列明清单地址、时间、摘要及每个型号的匹配边界。本轮真实浏览器从本站源成功读取官方清单（生成日期 `2026-06-26`），并取得 T-Display-S3 的两个项目；这项检查只读取元信息。

开源项目仍加入原有平铺共享的 `firmware-watchlist.json`，不另建 LILYGO 专用列表。项目支持的设备、镜像格式和自动同步规则分别核对，不因为同为 ESP32 而互换固件。

## 验证方法

`tools/platform-lilygo-runtime-test.mjs` 通过公共产品定义、平台 runtime 和真实 WASM worker 启动原有 MicroPython 价格屏测试。它检查固件绘制的 135×240 framebuffer，并通过固件读取 GPIO 验证两个按键的按下、释放、组合键及暂停后释放。此测试固件仅用于回归，不冒充出厂固件，也不进入公共固件目录。

早期原版 TTGO 出厂镜像每约 1.675 客体秒出现 RTCWDT 复位，负结果保留在首阶段回执。随后补齐 classic 轻睡眠后，固定镜像在 native 与实际 Worker 通过菜单、电压/扫描、深睡眠及唤醒，五个 RGB565 帧一致，未再出现 RTCWDT。详见 [EX233 扩展证据](../evidence/classic-light-sleep-tdisplay/README.md)。默认仍为同一个官方镜像，不以价格屏测试替换。

目录检查使用 `python tools/validate-platform-catalog.py`；平台回归使用 `node --test tools/platform-*-test.mjs`。UI 检查包括厂商切换、Spark 搜索与版本选择、3D/2D、外壳材质、分享及嵌入页。未进行真实设备刷写或实机对比。

首批 6 款接入时的 105 项平台测试、目录 schema 与证据隐私检查通过。真实浏览器检查了 6 款的出厂默认项及运行按钮边界、4 套模型加载、Spark 搜索和选择版本、尚未运行的选择分享、iframe 源/hash 还原、2D/3D 切换和外壳原色/金色/紫透。未验证远程源的 mini 自动启动会先确认，取消时没有固件下载；原版 3D 上下两个按键分别命中 GPIO0/35，并由 MicroPython 固件读取确认。结果见 [集成回执](evidence/lilygo-integration/receipt.json)。

最终检查与暂停范围见 [2026-10-28 收尾记录](lilygo-runtime-closeout-2026-10-28.md)。
