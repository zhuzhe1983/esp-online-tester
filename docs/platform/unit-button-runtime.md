# Unit Button U027 的真实 GPIO 输入

本轮支持 **U027 V1.0 → StopWatch Port A**。3D 按压弹片、配件面板、agent API 和录制回放使用同一输入协议，最终改变模拟 ESP32-S3 的 GPIO11 输入。接线、供电与客体 GPIO 读取均有独立证据；这不是向串口注入一段“按键成功”文字。

## 接线依据与电气范围

[U027 官方原理图](https://m5stack-doc.oss-cn-shenzhen.aliyuncs.com/804/U027_UNIT_BUTTON_SCHE.pdf) 标注 V1.0、2018-07-18：J1.1 为白线信号，J1.2 黄线未连接，R1=10 kΩ 接 VCC，R2=20 kΩ 接地，C1=100 nF，S1 按下接地。[StopWatch 原理图](https://m5stack-doc.oss-cn-shenzhen.aliyuncs.com/1242/C152-SCH_Stopwatch_PRJ_Main_VA_20251201_2026_04_24_17_46_22.pdf) 第 2 页将 Port A 白线接 G11、黄线接 G10，并由 PM_EXT_5V_EN 控制 Grove 5V 输出。

[M5PM1 v1.9 手册](https://m5stack-doc.oss-cn-shenzhen.aliyuncs.com/1207/M5PM1_Datasheet_EN.pdf) 的 PWR_CFG `0x06` bit 3 控制 5V，HOLD_CFG `0x07` bit 6 决定关机后是否保持，SYS_CMD `0x0c=0xa1` 请求关机。抓取时间、响应类型、最终地址与 SHA-256 见 [sources.json](evidence/m5stack-unit-button-u027/runtime/sources.json)，资料旧版日期与本次抓取时间分别记录。

| 条件 | 模型输入 |
| --- | --- |
| 已连接、有 Grove 5V、释放 | G11 高；10k/20k 分压的稳态约 3.33 V |
| 按下 | G11 低 |
| 关闭 Grove 5V | G11 低 |
| 拔出 | 释放外部驱动；由客体自身 pull-up / pull-down 决定 |
| 黄色线 | 不驱动 G10；该线在 U027 为 NC |

这是数字稳态近似，不计算 C1 的 RC 延迟、触点抖动、元件容差、模拟电流或 GPIO 输出争用。没有外接供电源，也没有实机波形或硬件校准。未知用户硬件版本保留为未知；其他 Button 版次和主机不能由本项推定已支持。

## 场景与操作

```js
scene.accessories = [{
  id: 'button-unit', catalogId: 'm5stack-unit-button-u027', parameters: {},
  wiring: {mode: 'physical', interfaceId: 'grove'},
}];
await espTester.configure(scene);
await espTester.loadFirmware();
await espTester.start();
await espTester.input({type:'accessory-button', accessoryId:'button-unit', id:'button', pressed:true});
await espTester.input({type:'accessory-button', accessoryId:'button-unit', id:'button', pressed:false});
await espTester.input({type:'accessory-connection', accessoryId:'button-unit', connected:false});
```

`capabilities().input.accessories` 给出实际路由；`observe().accessoryInputs` 给出连接和按压状态。接线不接受用户指定任意 GPIO，同一引脚不能由两个物理附件占用。GPS 的既有逻辑 UART 注入没有因此获得供电或物理路由支持。

客体 `esp_restart()` 保持外部开关的连接和按压；平台“重启”重新建立场景连接并释放按钮。暂停不自动改变 API 输入状态；暂停期间可释放或拔出，客体恢复后读取结果。UI 暂停会先释放用户按键。重新配置销毁旧 worker 和旧路由。拔插是可录制的运行输入，不会改写保存的 scene；回放从重启开始重现这些事件。

## 模型与验证

原厂 STL 的按压弹片只重新分组，没有添命中盒。按钮区域 256 个三角面，整个模型仍为 3908 个三角面；位置与法线的多重集逐位相同。按钮保留与外壳相同的塑料材质，悬停显示浅绿色。公开 scratch 重建结果与发布资产一致，见[重建证明](evidence/m5stack-unit-button-u027/runtime/model-rebuild.json)。旧尺寸、轮廓叠图的测量时间与 hash 原样保留，本轮复用几何证据，不声称重新拍照或完成隐藏 PCB 的 1:1 还原。

[内部 ESP-IDF 5.5.1 源码 fixture](../../tools/fixtures/unit-button-firmware/README.md) 经过真实 ROM/bootloader/WASM Worker，验证按下/释放、供电、黄线 NC、拔出后的客体上下拉、暂停释放、客体重启保持、平台重启释放及 HOLD_5V 关机状态。四事件录制回放再次产生客体 `1→0→1→0→1`。完整结果和镜像 hash 见 [fixture.json](evidence/m5stack-unit-button-u027/runtime/fixture.json)。BIN 只位于 ignored output，未放入线上固件库。

浏览器检查覆盖 3D cap、配件面板、2D 模式和 mini 页面；均以新 UART0 GPIO 读数验收，而不是以 API 接收回执为依据。检查结果见 [browser.json](evidence/m5stack-unit-button-u027/runtime/browser.json)，截图保留[绿色按压提示](evidence/m5stack-unit-button-u027/runtime/hover.png)、[工作台](evidence/m5stack-unit-button-u027/runtime/studio.png)和 [mini 页面](evidence/m5stack-unit-button-u027/runtime/mini.png)。

## 回归与性能门禁

Rust workspace、两项 Clippy、平台 Node、catalog 与真实 WASM 回归结果见 [checks.json](evidence/m5stack-unit-button-u027/runtime/checks.json)。未更改任何 golden。

这次复用 StopWatch 现有 PowerWiring 同步和 board GPIO 边沿路径，未增加通用总线轮询。仍对照基线 `7d03d2409912516030412d715454da8dd8e4bc93`，分别运行 S3/C3/C6 hello 和 pocket-tank，五轮 base/candidate 交替，记录 `/usr/bin/time -p` 的 user CPU。命令参数、镜像 hash、逐轮样本、指令数与客体输出 hash 见 [cpu.json](evidence/m5stack-unit-button-u027/runtime/cpu.json)。构建双方均使用 `cargo build --release --bins`；基线源码可用 `git archive 7d03d240` 在独立目录重建。ROM 通过 `tools/fetch-rom-elfs.sh` 获取，pocket-tank 模型通过 `CI=1 tools/fetch-pocket-tank.sh` 获取。

正式构建的 CPU 中位数：S3 hello 0.24→0.24 s、C3 hello 2.31→2.31 s、C6 hello 1.54→1.56 s（+1.30%）、pocket-tank 39.42→39.12 s。C6 追加一次七轮反向顺序复测，1.61→1.65 s（+2.48%，7/7 候选样本较慢），见[复测样本](evidence/m5stack-unit-button-u027/runtime/cpu-c6-repeat.json)。指令和输出门禁通过，但 C6 CPU 偏差保留为未归因回归标记，不声明无额外 CPU 成本。早期构建条件不一致的样本单独保留于 cpu-initial.json，不作为最终门禁。

该测量用于排查引入无关设备开销，不是性能优化结论。短 hello 的时间粒度和共享主机负载限制了精度。机制审查参考既有 [EX201](../experiments.md#ex201)、[EX203](../experiments.md#ex203)、[EX205](../experiments.md#ex205)、[EX206](../experiments.md#ex206)，没有重复实施其中的时序或优化实验。
