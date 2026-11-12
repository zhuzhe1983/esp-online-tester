# PaperS3 BM8563 数字日历

PaperS3 C139 的 BM8563 位于 I²C `0x51`，SDA41/SCL42。共享器件实现客体写入的 BCD 日期/时间、按客体周期推进的秒钟、月末及四年闰年进位、星期和世纪位翻转，以及 STOP 和 VL 位。它不读取宿主时钟，也不向固件填入预设的有效日期。官方 UserDemo v0.5 自行写入 `2077-01-01 12:00:00`；该值来自原程序。

寄存器依据见 [来源收据](evidence/papers3-rtc/sources.json)：BM8563 官方资料副本第 6–10 页、NXP PCF8563 Rev 11.1 的 8.4–8.5 和 8.10 节，以及固定 M5Unified `5580ff6` 的 PCF8563 驱动。板级型号仍是 BM8563：采用其复位值（TF=1、CLKOUT 禁用），不复制 NXP 不同的上电默认值。BM 资料秒字段一处写作 00–99，与寄存器概览及示例不一致，本模型按普通秒钟的 00–59 范围推进。世纪位在 99→00 时翻转；闰年按两位年数能否被 4 整除决定，包含 00，不擅自改成 Gregorian 世纪例外。

上电寄存器日/月为零，VL=1；直到客体写入有效日期前，不推进无效日历。VL 仅通过客体写入秒寄存器清除，STOP 不清 VL。非法 BCD 或日期保留读回，跳过日历运算，避免偷偷修正输入或触发越界。STOP 停止计数并清除分频相位；恢复后采用确定性的整一秒起步，**不模拟实物约半秒的分频器首个边沿和晶振相位**。

读写期间冻结日历寄存器，最多积存一个秒进位，STOP 后再执行；重复 START 保持本次读取的一致性。初次真实固件验收暴露了软件 I²C STOP 的既有遗漏：固定 M5GFX 用 GPIO 结束硬件 I²C 读取，但软件总线没有保存硬件控制器选中的器件，导致 RTC 永远保持访问锁，显示停在 `12:00:09`。见 [真实画面负结果](evidence/papers3-rtc/initial-display-negative.json) 和 [原生寄存器读回](evidence/papers3-rtc/native-lock-negative.json)。修复使物理 STOP 通知总线上的全部器件，重复 START 不再伪装成 STOP；没有取消 RTC 的一致性锁。

目前不实现 alarm/timer/INT、CLKOUT 引脚输出、外部晶振测试输入、模拟掉电、电池域掉电保持或 PMS150G 电源控制。寄存器可读写不代表相关硬件输出已实现。软件复位或重新加载固件会新建板级 RTC；不能据此声明电池备份或睡眠唤醒兼容。

定向回归：

```sh
cargo test -p esp-soc bm8563 --lib
cargo test -p esp-soc i2c_bitbang --lib
cargo test -p esp32s3 papers3 --lib
```

最终候选 WASM `8d8ccce696666eeb7a355cdbe15fd398eaec9ddb9ae9ade85544d91639b4fb20` 已通过真实官方 v0.5 时钟显示验收：同一次 ROM 启动，客体时间 `30.02` → `33.02` 秒，真实 540×960 framebuffer 上的时间由 `12:00:29` → `12:00:32`，日期保持官方程序设置的 `2077/01/01`。两张画面均人工检查，并在[通过收据](evidence/papers3-rtc/factory-clock-pass.json)保留原始像素及 PNG 摘要。修复前的冻结负例仍保留，没有用总帧摘要改变替代数字递增。

复现使用真实页面 worker 的探针；镜像只在内存中下载，`--actions` 文件包含先等待 3 秒、再截图和退出的动作（完整动作见通过收据）：

```sh
node tools/probe-papers3-edcbook.mjs --url 'https://m5burner-cdn.m5stack.com/firmware/16287f6984f8ec7fe8058a6c3497b3b4.bin' --sha256 d6733a0ca378f95335fa5fba4d4d992fb1dd97c17557b20e9aebfca08ba6d624 --output <local-output> --actions <clock-actions.json>
```

探针名称面向下一阶段 EDCBook，但本条运行的源 URL、SHA 和画面全部属于官方 v0.5，不能据此声称 EDCBook 通过。

同一最终产物的 [factory 回归](evidence/papers3-rtc/factory-regression.json) 也通过真实测试页、两次 GT911 静音图标切换、显式 BMI270 样本、扫描任务及 TF mount/write/reset/reload。本次扫描只尝试一次短按，尝试次数保留在收据；旧短按漏触负例仍在 [TF 证据目录](evidence/papers3-factory-tf/README.md)。最终产物内三次完整卡摘要一致，历史无 RTC 产物的卡摘要保持原样，不跨产物要求相同。
