# M5Stack Basic K001 与 K001-V27

两条目录记录保留各自的厂商型号、外观和固件条目。当前 Basic 与 Basic v2.7 的官方 PinMap 对屏幕、三枚实体键和 TF 的数字接线一致，分别选用经典 ESP32 板型 `m5stack-basic`，配置 16 MiB Flash 和 0 MiB PSRAM。厂商记录的早期 4 MiB 和 TN 批次未验证，v2.7 的玻璃、电源开关、Grove 升压及 USB 芯片差异沿用各自资料。

- ILI9342C 的 SCK、SDA、CS、D/C、RESET 和背光依次接 G18、G23、G14、G27、G33 和 G32。320×240 LCD 复用 DCS 控制器，支持 RGB565 写入、地址窗、MADCTL、睡眠、显示开关和 GPIO/LEDC 背光开关。已验路径使用 RGB565 SPI 写入。寄存器读回、其他像素格式、PWM 亮度曲线和颜色校准仍待验证。
- 实体 A、B、C 通过 G39、G38、G37 接入，低电平有效。2017 年官方原理图第 4 页给出外部上拉。浏览器支持三枚 CAD 键、按键面板和键盘 A/B/C。屏幕可拖拽旋转。侧面供电键走独立电源电路。
- TF 卡与 LCD 共用 G18/G23 时钟和输出，返回信号为 G19，独立片选为 G4。数字 SDHC 通过现有 SPI 卡模型执行。平台首次准备 64 MiB FAT32 卡，已写入的卡由产品及固件存储范围保存。卡接口保留 IMG 导入和显式创建空卡操作。
- IP5306、实体电源及电池逻辑、扬声器输出、外部 Grove/M5-Bus 设备、老批次的运动传感器与实机行为未验证。工厂自检会继续显示缺失的 IMU 结果。Wi-Fi 扫描完成不能替代路由、互联网及射频验证。

[来源与 SHA-256](evidence/m5-basic/sources.json)

## 原厂固件验收

两条记录默认选择官方 M5Burner 的 Basic Factory Test v0.4。完整镜像的启动头位于 `0x1000`，分区表位于 `0x8000`，应用头位于 `0x10000`。使用指定 ESP32 ROM 和同一 SHA-256 镜像启动。固件从 TF 根目录写入 `/hello.txt` 并读回 `Hello world`。对三枚按键分别施加真实低电平，固件在 UART0 输出 A、B、C，且屏幕写入增加 49 个像素。错误 GPIO34 和无输入的帧哈希相同。无卡的对照运行显示 CMD0 和 FAT 挂载错误。

固件保留试运行状态。写入、按键和屏幕验收只覆盖以上功能。操作发生在虚拟设备，没有连接真实硬件。

[原生运行与负例收据](evidence/m5-basic/native-v04.json)

两条记录也在 Chromium 无头工作台从官方 CDN 下载并运行同一镜像。K001 使用 3D A、键盘 B 和 2D 面板 C 后，UART0 依次输出对应字符，320×240 帧缓冲随每次输入改变。K001-V27 的 3D A 有同样的反馈。用共享场景在 iframe 中重开 K001-V27 后，固件从浏览器保存的 TF 卡先列出既有 `hello.txt`，再写入并读回。

[浏览器和嵌入重开收据](evidence/m5-basic/browser-v04.json)

## 复现

用 `createFat32Image()` 从 `web/platform/sd-card-image.js` 导出一张默认大小的测试卡。从固件目录所列官方地址下载镜像并核对 SHA-256。按原生收据的命令和注入时间复跑。浏览器默认 3D 视图可点左侧 A、中间 B 和右侧 C。等待串口输出 `Read from file: Hello world`，再检查每次按键后的新增输出和真实帧缓冲。
