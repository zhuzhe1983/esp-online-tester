# CloudZao 批量适配 · 2026-11-23

基准仓库为 `zhuzhe1983/esp-online-tester`，基准提交为
`4d8f3d892e9171a61e8ff0008a029db6aea0e4bf`。本次按用户要求统一显示为
**CloudZao**，保留 Metalio 旧名、原始仓库路径和既有设备 ID。交付方式是包含二进制资产的
Git diff，由本地 Codex 合并；本次没有推送、合并远端分支或部署 Pages。

日期按 Asia/Shanghai 记为 2026-11-23；抓取、运行收据中的时间保留 UTC。

## 四款设备与优先顺序

官方账号公开仓库清单中的四款主控设备都带屏。没有找到可统一核验的实物上市日期，故使用
**官方仓库最早可达提交日期**作为明确标注的排序代用值，不能据此推断真实上市或首次公开时间。

| 顺序 | 显示名称 / 稳定 ID | 主控与屏幕 | 模型来源 | 当前运行边界 |
| --- | --- | --- | --- | --- |
| 1 | CloudZao Claw 1.54 / `cloudzao-claw-154` | ESP32-S3；240×240 ST7789；CST816S | 精确型号官方单图估算，未知内部用 UNKNOWN | 新增独立板型。显示、轮询触摸、GPIO/盖板导航有客体诊断；官方整包已启动出帧，随后等待 ML307 |
| 2 | CloudZao INK4 Plus / `metalio-ink4-plus` | ESP32-S31；684×1216 并行电子纸；FT6336U | 沿用已有厂家 STEP、来源与 CC BY-NC 4.0 许可 | 保留现有部分运行实现及此前官方固件验证；未重新宣称全功能支持 |
| 3 | CloudZao INK4 / `metalio-ink4` | ESP32-S3；480×800 SSD1677；CST816S | 沿用已有厂家 STL、来源与 CC BY-NC 4.0 许可 | 保留现有部分运行实现、固件默认项与场景兼容性 |
| 4 | CloudZao Claw4 / `cloudzao-claw4` | ESP32-P4 主控 + ESP32-C5 无线协处理器；720×720 NV3051F MIPI-DSI；GT911 | 两张官方多视角宣传图与约 80×80 mm 尺寸 | 模型和知识预览；`board:null`。MIPI-DSI 板级执行及 P4 在线刷写尚未接入 |

对应排序日期为 2026-10-10、2026-09-30、2026-09-14、2026-07-01。原始 commit 身份、
抓取摘要与日期见 [排序收据](evidence/cloudzao-20261123/catalog-chronology.json)。
Burner 中另见的 AI ToyBox 使用 BK7258，不属于本次 ESP32 主控范围，见
[设备与渠道调查](evidence/cloudzao-20261123/inventory-and-firmware-audit.json)。

品牌筛选、知识标题和固件发布者现在使用 CloudZao；搜索 Metalio、Nologo、无名科技仍能找到
对应产品。两款 INK 的 runtime ID、固件 ID、下载 URL、SHA-256、型号资产路径和已有场景不变。
排序只调整 CloudZao 自身的队列，不重新排列其他品牌的设备。

## Claw 1.54 的实际执行证据

独立板型使用官方固定源码中的 SPI3 路由：SCLK 9、MOSI 10、CS 14、DC 8、RESET 18，
背光 13；CST816S 位于 I²C 0x15、SDA 41 / SCL 42。官方初始化的触摸 IRQ 和 RESET 均为
`GPIO_NUM_NC`，因此没有从其他设备借用中断线。BOOT 为 GPIO0，电源感知键为 GPIO47；
电源键输入不等于模拟实机断电。

新增两项板级测试、三项真实 Xtensa 寄存器诊断，覆盖引脚路由、错误地址/引脚拒绝、
复位、背光关闭不清空 GRAM、双轴镜像、导航触点与释放。PWM 只按零/非零门控显示；
光学亮度曲线、fade、反色和 MADCTL BGR 颜色校准不在已验证范围。

`tools/platform-cloudzao-runtime-test.mjs` 通过正式 public runtime、真实 Worker 与生产 WASM
执行原创 SRAM 诊断。全部 240×240 像素由客体 SPI 写入：红色 → BOOT 按下绿色 →
释放恢复红色；还检查暂停、重置、真实 ROM/WASM 字节、16/8 MiB 配置以及没有 stub。
该诊断不是官方固件，也没有冒充官方应用触摸验证。

另用未修改的官方完整镜像进行 ROM 启动探测，见
[官方启动收据](evidence/cloudzao-claw154-official-probe-20261122.json)：

- 官方 bootloader 加载 `0x100000` 的 OTA 应用，日志确认 `Metalio-Claw-1.54`、`2.0.51` 与 `app_main()`。
- 45.053 秒墙钟预算内执行 455,307,234 条指令、27.8 秒客体时间，接收 80 个画面事件。
- 最终画面为 240×240，16,991 个非黑像素、241 种 RGB；RGBA SHA-256 为
  `ed5db04f5696469b333a53d4ff711b3f93fa40fdced30f496a1a846dd71fd699`。
