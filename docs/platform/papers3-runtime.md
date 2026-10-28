# PaperS3 C139：官方测试页与单点触摸

C139 已启用独立 `papers3` 部分板级模型。固定官方 UserDemo v0.5 从完整镜像经 ROM 启动，显示原程序的测试界面；GT911 触摸能切换静音图标、启动 Wi-Fi 扫描，显式 BMI270 虚拟样本能改变数值。没有修改固件，也没有从客体内存直接抽取应用 framebuffer。灰阶、RTC、声音和独立电源控制器尚未完成，不能据此声称整机兼容。

## 来源与版次

[来源收据](evidence/papers3/sources.json) 固定官方产品页、V1.0 原理图、GT911 Rev09 datasheet、M5PaperS3-UserDemo `5e275ad4b70abb85f7193fda137844730e64c4db`、M5GFX 0.2.15 `c6f92dc03226cdc04d67c705a2020f62ad21ad01` 和 M5Unified 0.2.10 `5580ff6923a868cc71d5b30c962186bde2c85b67`。源码锁定 IDF 5.3.3。额外核对该版本的 lcd_cam_reg.h 与 gdma_reg.h：继承 i80 路径使用的 USER/MISC 偏移、START/DOUT/CMD、位序/字节序、完成中断和 GDMA owner/EOF 位与原 v5.5.1 一致；逐字段结果保存在 [寄存器比对](evidence/papers3/idf-register-compatibility.json)，不据此宣称所有 SDK 行为相同。镜像中的应用描述版本是 `b6abb5e`；当前源码 HEAD 并非该二进制的可复现构建证明，只作为公开接线、输入和 UI 行为依据。

固定官方镜像 1,439,168 字节，SHA-256 `d6733a0ca378f95335fa5fba4d4d992fb1dd97c17557b20e9aebfca08ba6d624`，来自目录既有 M5Burner v0.5。完整合并布局走 ROM，不使用 app-direct；网站仅保存远程链接与摘要。官方出厂固件仍为默认第一项；UIFlow2 和其他匹配固件可尝试，但没有继承本条验证。

## 物理路由与边界

ESP32-S3R8、16 MiB Flash / 8 MiB OPI PSRAM。ET047TC1 原生 960×540，竖置物理 framebuffer 为 540×960；官方程序使用横屏，所以初始模型上文字横向，旋转设备即可阅读。D0…D7 为 G6/14/7/12/9/11/8/10，XCL16、XSTL13、XLE15、SPV17、CKV18。固定 M5GFX 配置区分 G46 升压与 G45 输出使能，两者均须有效；产品页仅列 G45 PWR，不能丢掉 G46。PaperDIY 的 M5PM1 供电不同，不能共用其板型。

提取原 PaperDIY 的 ET047TC1 数字模型为共用器件，不改变原有驱动符号语义。LCD_CAM i80 / GDMA 从真实客体事务获得每行 960 个两位命令；1/2 写黑/白，0/3 保持。并行路由、供电、扫描和锁存决定是否更新。保留掉电后的画面，但没有校准 16 灰阶的多脉冲光学积分。局部刷新与全刷的图标边缘存在差异；不能将边缘差异误判为第二次触摸未释放。

GT911 使用 I²C SDA41/SCL42、IRQ48，模型选择 0x5d 地址；实际地址 strap 没有额外实机测量。16 位寄存器寻址、0x814e ready/count、0x8150 起坐标、写 0 清除和后续扫描通过独立器件实现，支持一个接触点。5 ms 数字扫描周期包含无接触报告，便于驱动丢弃旧报告后读取释放；这不是空闲功耗或中断时序校准。配置区有限 RAM 不代表配置生效；多点、手势、固件升级、休眠与唤醒未实现。原 Waveshare fixture 的旧 GT911 端点不变，防止扩大既有兼容声明。

BMI270 地址 0x68 复用显式样本寄存器子集；agent 可输入虚拟加速度与角速度，未建立实体安装方向与 3D 旋转的映射。BM8563 0x51 明确 NACK，故官方时间字段无效；不能用零填充 RTC 冒充正确时间。SPI TF 的 SCK39/MOSI38/MISO40/CS47 已接入共享 SDHC 卡存储；详见下方 TF 验收。蜂鸣器 G21 只验证固件图标，不输出声音。PMS150G 控制的电源键不伪造为普通 GPIO；虚拟 USB 检测 G5 和充电状态 G4 固定为高，未连接电池模型。电池 ADC、物理电源睡眠、USB 及无线网络未验收，空扫描不等于可联网。

