# LCD2.73 冷态结构化观测

本变更给 `waveshare-c5-lcd273` 的现有 `hardware_diagnostics` 链路补入只读
LCD/SD 状态，未改变 Bus、Soc、BoardModel trait、WASM ABI、SD byte/tick
处理或 UI。原厂启动、按键语义、TF 挂载/读写和持久化仍须独立验收；这里
没有运行固件，也没有原厂通过结论。

## 数据来源与精度

`SpiSdCard::observation(&self)` 复制内层真实状态，不发命令、不取回复字节、
不读卡、不推进时间、不重置计数。`Lcd273::hardware_diagnostics(&self)` 只
读取已锁存的 LCD reset/on/sleep/backlight/format/MADCTL、已有像素与 RAMWR
命令计数、板级 rejected count，以及卡的这份观测；它不调用 sync/advance。

所有 u64（包括容量、像素、RAMWR、reset 和 SD counters）以规范十进制
**字符串**发出，`counterEncoding:'u64-decimal'`。JS parser 接受范围为
`0..18446744073709551615`，拒绝 Number、前导零、指数、符号、小数和溢出，
不将字符串转成会舍入的 Number。状态对象按字段复制，额外字段不进入
`observe().hardwareDiagnostics`。

| 字段 | 真实含义与边界 |
| --- | --- |
| `nativeCommandCalls` | 内层 `SdCard::command` 调用数，包括随后失败的调用；SPI wrapper 特判的 CMD0/58/59/9/10 等不在其中，不能当全部 wire CMD 或成功 CMD 数。 |
| `storageBlocksLoaded` | `store.read` 成功加载块数；发生在 reply 被完整时钟取出之前，且板级 MISO 路由验证更晚。不能当 guest-completed reads。 |
| `storageBlocksWritten` | 完整 512B `store.write` 成功提交次数；不证明固件已读 ACK、FAT mounted 或 file flush。 |
| `sd.present`、`cardState`、`spiMode` | 卡对象存在、SD card 状态和 SPI 模式；不是文件系统挂载状态。无卡时 `card:null`。 |
| `ramWriteCommands`、`pixelsWritten` | 既有 ILI9488 RAMWR 命令与像素写入数，不是完整帧呈现或物理光学效果计数。 |
| `boardRejectedTransactions` | 既有板级拒绝计数，包含 LCD/SD 路由或模式拒绝，不能当 SD 专属错误数。 |

SD counters 属于卡对象，CMD0 / power_cycle 不清累计值；重新附加卡或新 worker
产生新对象。调用方必须按 worker/card epoch 比较增量，不能跨重建直接相减。
没有新增 mounted、guestCompletedReads、lastLba、传输内容 hash 或其它并未
记录的事实；这些名字即使出现在输入消息中也会被白名单 parser 丢弃。

## 接入范围

沿用现有 `esp32sim_hardware_diagnostics`，无需新 export。worker 只给确切
`waveshare-c5-lcd273` 增加 allowlist，复用原有最多每 250 ms 的冷态采样、
相同快照去重和 pause/stop 强制采样，没有新增定时器。runtime 的
capabilities 对这块板标记 `source:'guest-registers-and-device-transactions'`，
其它板的 source 和可用性契约不变；其它 C5 板没有继承此能力。没有添加
新的诊断 UI。

既有验收脚本 `tools/c5-lcd273-acceptance.mjs` 本轮未修改，尚未消费新 counters。
其完整验收仍受 read completion / FAT mount / browser persistence 等观测缺口
限制。即使未来消费本快照，仍需原厂真实输出、有效路由和实际文件/帧证据。

## 检查记录

已运行并通过：

- `node --test tools/platform-c5-lcd273-diagnostics-parser-test.mjs`：6 个纯 parser
  测试，覆盖完整 u64 精度、坏值、卡存在/空卡、伪造 mount/read-complete
  字段丢弃、边界和复制，以及原有 StopWatch/CoreMatrix schema。
- `node --check`：worker、runtime、hardware-diagnostics parser、现有诊断
  测试文件及新增 fixture。
- `git diff --check`。

已新增但本轮未运行，交由统一检查：

- `esp-periph` 的 `cold_observation_*` Rust tests：观察不消耗 OCR/写回复，
  partial write 不增提交数，store load 可早于 payload delivery，reset 保留
  counters，完整 u64 不缩窄。
- `esp32c5/tests/lcd273.rs` 的冷快照测试：重复观测不改变 cycles、frame
  generation、RAMWR、report 或 display；亮度锁存与可见性一致，未附卡和
  非目标板保持真实缺省状态。
- `tools/platform-hardware-diagnostics-test.mjs` 的 runtime/worker doubles：
  确切 board 门槛、复制、畸形消息、旧 worker 隔离、pause/stop 和其它 C5
  产品隔离。这些 doubles 也不代表固件执行证明。

没有运行 Cargo、native、WASM、完整 platform/browser 或 benchmark，没有
修改 SD 媒体。后续统一验证应运行上述 Rust 与 runtime/worker tests，并
检查生产 WASM/真实原厂输出；本记录的静态及 parser 结果不能代替它们。
