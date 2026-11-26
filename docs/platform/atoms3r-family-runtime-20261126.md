# AtomS3R 的五款无屏主控与相机采集

本次为五个已有外观条目接入精确 SKU 的 ESP32-S3 板型，目录从 `unavailable` 改为 `partial`。运行能力仅包含下表的子集；官方摄像头应用、视频、语音、联网业务和实机均未通过本次验证。

| 产品 | 板型 | 本次实现的外设子集 | 未实现的主要能力 |
| --- | --- | --- | --- |
| C126-Ext | `atoms3r-ext` | BMI270 轮询寄存器、G41 USER 输入 | BMM150、IR、扩展板电气模型 |
| C126-CAM | `atoms3r-cam` | BMI270；GC0308 SCCB、供电、RGB565/YUYV/Y8 虚拟源经 DVP/LCD_CAM/GDMA 写入客体 RAM | UVC、IR、光学/ISP |
| K147-CAM | `atoms3r-cam-chatbot` | 精确 GC0308 主机的同一采集链 | Atomic Voice Base 音频、UVC、云端语音 |
| C126-M12 | `atoms3r-m12` | BMI270；OV3660 SCCB、供电、RAW/同尺寸已编码 JPEG 的 DVP/GDMA 采集 | UVC、IR、光学/ISP/编码器 |
| D062-M12 | `atoms3r-m12-volcengine` | 精确 OV3660 主机的同一采集链 | Atomic Voice Base 音频、Volcengine 业务 |

全部使用真实产品的 8 MiB Flash / 8 MiB Octal PSRAM 与 PICO 包型 1，不继承 C126 显示屏、背光或 G41 屏幕按钮。只有 Ext 的官方 pinmap 列有 USER G41。本次将其接入面板/键盘 A；因尚无可核验的外观按钮位置，不增加模型按钮热区。摄像头两款与 M12 两款均没有假屏幕、假 G41 或静态视频画面。

BMI270 的实际器件地址为 `0x68`，SDA45/SCL0；硬件 I²C0/1 与同脚软件 I²C 共享一个器件状态。其已有寄存器模型要求完整 8192 字节初始化配置，按 8g/2000dps 缺省范围返回已设置的虚拟样本；物理安装轴未标定。BMM150 位于 BMI270 AUX 总线，不另加一个伪造的直接 I²C 地址。

GC0308 的 SCCB 地址为 `0x21`，M12 的独立 OV3660 为 `0x3c`、ID `0x3660`，不使用其他传感器代替。两种传感器均使用 SDA12/SCL9；G18 经真实 GPIO 矩阵拉低后才响应，掉电后 NACK，重新上电恢复缺省寄存器与无已配置图像输出。硬件 I²C0/1 与软件 SCCB 共享同一个传感器。声卡 `0x18` 不因模型上有音频底座就返回 ACK。

四款相机的 D0..7 固定为 G3/G42/G46/G48/G4/G17/G11/G13，PCLK40、HREF14、VSYNC10、XCLK21。客体配置 LCD_CAM 时钟源/分频及 GPIO 矩阵后，明确的虚拟 RGB888 图片按寄存器输出格式转换，经现有 GDMA trigger5 描述符实际写入客体内存；SCCB、DVP、XCLK 或供电错误不会由前端图片绕过。采集帧时序采用公共 S3 模型的近似 10 Hz、5% 消隐与半周期有效数据。没有校准真实传感器 PLL/PCLK、启动延迟、电源爬升或物理光学。

GC0308 `0x25` 的 DATA/PCLK/HREF/VSYNC 四个输出使能位必须全部置位，`0x24[4:0]` 的 6/2/17 分别选择 RGB565/YUYV/Y8。窗口 `0x09..0x0c` 含八行/列非有效数据；`0x46..0x4c` 提供裁剪，P1 页支持 esp32-camera v2.1.4 的八组已公开 subsample 配置，其他模式不给出猜测图像。模拟图片以 640×480 有效区域为坐标基准，最近邻采样不是真实 ISP 的邻域平均。模拟镜像与翻转来自 `0x14[1:0]`。未知 Bayer/其他格式、关闭输出、越界 ROI 与未支持的抽样模式均不产生采集字节。

OV3660 使用 `0x3808..0x380b` 输出几何、`0x4300` 格式、`0x3008` 复位/待机、`0x3821` JPEG 位，支持 RAW RGB565/YUYV/Y8 与数组 ROI、镜像/翻转。JPEG 没有内部编码器：只将同尺寸宿主 baseline JPEG 原样传输，不从 RAW 或前端预览伪造 JPEG；JPEG 已编码字节不再应用镜像/翻转。GC0308 不提供 JPEG 模式。四款都不实现 USB UVC 端点。

2026-11-26 实际重新读取五款官方产品页，URL、UTC、字节数和 SHA-256 保留在 [来源记录](evidence/atoms3r-family-20261126/sources.json)，知识记录分别增加本次来源，旧版来源保留。M12 与 Volcengine 的旧知识表头曾写 `OV3360`；当前官方页面已改成 `OV3660`，本次记录更正与历史差异，未据此推断额外运行支持。

既有厂家外壳 CAD、照片纹理、尺寸和 UNKNOWN 内部件保持原有验收边界。本次不改模型几何或纹理，也不声称模型 1:1 通过。Ext 的原型板/FPC 外观缺件仍在条目限制中，Grove 视觉方位未核验的线缆仍不显示连接。被测资产 hash 见 [检查记录](evidence/atoms3r-family-20261126/checks.json)。

C126-CAM 与 C126-M12 保留各自已有的固定 hash 官方出厂镜像与 factory-first/default 设置，既有 UIFlow2/小智映射不变。重新核对当前产品页中的出厂说明和源码入口后，没有把主机 UVC 固件认作整套 K147-CAM 或 D062-M12 的出厂程序，也没有为 Ext 编造镜像。调查结果及本次 M5Burner 元数据请求回执见 [固件调查](evidence/atoms3r-family-20261126/firmware-audit.json)。没有镜像被下载到服务器，也没有执行或刷写官方固件。

定向 Rust 检查覆盖五板身份、真实 BMI270 初始化/寄存器结果、错地址拒绝、无屏/无假音频、相机掉电/恢复/板复位。新增 `esp32s3/tests/atoms3r_capture.rs` 的原始 LX7 S32I/L32I 客体通过真实 GPIO/I²C/LCD_CAM/GDMA MMIO 验证四款相机 RAW/Y8 内存值、OV3660 JPEG 原样传输、错误针脚、时钟、供电和两种重启边界；测试不等同于官方固件或硬件验收。源资料见[固定版本 M5 示例及驱动](research/atoms3r-capture-20261126/sources.json)，共用 S3 信号/寄存器与 OV3660 原语来源见 [CamS3 清单](research/m5-cams3-runtime-20261126/sources.json)。生产 WASM 与浏览器需要在合并构建后单独验证，未完成前不得把本报告升级为应用兼容通过。
