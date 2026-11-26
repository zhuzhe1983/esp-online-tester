# M5Stack ESP32主控审计与增量补丁交付

本轮基线 `81bc93c0333a9fd13c16565b245b3d1c0d7ce237`。产品与代码检查快照 `fbe068e3174486f867d61c3a560dc2e81c44909b`。按用户确定的范围，纯配件另批；内置可编程ESP32的Unit设备纳入本轮。

已对固定来源的687个商城商品完成两阶段角色复核，并与原厂硬件目录交叉检查。当前M5Stack目录从91增加到141条，新增50条；本轮确证的缺失SKU已全部进入目录。该覆盖不等于所有历史PCB版次、隐藏结构或整机固件都完成验收。

运行状态为101条partial、40条unavailable。后40条仍未完成运行适配；模型预览和固件参考不能作为执行支持。44个含主机组合另有逐SKU映射，其中5个精确供货版次未锁定；A072的标题与Includes冲突继续保留。

## 有证据的修订

- 四款ATOM排母区域背贴遮罩、StampFly照片投影UV、Paper v1.1背贴槽、CoreInk中空USB口已修正；纸贴、PCB、端子及光学件与壳体改色隔离。
- 补齐独立SKU、原厂CAD或明确的照片近似；包括相机各传感器/版本、Atom-PSRAM 8/2MB、BALA旧FIRE 16/4MB/ILI9341等差异。未知PCB、隐藏尺寸、缺件和来源冲突保留。
- 增加或核实AtomS3R、Station、NanoH2、C6L、PowerHub、Capsule、AtomS3U、Cardputer Mesh，以及Paper/CoreInk/VAMeter/XiaozhiCard/K159/C156的已声明数字子集。
- 真实CPU/Worker/WASM测试覆盖按键、编码器、触摸引起的客体像素变化，以及错误引脚、未供电或未实现路径的负例。未把诊断程序当厂家完整固件验收。
- 固件身份逐SKU记录；factory/UIFlow/小智参考和watch保留远端链接，不镜像厂家BIN。新运行代码需要重建WASM，下载页另附对应的已测生成产物。

## 集中验证

| 检查 | 结果 |
| --- | --- |
| 全量平台Node回归 | 577项，573通过、4跳过、0失败 |
| 全工作区Clippy，全部targets，warnings denied | 通过 |
| 生产WASM的M5Stack目录构造 | 101个实际构造通过 |
| 通用模型资产加载、hash与非壳材质隔离 | 139通过；2个专用渲染器不在此工具范围 |
| 图像格式与首帧解码 | 882文件通过，0失败 |
| 实际透明度与通用显示面CPU视线 | 621采样，0遮挡候选；逐灯矩阵单列 |
| release工作区完整Rust回归 | passed |
| release WASM Clippy + jit-tests | passed |

细项见 [机器状态](evidence/m5-final-audit-20261126/final-status.json)、[产品CSV](evidence/m5-final-audit-20261126/product-status.csv)、[模型资产](evidence/m5-final-audit-20261126/assets.json)、[图像解码](evidence/m5-final-audit-20261126/images.json) 和 [显示面视线](evidence/m5-final-audit-20261126/screen-surfaces.json)。完整运行回执、命令和日志校验值保留在同目录。

## 仍未完成运行适配的条目

逐款外设与固件限制在机器状态及正式知识记录中；Tab5另有DSI/PHY/DMA和触摸链路阻断报告。下表保持unavailable，不隐去未完成项。

| SKU | 产品 | 芯片 |
| --- | --- | --- |
| A172 | M5Stack Stamp-AddOn C6 For P4 · A172 | esp32c6 |
| C145 | M5Stack Tab5 · C145 | esp32p4 |
| K014-B | M5Stack BALA FIRE 主控预览 · K014-B | esp32 |
| K016-B | M5Stack M5Stick 1.3 OLED · K016-B | esp32 |
| K016-P-SE | M5Stack StickC-Plus SE · K016-P-SE | esp32 |
| K016-P2 | M5Stack StickC-Plus2 · K016-P2 | esp32 |
| K016-T | M5Stack M5StickT Lepton 3.0 · K016-T | esp32 |
| K049 | M5Stack Paper (first generation) · K049 | esp32 |
| K049-C | M5Stack Paper COMM Edition · K049-C | esp32 |
| K115 | M5Stack Atom-PSRAM LCD Display Driver Kit · K115 | esp32 |
| K117 | M5Stack AtomU · K117 | esp32 |
| K126 | M5Stack T-Lite 热成像套件 · K126 | esp32 |
| K140 | M5Stack SwitchC6 单火线开关 · K140 | esp32c6 |
| K162 | M5Stack ToughC5 · K162 | esp32c5 |
| M141 | M5Stack Module Gateway H2 · M141 | esp32h2 |
| U007 | M5Stack ESP32 Camera · U007 | esp32 |
| U017 | M5Stack M5Camera · U017 | esp32 |
| U017-PCBA | M5Stack ESP32-CAM PSRAM Fisheye · U017-PCBA | esp32 |
| U037 | M5Stack M5Camera-F · U037（160°文档版） | esp32 |
| U038 | M5Stack M5Camera-X · U038 | esp32 |
| U082 | M5Stack TimerCamera · U082 | esp32 |
| U082-F | M5Stack TimerCamera-F · U082-F | esp32 |
| U082-X | M5Stack TimerCamera-X · U082-X | esp32 |
| U109 | M5Stack Unit CAM · U109 | esp32 |
| U109-B | M5Stack Unit Camera · U109-B | esp32 |
| U109-X | M5Stack UnitCAM DIY Kit · U109-X | esp32 |
| U120 | M5Stack Unit LCD · U120 | esp32 |
| U121 | M5Stack Unit PoE CAM · U121 | esp32 |
| U121-B | M5Stack Unit PoE CAM-W · U121-B | esp32 |
| U121-B-V11 | M5Stack Unit PoE CAM-W v1.1 · U121-B-V11 | esp32 |
| U133 | M5Stack Unit KMeter (ESP32-C3) · U133 | esp32c3 |
| U138 | M5Stack Unit PoESP32 · U138 | esp32 |
| U139 | M5Stack Unit ACSSR · U139 | esp32c3 |
| U140 | M5Stack Unit DCSSR · U140 | esp32c3 |
| U149 | M5Stack Unit Thermal2 · U149 | esp32 |
| U174 | M5Stack Unit CamS3 · U174 | esp32s3 |
| U174-B | M5Stack Unit CamS3-5MP · U174-B | esp32s3 |
| U195 | M5Stack Unit Gateway H2 · U195 | esp32h2 |
| U213 | M5Stack Unit PoE-P4 · U213 | esp32p4 |
| U215 | M5Stack Unit CardKB2 · U215 | esp32c61 |

## 来源、交付与边界

全量来源角色表、363+324逐条复核、44个主控映射与72个补充源的实际收据见 `docs/platform/research/m5-*-20261126.json`。源照片版权与CAD授权分别保留。模型重建、alpha承载与拒绝候选均留证；不将尺寸相近、构造成功或一次光线采样当作完整制造验收。

云浏览器未能建立WebGL上下文，因此没有完成产品3D真实渲染、文字可读性或同机位照片叠图验收。下载页本身已做实际浏览器检查。没有实机刷写、工厂校准或完整硬件验收。

本次以Sites顺序增量diff交付，含二进制模型资产与逐批SHA-256。第006批使用006-r2；已应用旧006者使用对应小补丁再接007。每批从上一目标tree连续验证。常规Git推送检查没有HTTPS写入凭据，未改GitHub主分支或宣称GitHub Pages已更新。
