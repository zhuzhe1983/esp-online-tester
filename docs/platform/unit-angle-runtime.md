# Unit Angle U005 的模拟电压输入

支持 **U005 → StopWatch Port A 白线 G11 → ESP32-S3 ADC2_CH0**。旋钮、面板、agent 和回放提交同一个归一化位置；板级模型按供电状态产生电压，客体自己的 ADC 驱动完成转换。未直接替换固件的变量、ADC 返回值或串口输出。

## 本轮采用的电路资料

[U005 官网](https://docs.m5stack.com/en/unit/Unit_Angle)列出 10kΩ 电位器、5V 供电、白线模拟输出、黄线 NC，标称输出 0–2500mV。官方推荐的旧主机 Port B 不是信号协议；本轮使用经[StopWatch 原理图](https://m5stack-doc.oss-cn-shenzhen.aliyuncs.com/1242/C152-SCH_Stopwatch_PRJ_Main_VA_20251201_2026_04_24_17_46_22.pdf)核对的白线 G11。[ESP-IDF v5.5.1 映射头文件](https://raw.githubusercontent.com/espressif/esp-idf/v5.5.1/components/soc/esp32s3/include/soc/adc_channel.h)确认 G11 是 ADC2_CH0。

厂家两张电路图存在冲突：[第一张](https://m5stack-doc.oss-cn-shenzhen.aliyuncs.com/800/U005_Sche.jpg)标 R1=4.7kΩ，[第二张](https://static-cdn.m5stack.com/resource/docs/products/unit/angle/angle_sch_02.webp)标 R1=10kΩ，并写出 `Uo=5000mV×Rp/(Rp+R1)`。三端电位器的绘法与这个随 Rp 变化的公式也不完全一致。没有硬件版次、实测或 PCB 数据可消除冲突。

因此模型名明确为 `potentiometer-u005-10k-curve`，采用**厂家公布的 10k 标称曲线**，`value=Rp/R2`，范围 0–1。它不是所有 U005 实物的电路鉴定，也不是机械角度。0、0.25、0.5、0.75、1 分别给出约 0、1000、1667、2143、2500mV。抓取时间、最终地址、媒体类型、字节数及哈希保存在 [sources.json](evidence/m5stack-unit-angle-u005/runtime/sources.json)。

## 操作与生命周期

```js
scene.accessories = [{
  id:'angle-unit', catalogId:'m5stack-unit-angle-u005',
  parameters:{position:0.5},
  wiring:{mode:'physical',interfaceId:'grove'},
}];
await espTester.configure(scene);
await espTester.loadFirmware();
await espTester.start();
await espTester.input({type:'accessory-angle',accessoryId:'angle-unit',id:'knob',value:0.75});
await espTester.input({type:'accessory-connection',accessoryId:'angle-unit',connected:false});
```

`observe().accessoryInputs` 中的 `value` 是输入位置，不能作为客体已读取 ADC 的证据；固件输出或转换观测才是读取证据。界面显示“供电时标称”电压，未伪称测量值。`accessory-positions` 发布输入状态，录制保存语义事件和客体时间确认。

固件必须通过 M5PM1 `PWR_CFG 0x06` bit 3 开启 Grove5V；`HOLD_CFG 0x07` bit 6 决定关机后是否保持供电。关闭电源或拔出配件后不再驱动标称电压，本轮把未驱动输入按 0V 处理。拔出期间可旋转，重接后恢复当前位置；暂停时仅允许拔出，旋转输入返回 `NOT_RUNNING`。

客体 `esp_restart()` 保留物理旋钮位置和连接状态，PMIC 重新初始化后需固件再开电源。平台 `reset()` 恢复场景的 `parameters.position` 和连接。一个 Grove 插口只能有一个物理附件；其他主机、任意引脚和旧 UART 接线格式不能绕过路由校验。GPS 的逻辑注入仍不代表物理连接。

## 模型和限制

外壳与已有派生旋钮几何没有更换。公开 `python tools/rebuild-unit-library.py m5stack-unit-angle-u005 --scratch output/u005-rebuild-complete` 可重建；几何二进制逐字节不变，默认位置 0.5 对应原来的姿态。旧照片叠图和尺寸记录原样保留，没有冒充新一轮实测。原正面轮廓 IoU 0.94510 未达到 0.95 门槛，这项失败仍保留，外观完整 1:1 验收仍为 partial。[重建记录](evidence/m5stack-unit-angle-u005/runtime/model-rebuild.json)同时记录首次重建遗漏参考照片的问题和修正。

可拖动的旋钮显示蓝色浅光晕。270° 的显示行程及顺时针增大是操作约定，未核实实物机械行程、方向或电位器阻值规律。内部 PCB 继续使用 UNKNOWN 占位。尚未模拟源阻抗、RC、触点噪声、悬空电压、ADC 争用或实物校准；既有 S3 ADC 模型采用瞬时转换和合成校准参数。

## 验证

[内部源码 fixture](../../tools/fixtures/unit-angle-firmware/README.md)使用 ESP-IDF 5.5.1 的 ADC one-shot 和 calibration 驱动，经 ROM/bootloader/WASM 运行。五点读数、错误 G10/G12 通道、非法输入、供电、拔插、暂停、HOLD5V、两种重启和录制回放共十组通过，见 [fixture.json](evidence/m5stack-unit-angle-u005/runtime/fixture.json)。首次探针误以为暂停时能旋转，失败及修正保留在 [initial-probe.json](evidence/m5stack-unit-angle-u005/runtime/initial-probe.json)。BIN 只在 ignored output 内，不加入线上固件库。

浏览器实际鼠标拖动把位置从 0.25 改为 0.535387，固件读数从 1000mV 改为 1742mV；滑块最大值读出 2499mV，拔出后为 0。2D 面板和 mini 页面也通过，截图和原始数值见 [browser.json](evidence/m5stack-unit-angle-u005/runtime/browser.json)。

本项复用 EX207 的 ADC 转换与 EX215 的共享模拟输入，只增加冷输入解析和已有 StopWatch 电源同步中的可选状态。新 `AnalogStream.replace_current()` 清除过时的排队样本，不改变采样分支；没有新通用逐 tick 服务。CPU 对照以 `69ad0d35` 为基线，前两项 U027/C6 +1.875%、U003/Pocket Tank +1.68% 的历史记录保留，不由本轮覆盖。

每个负载先排除一对暖机，再交替测五对，各运行 30 秒客体时间。所有样本指令数及客体输出哈希相同；[完整样本](evidence/m5stack-unit-angle-u005/runtime/cpu.json)保留 CPU 值、输入及二进制哈希。

| 负载 | 基线 CPU 秒 | 本项 CPU 秒 | 变化 | 本项较慢对数 |
| --- | ---: | ---: | ---: | ---: |
| S3 hello | 0.26 | 0.27 | +3.85% | 2/5 |
| C3 hello | 2.49 | 2.48 | −0.40% | 2/5 |
| C6 hello | 1.67 | 1.69 | +1.20% | 5/5 |
| Pocket Tank | 42.09 | 42.05 | −0.095% | 2/5 |

S3 hello 的 0.01 秒差值与计时输出粒度相同，不能据此声称精确的百分比成本。C6 五对都较慢，+1.20% 尚未归因，不声明性能无回退。Pocket 测量期间共享机器有两次约 1–2 秒的 CAD 读取，负载未完全隔离。本轮没有无改动重复计时，也没有选择性丢弃较慢样本。

复现时先获取公开 ROM/四个既有负载，用独立目录分别在 `69ad0d35` 和当前提交执行 `cargo build --release --bins`，然后从当前目录运行：

```sh
python3 docs/platform/evidence/m5stack-unit-angle-u005/runtime/bench.py --base-bins <baseline-target/release> --candidate-bins target/release --rom-dir web/wasm/fw --receipt output/u005-recheck.json
```

两项 Clippy、1367 项 Rust 测试、72 项平台 Node 测试、严格知识 JSON Schema、目录检查、11 组既有 WASM 回归、U027/U003 各九组真实 WASM 回归通过。[checks.json](evidence/m5stack-unit-angle-u005/runtime/checks.json)列出命令、输入和失败修正记录。
