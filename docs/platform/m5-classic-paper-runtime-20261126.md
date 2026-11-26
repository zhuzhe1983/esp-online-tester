# Paper v1.1 / CoreInk 当前 D67 批次的真实运行子集

> 历史批次记录：下文“未实现”等状态只描述当时快照。当前 Paper 三SKU 已补 GT911 单触点、SD SPI 数字传输子集；详见[当前运行边界](m5-classic-completion-runtime-20261126.md)和[最终13SKU生产Worker回执](evidence/m5-final-integration-20261126/classic-thirteen.json)。CoreInk/Paper/K146 的本次实际3D键面映射与外观增量另见[五SKU控制证据](evidence/m5-control-caps-five-20261126/README.md)。这些更新不构成厂家完整固件或实机验收。

这次补齐两个独立 ESP32 板型的显示、按键和 RTC 子集。测试中的每个 GPIO、SPI 与 I²C 写入及输入读取都由真实 LX6 `s32i/l32i` 指令经过 SoC 总线执行；没有前端代画像素、串口注入按键或相近板型别名。两项都保持 `partial`，厂家完整固件仍未验证。

| 产品 | 板型与已实现能力 | 明确边界 |
| --- | --- | --- |
| Paper v1.1 K049-B | `m5paper-v11`；IT8951E 原生960×540，按物理竖屏540×960发布；SPI14/12/13、CS15、HRDY27，G23真实EPD电源门；4/8位灰阶RAM、区域0x37刷新、四种载入旋转；G37/38/39与I²C21/22上的BM8563日历 | GT911触摸、SHT30、EEPROM和SD传输未实现。RTC alarm/wake、电池/主供电关闭、模拟波形/温度/残影、完整厂家固件启动未验证 |
| CoreInk K048 | `coreink-d67`；仅当前GDEY0154D67/SSD1681批次，200×200；SPI18/23、CS9/DC15/RST0/BUSY4；字节窗口、RAM、更新控制/刷新、休眠；G37/38/39/5/27和BM8563日历 | 旧M09命令流明确不兼容。LED/蜂鸣器、主供电关闭、RTC alarm/wake及模拟波形未实现；侧边3D点击区域仍待几何校准，可使用控制面板/agent的实际GPIO输入 |

CoreInk 官网现为 **GDEY0154D67**，历史入库资料为 **GDEW0154M09**。官方 M5GFX 固定提交同时列出两种控制器选择，当前默认D67。前者使用0x24写RAM、0x22/0x20刷新且BUSY高有效；后者使用0x10/0x13/0x12、BUSY低有效。新板型不把M09流自动翻译成D67，也不推断用户手中设备的批次。目录屏幕与限制同步纠正，历史知识事实和回执保留。

Paper 的G23在M5EPD中明确为 `EPD_PWR_EN`，通用M5GFX元数据把它命名reset。本实现采用具体SDK的电源门，断电清掉易失RAM并保留电子纸画面。GPIO2/12主电源保持只是观测，不强行模拟USB供电下的关机。当前K049-B文档将G37称Right/G39称Left，旧M5EPD头文件反向命名；目录采用当前精确SKU文档并公开GPIO。IT8951回读使用明确 `SIMULATED` 身份，固定图像地址0x001236E0来自SDK合同。

保存的[固定提交来源、原始URL、UTC时间、大小与SHA-256](evidence/m5-classic-paper-runtime/receipt.json)包含M5EPD显示驱动/引脚、M5GFX两款板配置及D67驱动、CoreInk RTC源码。副本只供离线协议核查，保留源码版权和许可证；不含第三方固件二进制。当前产品页原始HTML仅保留真实下载回执，源码与结构化结论可离线复核。更多逐设备能力与冲突见[机器可读报告](research/m5-classic-paper-runtime.json)。

离线验证（使用当前 stable Rust，设置共享 `CARGO_TARGET_DIR` 时勿并行占用同一锁）：

```sh
cargo test -p esp32 --test m5_classic_paper
cargo test -p esp-soc it8951 --lib
cargo clippy -p esp32 --test m5_classic_paper -- -D warnings
node --test tools/platform-m5-classic-paper-runtime-test.mjs
python tools/validate-platform-catalog.py
node tools/platform-knowledge-test.mjs
```

LX6客体断言涵盖四向非对称灰阶像素和物理竖屏坐标、区域之外白色保留、连续CS回读、BUSY期限、错误RAM地址/越界窗口/MISO/IO_MUX/共享SD片选拒绝、EPD电源断开和RAM丢失、CoreInk硬复位/深睡/M09负例、MCU重启光学保留、每只GPIO按下释放及RTC闰日跨越、错误I²C引脚/地址NACK。共享IT8951既有五个reTerminal测试保持通过。20ms刷新/1ms上电为命名的虚拟时序，不能视为物理刷新速度测量。

官方出厂条目仍分别为Paper v2.1和CoreInk v1.0.1，`simulationVerified:false`保持不变。CoreInk出厂镜像属于哪种屏幕批次尚未建立证据。本轮不把协议fixture称为完整厂家镜像运行，也不把已有模型照片验证升级为整机1:1或实机验收。浏览器WASM构建/完整目录联调由批次集成执行。

本工作目录的config/input/view检查通过；`platform-runtime-test`在缺少本轮生成的 `web/wasm/esp32sim.wasm` 处停止，未得到行为通过结论。集成后须重建WASM并重跑，不能用旧二进制代替本次Rust实现。

按共享协议变更要求，通过GitHub只读查询核对仓库全部open PR（第一页上限100），返回空列表，未发现正在审阅的同文件实现；原始空列表与查询范围/时间/摘要已记录在来源回执中。这是当时的查询结果。
