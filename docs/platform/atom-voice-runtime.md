# Atom Voice C008-C：来源检查点

当前产品页称 Atom Voice，官方源码与结构目录仍使用 AtomEcho；SKU 都是 C008-C。资料固定到 M5-ProductExampleCodes `1bc38736e61a362080d1dffc4020b3a2a1dff6ae`，不是以名称相近的 Atom Lite 代替本产品。

ESP32-PICO-D4 / 4 MB Flash、G39 用户键、G27 SK6812、Grove G26/G32；音频线路为 NS4168 DATA22/BCLK19/LRCK33 与 SPM1423 PDM CLK33/DATA23。官网底部引脚表说明音频脚为内部保留，但原理图链接实际是功能框图，不含完整 PCB 布局；未把框图当 PCB CAD。

[来源收据](evidence/atom-voice/sources.json) 保留官方页、框图、两颗音频器件 datasheet、固定源码和原始官方镜像的地址、抓取时间及摘要。M5Burner 出厂 v1.0.0 与固定官方源码目录中的 `Atom_echo_a2dp_led_0x00.bin` **逐字节相同**：2,097,152 字节，SHA-256 `fa9a4c4491a83fa5b60310ee35b14e3cef71bfeac5e1cd0883b84448f6923951`。二进制/PDF 仅在 ignored output 保留，不发布到 Pages。

固定出厂源码配置 44.1 kHz、16 bit I²S TX，G27 RMT 先写 `0x000500`（GRB 红色），随后初始化 classic Bluetooth/A2DP。源码没有读取 G39，因此不能要求按键使这一份固件改变颜色，也不能把红灯当作音频或配对成功。Classic BT、A2DP、麦克风与实际扬声器播放都需要独立验证。

本检查点尚未解锁目录。下一步建立独立 `atom-voice` 板型，仅挂接经核对的 G39/G27 数字路径，再运行原始官方镜像，保留其真实停止位置和外设缺项。基于已交付的 Atom-Lite 运行基础 `a06f4a84`、`6524e2e5`，不复用 Lite 的产品身份或固件兼容结论。
