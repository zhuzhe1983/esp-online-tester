# Unit RGB U003 的真实灯光输出

支持 **U003 输入口 → StopWatch Port A**：固件的 RMT 外设通过 GPIO matrix 把数据输出到黄线 G10，三颗 SK6812 的 GRB 数据解码后驱动配件面板和 3D 透光件。没有前端设置灯色的接口，也没有把串口日志当作 LED 输出。

## 来源与边界

[U003 产品资料](https://docs.m5stack.com/en/unit/Unit_RGB)标注三颗 SK6812（3535）、5 V、黄线 LED Signal；[V1.0 原理图](https://m5stack-doc.oss-cn-shenzhen.aliyuncs.com/760/U003-UNIT_RGB-SCHE.pdf)日期为 2018-04-18，J2.2 → LED3 → LED2 → LED1 → J1.2，输入有 4.7 kΩ 上拉。官网把白线标为 NC，旧原理图则将两端白线连接为 DI；本轮不模拟白线回传和串级出口。用户实物版次不能据此推定。

[StopWatch 原理图](https://m5stack-doc.oss-cn-shenzhen.aliyuncs.com/1242/C152-SCH_Stopwatch_PRJ_Main_VA_20251201_2026_04_24_17_46_22.pdf)确认 Port A 黄线 G10、白线 G11。[M5PM1 数据手册](https://m5stack-doc.oss-cn-shenzhen.aliyuncs.com/1207/M5PM1_Datasheet_EN.pdf)的 PWR_CFG `0x06` bit 3 控制 5 V，HOLD_CFG `0x07` bit 6 控制关机保持。实际 GET 时间、媒体类型、最终地址和 SHA-256 见 [sources.json](evidence/m5stack-unit-rgb-u003/runtime/sources.json)。

实现采用数字供电门控和现有 RMT 解码；供电在已完成帧交付时判断，跨电源切换的半帧时序尚未模拟。不计算上拉模拟电压、电流、光谱、PWM 光学亮度或寄生供电。软件 GPIO 脉冲未实现，不会为此开启全板逐指令调度。其他主机、串级、RGBW 和不同 Unit SKU 不在本项支持范围内。

## 配置和输出

```js
scene.accessories = [{
  id:'rgb-unit', catalogId:'m5stack-unit-rgb-u003', parameters:{},
  wiring:{mode:'physical',interfaceId:'grove'},
}];
await espTester.configure(scene);
await espTester.loadFirmware();
await espTester.start();
const output = (await espTester.observe()).accessoryOutputs;
await espTester.input({type:'accessory-connection',accessoryId:'rgb-unit',connected:false});
```

每个输出记录包含 `accessoryId`、`model`、`port` 和链序排列的三个 `[r,g,b]`；更新通过 `accessory-outputs` 事件发布。一个 Grove 插口只能连接一个物理附件，不能同时插 U027 和 U003。逻辑 GPS 注入不因此变成真实电气连接。

断电或拔出立即清黑；断开期间收到的帧丢弃，恢复连接和供电后仍需固件重新发送。客体 `esp_restart()` 保留物理连接状态，但 PMIC 重新初始化会关闭电源；平台 `reset()` 恢复场景连接。暂停时允许拔出，恢复执行后消耗该事件。录制只包含拔插事件，不会把 UART 命令录制成灯效。工作台、2D 面板、mini、agent 和分享使用同一通路。

## RMT 解码修正

首轮真实 WASM 测试使用 600 ns 高、600 ns 低表示 1，原来的 `high > low` 将其读成 0，三灯保持黑色。失败保留在 [initial-rmt-failure.json](evidence/m5stack-unit-rgb-u003/runtime/initial-rmt-failure.json)。[SK6812 协议手册](https://cdn-shop.adafruit.com/product-files/1138/SK6812+LED+datasheet+.pdf)第 5/7 页给出这一标称时序；它是协议族资料，不证明 U003 使用该 PDF 所画的 5050 封装。

修正将非零且 `high >= low` 判为 1，保留现有 300/900、400/800、400/850 的 0 和 800/400、800/450 的 1；全低 reset 和零时长不误判成 1。它仍是比值解码策略，并未验证绝对时间窗或所有灯珠版次。经典 ESP32 的八通道隔离测试相应将等长符号期望改为 1；没有更新 golden。

## 模型与证明

原厂壳体和透光件的 4402 个三角面只重新分组，位置/法线多重集逐位不变。公开 `python tools/rebuild-unit-library.py m5stack-unit-rgb-u003 --scratch output/u003-rebuild` 可重建；证明见 [model-rebuild.json](evidence/m5stack-unit-rgb-u003/runtime/model-rebuild.json)。旧尺寸、照片叠图及测量时间保留，未把旧检验说成本次重新测量。

原理图没有器件位置。3D 左到右暂显示链序 0/1/2，这个位置对应尚未实机校准；数值面板保持真实链序。隐藏 PCB 仍为 UNKNOWN。三颗透光件独立于外壳材质，金色或透明壳不改变固件数值。

[内部 ESP-IDF 5.5.1 fixture](../../tools/fixtures/unit-rgb-firmware/README.md)通过 ROM、bootloader、真实 IDF RMT 驱动和 WASM Worker 测试精确颜色、错引脚、供电、拔插、暂停、两种重启、HOLD5V 及录制回放；结果见 [fixture.json](evidence/m5stack-unit-rgb-u003/runtime/fixture.json)。BIN 只放 ignored output，没有加入线上固件库。本轮没有实机光学或电气校准。

CPU 回归以 main `9d072179` 为基线，复用 EX219 的现有 RMT 路径和 EX201 的冷输入接口；既有 U027 相对其旧基线的 C6 +1.875% 未归因记录不变。本项测量与之前的成本分别记录，不相互抵消。

每个负载先排除一对暖机，再交替测量五对，客体运行 30 秒。四个负载的指令数、客体输出 SHA-256 均相同；CPU 中位数如下。

| 负载 | 基线 CPU 秒 | 本次 CPU 秒 | 变化 | 本次较慢的对数 |
| --- | ---: | ---: | ---: | ---: |
| S3 hello | 0.27 | 0.27 | 0.00% | 2/5 |
| C3 hello | 2.97 | 2.71 | −8.75% | 1/5 |
| C6 hello | 2.48 | 2.49 | +0.40% | 4/5 |
| Pocket Tank | 39.18 | 39.84 | +1.68% | 5/5 |

测量前关闭了本项浏览器、服务和编译器；共享机器的其他 Rust 编译持续到约 09:12 UTC，影响早期负载和 Pocket Tank 暖机。后续样本仍随时间上升。C3 的下降不作为性能提升；Pocket Tank 的五对样本全部较慢，+1.68% 尚未归因，也不能断言性能无回退。完整数据、二进制和输入哈希见 [cpu.json](evidence/m5stack-unit-rgb-u003/runtime/cpu.json)。原始解析器只识别双核指令汇总，已保留失败时完整 S3 样本，修复解析后从其余负载续跑，没有重选 S3 样本。

复现时先按项目文档取得公开 ROM 和四个负载，再分别构建基线与当前版本。下例的 `output/u003-benchmark-base` 必须为空目录；不需要改动另一工作树。

```sh
mkdir -p output/u003-benchmark-base
git archive 9d072179 Cargo.toml Cargo.lock emu-core esp-periph esp-soc xtensa-lx7 esp32s3 cli wasm wasm-jit riscv-rv32 esp32c3 esp32c6 esp32 esp32p4 | tar -x -C output/u003-benchmark-base
cargo build --release --bins --manifest-path output/u003-benchmark-base/Cargo.toml
cargo build --release --bins
python3 docs/platform/evidence/m5stack-unit-rgb-u003/runtime/bench.py --base-bins output/u003-benchmark-base/target/release --candidate-bins target/release --rom-dir web/wasm/fw --receipt output/u003-recheck.json
```

两项 Clippy、1365 项 Rust 测试、69 项平台 Node 测试、11 组既有 WASM 回归、U003 与 U027 各九组真实 WASM 检查均通过；目录验证和公开模型重建通过。命令见 [checks.json](evidence/m5stack-unit-rgb-u003/runtime/checks.json)，真实浏览器工作台、2D、mini 与拔插截图见 [browser.json](evidence/m5stack-unit-rgb-u003/runtime/browser.json)。
