# Station、Paper、CoreInk 与 VAMeter 修复

> 历史批次记录：下文“未实现”等状态只描述当时快照。当前 Paper 三SKU 已补 GT911 单触点、SD SPI 数字传输子集；详见[当前运行边界](m5-classic-completion-runtime-20261126.md)和[最终13SKU生产Worker回执](evidence/m5-final-integration-20261126/classic-thirteen.json)。CoreInk/Paper/K146 的本次实际3D键面映射与外观增量另见[五SKU控制证据](evidence/m5-control-caps-five-20261126/README.md)。这些更新不构成厂家完整固件或实机验收。

基线：`81bc93c0333a9fd13c16565b245b3d1c0d7ce237`。本批涉及五款产品，逐款记录能力边界，不以目录中出现运行板型代替官方固件验收。

| 产品 | 修复 | 仍未验证或实现 |
| --- | --- | --- |
| Station-Bat K124-B | 独立 `station-bat` 板型，真实 SPI18/23、CS5/DC19/RST15 的 ST7789V2；GPIO37/38/39，硬件 I²C21/22 上 AXP192 寄存器/LDO3 背光、MPU6886 轮询与 BM8563 日历；三个原厂 CAD 键面内的 3D 热点 | 出厂固件启动、INA3221/199、RS485、RGB、Grove 供电、软件 I²C、模拟电池与物理电源行为 |
| Station-485 K123 | 独立 `station-485` 身份，按同一官方 Station 引脚资料接入上述子集；补回原来为空的按键目录与三个 CAD 热点 | 同上；未把 RS485 的存在当作已模拟 |
| Paper v1.1 K049-B | 纸贴 alpha 避开复位键旁真实机械细槽，移除被错误烘焙为印刷的黑色孔洞；保留相邻原始纸贴像素 | IT8951/GT911 板级运行未实现；没有声称全机 1:1 外观通过 |
| CoreInk K048 | USB-C 改为从原口沿向内延伸的中空插座，避免原口沿上的黑色封口面；隐藏深度 3.2 mm 明确为估算 | 两种墨水屏硬件版次和运行协议未适配；非制造 CAD 精度 |
| VAMeter K136 | 按官方固定提交 HAL 配置补充编码器 A=G3/B=G1、按压 G2、BOOT G0；恢复可发现的硬件操作资料 | 板级运行与旋钮 3D 热点未实现；硬件 RESET 不冒充 GPIO |

Station 的按键平面直接测自已固定的原厂几何：X 中心 -10/0/10 mm，顶面 Z=16.5 mm。K123 顶面范围 Y=4.726259…9.473742 mm；K124-B 为 Y=2.726259…7.473742 mm。热点宽高 6.5×4.5 mm，分别居中于 Y=7.1/5.1 mm，距顶面 0.02 mm。外壳、坐标变换及参考照片均未重新拟合。

Paper 原图固定 SHA-256 为 `1c1239c6e6a283b463811458975bde4e0af0bfa146a926cca840aae4bab7bdea`。原图像素 X290…298/Y281…309 的机械细槽在现有 alpha 中被保留为不透明，本次以显式排除多边形恢复透明；其余包含区域和裁切保持不变。重建仍由 `model-plan.json` 的 `textureSources` 使用公开导出器完成，不依赖临时素材。

本轮读取了五款独立官方页面，并保留原始 URL、UTC 时间、媒体类型、字节数与 SHA-256，见 [回执](evidence/m5-classic-audit-20261126/receipt.json)。Station 引脚交叉核对官方 M5GFX 提交 `28d47949bdc84f30881623476f555398e2194313` 的 `extras/board_spec/boards/m5station.json` 和生成引脚表。VAMeter 引脚来自官方固件提交 `5e8f847bbc771da2d354be7472c6345acb5655ac` 的 `platforms/vameter/main/hal_vameter/hal_config.h`。

五款官方页面中的出厂程序、恢复工具或官方源码入口均已调查；现有固定/参考固件条目保留。没有下载 EasyLoader 的 EXE 冒充可启动 BIN，没有重跑 M5Burner 检索，也没有虚构新固件版本、校验值或通过结论。

验证命令：`cargo test -p esp32 --lib board::station`、`cargo clippy -p esp32 --lib -- -D warnings`、`node tools/platform-m5-classic-audit-test.mjs`。Station 测试实际发送显示命令/像素，核对错线路拒绝、LDO3 背光开关、GPIO 复位、引脚和独立 I²C 地址；模型测试测量真实 CAD 射线命中和 USB 空腔。浏览器三色材质、完整固件、整机照片叠图与实机验收本批尚未执行，不能由这些测试推导通过。