## 验收方式

`tools/probe-papers3-factory.mjs` 接收显式 URL 和 SHA-256，执行实际页面 worker / WASM。测试等待真正的官方应用日志及 framebuffer，再触摸物理坐标 `(116,330)` 两次，验证静音斜线由黑→白→黑；输入 `(225,640)` 后仅检查新产生的扫描日志。检查图标内部稳定区域而非要求整块像素一致：首帧 63 个黑像素、开启后 0、恢复后 49（10×10 区域）。原先整块图标 hash 断言失败，429 个边缘像素不同，视觉检查确认状态已恢复；该负结果保留，不改写为光学精确通过。

[真实固件收据](evidence/papers3/factory-wasm.json) 保存图片摘要、实际输入时间、远程镜像和 WASM 摘要及串口白名单。RTC/TF 错误和芯片标识不全量入库；无私人数据或真实硬件测试。复现命令：

```sh
tools/wasm-build.sh
node tools/probe-papers3-factory.mjs --url '<目录 C139 官方 v0.5 地址>' --sha256 d6733a0ca378f95335fa5fba4d4d992fb1dd97c17557b20e9aebfca08ba6d624
```

厂家 STL、照片纹理和尺寸证据保持原有来源。独立重建七个产物字节一致；不将本次行为验收写成新的 1:1 模型精度认证。缺失 PCB 数据仍为 UNKNOWN 占位，电源键在运行 UI 中不可操作。浏览器、回归和性能的最终数值见同目录收据。

最终 stable 1.99.1 双 Clippy、1435 个 Rust 测试（132 组）、11 个 WASM 场景、100 个平台测试、111126 个 JIT 差分案例及 24 个 GPIO 波形通过；目录 schema、时序/BLE、worker、benchmark 策略和保留 x5 收据检查均通过。黄金文件没有修改。18 个真实录制输入的生命周期回归通过，但最后帧不同，不宣称逐帧回放一致。[完整检查](evidence/papers3/checks.json) 与 [浏览器收据](evidence/papers3/browser.json) 保留命令、源码和产物摘要。

[CPU 对照](evidence/papers3/cpu.json) 使用固定主线 `84e67571` 的独立保全二进制，关闭本任务浏览器/客体并协调其他任务后执行五对交替运行：hello 0.25→0.25 秒（+0.000%）；c3-hello 2.34→2.42 秒（+3.419%）；c6-hello 1.56→1.62 秒（+3.846%）。每组 UART 摘要和指令数完全一致；样本量小且计时粒度有限，没有置信区间，不据此保证严格零开销。未修改 S3 CPU 执行核心，本轮没有重跑 pocket-tank。

## SPI TF 与浏览器存储

PaperS3 的卡槽走 SPI，不是 PaperMono 的原生 SDMMC。V1.0 原理图 J3 的 `SOC_VDD` 供电与官方引脚表一致；模型检查 SCK39/MOSI38/MISO40/CS47，支持硬件 SPI2/3 的既有路由，错误脚和未插卡返回高电平。没有新增虚构的 GPIO 电源开关。

默认建立 64 MiB FAT32 卡。工作台和 mini 页点击「设备存储」即可导入、导出或清空 TF 镜像；数据仍按产品、板型和原始完整 Flash 摘要隔离。reset、相同固件重载和正常页面重载保留数据，清空/导入 TF 不清 Flash。带私人 SD 改动的录制继续拒绝导出；不会把文件放进分享链接。

一份用户提供的原始完整镜像（输入 SHA、大小见 [收据](evidence/papers3-tf/private-firmware-result.json)）在修复前停于 CMD0、报告 SD 未检测到；修复后原生与真实页面 WASM 均成功挂载，客体写出 2,389 字节、跨 5 个簇的文件。reset、新 worker 和 IndexedDB 页面重载后文件及整卡 SHA 一致，mini 重新挂载同一镜像。该私人固件、文件名/内容、源代码、完整串口和画面不公开；可用 [调用方输入探针](../../tools/probe-papers3-sd.mjs) 在本机复核，不修改镜像。

这是 SPI **SDHC 即时数字子集**：命令/响应、CRC7/CRC16、单/多块读写与停止令牌复用既有块存储。没有校准供电/编程忙时间、模拟信号或物理拔插，也未支持 SDSC、任意半双工驱动和全部 SD 标准命令。不据此声明整个私人应用、灰阶显示或网络可用。来源、负例、浏览器和 CPU 检查见 [证据说明](evidence/papers3-tf/README.md)。