- 运行没有报告错误；预算结束后主动暂停。ML307 波特率探测到第 22 轮仍是
  `tx_written=36`、`rx_bytes=0`，与本板尚未模拟的蜂窝模块边界一致。

这证明原始镜像的部分启动和显示链路，不证明完整应用、云服务、音频或实机通过。
固件 `simulationVerified` 和 `hardwareValidated` 保持 `false`，产品运行状态保持 `partial`。

## 模型与来源精度

**厂家模型优先。** 两款 INK 已有厂家 CAD，直接保留；没有为改品牌而重做或改写许可证。
两款 Claw 的官方仓库树、资料链接与精确型号搜索未取得匹配 CAD，才使用来源图和标称尺寸。

Claw 1.54 中英文产品图的摘要相同，实际只有一个独立视角。模型按 1.54 英寸方形显示区
估算约 35.7×42.8 mm 的前盖；8.5 mm 厚度仅为可替换的展示参数。另一张 Burner 图片缺少
精确 SKU 关联，只记录为候选。背面、内部与物理 BOOT/电源键位置未知，后两者放在独立
DEBUG INPUT 区域。USB-C 只作外观，`allowConnections:false`，没有推测完整接插件线序。

Claw4 使用官方正反面与斜视宣传图；未将其视为已校准的实物照片。主体按约 80×80 mm，
厚度 10 mm 与镜头/按键突出均为估计，保留屏幕标称尺寸与宣传图比例不完全一致的事实。
两款模型均提供可展开的 UNKNOWN 内部层，保持屏幕、按键、印刷和外壳材质独立。
官方图片中的屏幕内容不会充当模拟 framebuffer。模型均不用于制造或装配公差校核。

详细来源、假设、许可与离线重建命令分别见
[Claw 1.54](cloudzao-claw154.md) 与 [Claw4](cloudzao-claw4-adaptation.md)。

## 官方固件与更新策略

| 产品 | 已选官方资产 | 实际大小 | 审计与执行状态 |
| --- | --- | --- | --- |
| Claw 1.54 | `Metalio-Claw-1.54_2.0.51_20261014_1105.bin` | 16,722,264 字节 | 固定日期文件名 + SHA-256；静态校验通过；上述部分启动/出帧 |
| Claw4 | `v2.0.5.1/Metalio_Claw4_2.0.5.1.bin` | 30,944,336 字节 | 固定版本 tag + SHA-256；静态校验通过；未执行 |

两份完整下载只在内存中审计，检查 SHA-256、芯片 ID、容量、boot offset、分区 MD5、
已填充应用段边界、XOR checksum 与附加 SHA-256。Claw4 发布 tag 是 `v2.0.5.1`，但镜像内
app descriptor 是 `2.0.5`，目录分别保留；不能用当前 main 配置代替历史镜像事实。
报告与复跑器见 [RAM 审计](evidence/cloudzao-20261123/firmware-memory-audit.json) 和
`tools/audit-cloudzao-firmware.py`。

两款均登记为 `role:official`，没有证据认定选定版本是出厂预装版本。GitHub 发布没有开启
不可变发布；Claw 1.54 的 `latest` 还会被上游 CI 删除重建。固定摘要限制平台接受的内容，
不保证上游资产永久在线。两个新 watchlist 使用规范 GitHub adapter URL 与历史发现，
`catalogTargets:[]`，只发现候选，不自动替换已审计的默认镜像。
UIFlow/小智渠道调查按精确设备身份记录，未将同芯片或相似名称当作兼容证据。

## 验证与本地合并

最终命令、计数与限制见 [集成验证记录](evidence/cloudzao-20261123/validation.json)。
模型资产已经做来源摘要、几何有效性、实际 Three.js 加载、展开复位、材质隔离和确定性重建检查。
Node 模型检查用本地文件读取与图片解码替身；这些检查不等于 WebGL 视觉验收。

本环境 Chromium 在导航前遇到 socket 权限错误；云浏览器也无法访问本地服务且被访问策略阻止。
因此**真实浏览器的正反面、侧面、展开状态和画面贴合验收未通过执行**。保留
`tools/fixtures/cloudzao-claw4-model-check.js` 供本地按仓库浏览器流程复跑，同时复核 Claw 1.54。
没有生成或提供虚构的浏览器截图。

合并补丁后，需要用仓库构建流程重新生成 `web/wasm/esp32sim.wasm`；编译产物、ROM、
第三方官方 BIN 和既有大体积 INK CAD 不重复放入补丁。源码、原创诊断、两款新模型及来源资产
全部包含在补丁中。新增 skill 不需要重复安装：当前已安装的 `esp-product-onboarding`
指令与仓库版本一致，本次已按该流程执行。

本地主要检查命令：

```sh
python3 tools/validate-platform-catalog.py
tools/fetch-rom-elfs.sh roms
tools/wasm-build.sh
ESP32SIM_ROM_DIR="$PWD/roms" node --test tools/platform-*-test.mjs
ESP32SIM_ROM_DIR="$PWD/roms" cargo test --release --workspace -- --include-ignored --skip external_
```

全工作区原生测试还需按 `CONTRIBUTING.md` 获取固定版本的 MicroPython 与 P4 示例。
模型/官方整包复跑脚本的详细参数见两款专项文档。
