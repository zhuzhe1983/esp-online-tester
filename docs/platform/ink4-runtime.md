# 普通 Metalio INK4 首批运行支持

2026-10-27。普通 INK4 使用独立 `metalio-ink4` 板型；官方 2.0.53 已从远程地址下载、校验、通过 S3 ROM 启动，在实际浏览器 WASM 中显示 480×800 主页，并通过触摸进入设置。目录从 unavailable 改为 partial，官方稳定版保持默认；两条完整官方镜像均可确认后尝试运行。INK4 Plus 的 S31 仍保持独立、不可运行，不使用此板型代替。

依据是 [固定厂家源码](https://github.com/CloudZao/Metalio-E-INK4/tree/d12f481f4a42420634bb56a928f3959a977367d4) 的 config.h、IOExpander.hpp、metalio_touch.c、显示驱动和 LUT，及 TI TCA9555 SCPS200E、SSD1677 rev1.0、Espressif CST816S component 1.0.6。实际抓取 URL、时间与 hash 见 [来源收据](evidence/ink4-runtime/sources.json)。

实现范围：

- SSD1677 SPI 实际 MOSI8/SCLK14/CS45、DC13/RESET18、BUSY9，800×480 原生 RAM 转为竖屏480×800。支持厂家黑白全刷/局刷数字端点。0xCC 是 VSH2 黑色维持波形，不能误当 field-free，也不能套用 M5GFX 四灰阶映射。光学波形、温度、残影与精确刷新时间未模拟。
- SDA41/SCL42 上的 TCA9555 0x20，成对寄存器自增、方向/极性/输出锁存与逐端口读清中断；GPIO2 中断，P0.5/P0.6 电源、P1.1 触摸复位。输入上拉不冒充输出电源。
- CST816S 0x15、GPIO1 中断、02h–06h触摸报告，A5h睡眠及硬复位。未公开确认的芯片 ID 保持0；自动待机、扫描时序与模拟阈值未实现。
- GPIO3电源键；音量键经 TCA9555 P1.0/P0.7，盖板键经 CST816S 固定坐标，使用语义 `board-button`，不会伪造CPU GPIO。鼠标面板、键盘和 agent 共用输入。真实固件完整长短按/录制验收继续进行，不能仅凭队列回执认定行为通过。侧键 CAD 物理对应仍未校准，保持原有不绑定限制。
- microSD 1-bit 的物理路由已声明 CLK38/CMD40/D0=39/CD46；当前网页接卡入口尚未开放，存储功能仍待后续运行验收。

运行配置为16 MiB Flash、8 MiB PSRAM。8 MiB 是此次可启动测试配置，**不是已确认的实机PSRAM容量**；固定源码只确认 Octal PSRAM。电量计、RTC、IMU、音频、Wi-Fi/BLE与4G不存在可用模型，缺失I²C器件NACK，不伪造初始化成功。固件仍标记 trial，未实机验证；2.0.63未进行本轮运行验证。

复现：先 `tools/wasm-build.sh` 并准备官方 ROM，再运行：

```sh
node tools/probe-official-ink4.mjs \
  https://cloudzaoai.oss-rg-china-mainland.aliyuncs.com/OTA/xiaozhi/eca923cb-7e07-45d6-9e83-0aefe6e48a46.bin \
  1d02cba522fe1aec5b2c160973c6a15d89cbf0e13a5794a42faf6e3f15587919 output/ink4-wasm
```

探针要求调用者显式提供远程地址与 hash，固件在内存中使用，不写入公开资产。产出仅在 ignored output/；提交的收据不保留原始串口、设备标识或截图。首次运行发现错误灰阶分类造成反色，按固定LUT修正后黑白主页/设置页可用；设置页标题仍有叠字，未断言光学完全一致。

本阶段双 Clippy、1393项 Rust CI 测试（include-ignored / skip external）、90项平台 Node 测试、11个真实WASM demo、既有真实录制回归通过，黄金输出未更新。浏览器从实际运行按钮加载远程官方镜像，30.12客体秒、1,206,711,719指令、4帧，无runtime错误。详见 [检查](evidence/ink4-runtime/checks.json) 和 [官方WASM](evidence/ink4-runtime/official-wasm.json)。新控制器完整录制、3D盖板命中与SD继续交付，未计为此阶段通过。

CPU 对照：相对 df198a74 同机 release 二进制，S3/C3/C6 hello各交替五对，客体30秒；user中位数分别0.281033→0.285103秒（+1.448%）、2.598123→2.627946秒（+1.148%）、1.833083→1.820878秒（−0.666%）。各组指令数和stdout SHA完全一致；样本较短，不据此宣称加速或零成本。基线/候选二进制与输入hash、命令和全部样本见 [CPU收据](evidence/ink4-runtime/cpu.json)。测量时所有协作任务无编译/客体/浏览器负载，保留用户静态服务器。未修改S3核心，无Pocket对照。最初解析器误将wall时间纳入指令串、C3命令误带不支持的PSRAM选项；两次未形成完整组的尝试丢弃，仅保留修正后完整五对。


后续触摸/UI 修正：目录补齐 `runtime.touch`，修复浏览器3D/2D输入开关与已实现触摸器件不一致。透明盖板印刷的现有 alpha-aware 命中逻辑实测正常，未修改。三个盖板键按保留照片印刷位置增加独立圆形/胶囊命中区，CST816S原生坐标仍由板型发送；CAD三角形、位置与印刷纹理不变，四个重建文件逐字节一致。六枚备用按钮改成中文，超过三枚时收进「按键」菜单；工作台和480px mini无水平溢出。

真实鼠标点3D设置图标产生客体(392,675)按下/释放，官方进入SettingsScreen；点击橙色主页盖板键，官方处理vk_home并回到主页。新探针同时采集USB控制台，覆盖音量上键的实际 `volume=80` 日志与刷新输出。首轮Home探针只等1.5客体秒，捕捉到固件已切页但尚未刷新的旧帧；延长观察至其刷新完成后通过。保留 [首轮失败](evidence/ink4-runtime/official-wasm-initial.json)，修正后的 [WASM收据](evidence/ink4-runtime/official-wasm.json) 和 [3D/UI收据](evidence/ink4-runtime/touch-ui.json) 分别记录观察范围。长按Home在设置页会由固件报告no-op，释放后的默认返回才回主页；不把固件本身的菜单规则改成前端强制跳转。新设备录制导出已能保存触摸和语义键事件，但使用旧stats观察时间的浏览器脚本曾在重启初始阶段发送输入，本轮不将其计为有效回放通过。
