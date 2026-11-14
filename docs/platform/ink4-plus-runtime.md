# Metalio INK4 Plus 运行支持

INK4 Plus 使用独立板型 `metalio-ink4-plus`，不能复用普通 INK4 的 ESP32-S3、SPI 屏幕或 CST816S 触控配置。当前板级接线依据 CloudZao 官方源码 `ad8edf4150a688737c0d7c00cae4413079b262ef`（2.0.68）与 `14aa1d06d15a46435f234aa14f3cffa478ed2802`（2.0.79）：XSTL 为 GPIO4、MODE 为 GPIO19。该源码不能证明所有实物硬件版次都采用相同接线。

## 运行配置与真实器件路径

- Flash 为官方注明的 16 MiB。公开资料仅确认 Octal PSRAM、200 MHz，未给出容量；运行配置采用 **16 MiB 模拟 PSRAM**，知识库中的实物容量仍为未知。通用 NOR JEDEC、模拟电源读数及空白 eFuse 不是实物读取结果，也不继承 Mosaico 的身份或密钥配置。
- ROM 启动使用 S31 rev0 ROM、chip ID 32、bootloader `0x2000`、分区表 `0x8000`；官方完整镜像中的应用位于 `0x80000`。浏览器使用生产 S31 WASM 路径。
- 屏幕通过 S31 LCD RGB、AXI DMA 与 RMT 门极时钟运行。D0–D7 为 GPIO8–15 的专用功能，PCLK16、HSYNC17、DE4、SPV35、CKV40；XOE18 与 MODE19 都必须输出高电平。客体填写双缓冲描述符，EOF 中断允许客体回填，完成扫描后才送出真实波形字节。
- 物理像素为 1216×684，工作台显示为 684×1216 竖屏。2.0.68 的 LCD 数据是 305×688，包含行首占位字节与补齐行；2.0.79 是 304×684，无占位。两种布局分别解码，不能互相套用。
- I²C1 仅在真实 SDA0/SCL1 路由有效时访问设备。TCA9555 `0x20` 控制 TPS65185 `0x68` 的 WAKEUP/PWRUP/VCOM 和屏幕电源；PWRGOOD 跟随控制输出。温度固定为名义 25 °C，不模拟模拟升压过程或故障。
- FT6336U `0x38` 使用 GPIO5 中断和 TCA P12 复位，提供官方 v04 应用的单点报告协议；不执行触控芯片 MCU 固件或升级器。电源、音量加、音量减分别进入 TCA P17/P10/P11；主页、上一页、下一页进入屏外触控区域。它们没有伪造的 ESP GPIO 编号。
- SDMMC slot0 使用专用 GPIO20–25，客体命令与 IDMAC 读写落到同一张镜像。首次运行准备 64 MiB FAT32 卡，已有卡数据保留。控制器复位保留外部卡，平台沿用导入、导出与持久化接口。
- HP UART1–3 使用各自寄存器、时钟和中断，未连接端口保持无 RX 数据；不伪造 BH1098G 蓝牙音频处理器或 NT26 调制解调器的 AT 回复。

## 已观察的固件行为

默认固定官方 **2.0.79**，保留 `trial`、`simulationVerified: false`，不把有限功能验收当作全部应用或实机兼容。官方源经浏览器直接下载，镜像未修改，未镜像到本仓库。原 2.0.68 的 URL/hash 仍保留在历史资料和本次浏览器回执中。

| 固件 | 实际执行与观察 | 尚未据此验证 |
| --- | --- | --- |
| 2.0.68 | ROM 启动、684×1216 主页；触摸进入设置；Home/PREV 返回；NEXT 单独触发显示测试；侧键把音量设置从 70 改到 80 再回 70 | 音量设置不等于声音播放；显示测试只有黑白端点，不能验收 16 级灰度 |
| 2.0.68 | 固件挂载 64 MiB FAT32，真实 SDMMC 读写；浏览器保存 26 个修改页，重新配置并载入的新 worker 恢复完全相同卡镜像 SHA-256 | 没有私有 SD 文件或物理卡时序验证 |
| 2.0.79 | 最终双使能门控 WASM 从官方源下载并启动；原生 304×684 波形产生竖屏主页；设置触控改变画面，SD 挂载成功并写入 9 页 | 没有将 2.0.68 的全部输入测试自动继承给新版 |

电源键通过 TCA9555 的电气按下/释放已接入；显示测试中的短按未产生已归因的应用反应，所以仍未验收它的应用功能。浏览器输入按实际客体时间保持，不能用排队成功当作执行证据。

初期负结果也保留：缺失 I²C 路由时 EPD 初始化失败；LCD/SD 接入后能显示主页，却因 UART1 `REG_UPDATE`/时钟未实现而停在音频初始化。修复四路 HP UART 的独立寄存器和真实时钟后进入应用任务。外部 BH1098G 仍无响应，固件会输出 `mode_ack` 重试；未伪造回复绕过它。

[最终原生验收](evidence/ink4-plus-runtime-20261115/native-final.json)使用最终双门控构建：触摸 `(545,1015)` 后出现 `SettingsScreen: settings ready tabs=12`，LCD 206 次扫描、0 faults；`RMT 0 transfers` 是完成的 RMT 包计数，不表示门极扫描未运行。[浏览器验收](evidence/ink4-plus-browser/README.md)保留各版本输入、存储、原始截图 hash 和未验证范围。

## 边界

墨水屏当前提供黑白光学端点：驱动码 `01` 黑、`10` 白、`00/11` 保持原状态。没有标定多相脉冲积分、16 级灰度、残影、模拟电压或精确 FIFO 欠载时序。断电保留已显示像素。

BQ27220 为名义电池模型。PCF8563、SC7A20H、充电器、相机、前光亮度、振动执行器、外置音频与蜂窝模块未完成整机验证。S31 的 STA 中继能力不代表 SoftAP、真实射频或产品云服务已经兼容；不提供虚构的联网与声音结果。未进行实机验证。

## 来源与复现

[硬件与镜像审计](evidence/ink4-plus-runtime-20261115/hardware-source-audit.json)记录引脚、寄存器、IRQ、镜像布局、版本差异与未知项；同目录保留官方源码 URL、固定提交、抓取 UTC 和 SHA-256。寄存器依据 ESP-IDF v6.1 `fff9895c82d744c7237be8847347bdd1b07c6643`；官方 BIN 自报 `v6.1-dirty`，公开源码与该标签之外的所有修改未被完整识别。

固件始终来自发布方或用户本地文件，不把第三方 BIN 放入 Git 或 Pages。官方发布元数据保留公开产品/版本字段，删除账户、作者头像与无关目录信息；原响应与筛选结果的 hash 见 [元数据溯源](evidence/ink4-plus-runtime-20261115/official-firmware-metadata-receipt.json)。

```sh
cargo build --release -p esp32sim --bin esp32sim-s31
target/release/esp32sim-s31 --board metalio-ink4-plus --boot rom \
  --flash-image /path/to/official-complete.bin \
  --rom roms/esp32s31_rev0_rom.elf --sd /path/to/card.img \
  --max-seconds 30 --console usb --tft-png /tmp/ink4-plus.png
```

`--psram-mb` 可显式改变模拟配置，不改变知识库中的物理容量结论。使用私有 Flash 或 TF 镜像时，不将内容放入公共验证资料。
