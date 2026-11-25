# C5 LCD2.73 原厂验收脚手架

`c5-lcd273-acceptance.mjs` 使用 `createPlatformRuntime`、生产
`web/wasm/worker.js` 与 caller 指定的生产 WASM，不另造模拟器。
`--run` 才启动客体；导入模块、`--help` 和 `--check-inputs` 均不会实例化
WASM、启动 worker 或打开浏览器。本脚本没有构建、native 或 Playwright 步骤。

本脚手架尚未执行原厂验收。历史阻塞点为 BOD0 READ `0x600b2c00` / PC
`0x42070f9c`，尚未到 `app_main`；每次运行必须重新观察，不能把历史记录
写成当前结果。本脚本不能宣布产品或 TF 完整验收通过。

本工具只接受严格硬件路线。显式 Wi-Fi 离线驱动兼容模式、对应 worker
命令、非空运行快照或 firmware receipt profile 均被拒绝；即使应用已经产生
真实画面，也不能把驱动协助后的结果归入本工具的严格验收。兼容模式必须
另记 `compatible-driver-assisted`，不能因同一原厂 BIN 或 pixels 非零而互认。

## 输入与运行

需要 Node.js 20 或更新版本。先在父级完成需要的构建与输入准备，再传入具体
文件路径或 HTTP(S) URL。脚本不内置私有主机、不下载替代固件、不改 ROM、
factory 或原 SD 文件，也不格式化或清除卡片。每次运行使用独立的现有 Node
memory storage，只把原卡复制到任务自己的运行时。

固定输入如下，摘要不能由命令行换成任意值：

| 输入 | SHA-256 |
| --- | --- |
| C5 MP rev100 ROM | `49693d745eb4b4e6e6c77be4f5213765d545d8c7a1d9742a8a619bf596c4e27a` |
| 原厂 FactoryOnly_20260724 | `cf10a4358d03898458c16390f3f16ceb0306befe2260160de0919d755b920942` |
| 64 MiB SD 原始镜像 | `a0e026d9aeafab624884455d25147647980bd6a83011618da04140659c28bfc6` |

`--wasm-sha256` 必须由调用方从要验收的生产构建单独确定。脚本记录实际 WASM
摘要、runtime/worker/关键配置文件摘要、Git HEAD 与是否有未提交改动。WASM
摘要本身不能证明它来自该源码；构建来源需保留父级构建记录。

以下变量均由调用方赋值；不要把原始卡或私有日志加入 Git：

```sh
node tools/c5-lcd273-acceptance.mjs --help

node tools/c5-lcd273-acceptance.mjs --check-inputs \
  --runtime-root "$C5_RUNTIME_ROOT" \
  --wasm "$C5_PRODUCTION_WASM" --wasm-sha256 "$C5_PRODUCTION_WASM_SHA256" \
  --rom "$C5_REV100_ROM" --factory "$C5_ORIGINAL_FACTORY" --sd "$C5_ORIGINAL_SD" \
  --out "$C5_INPUT_CHECK_OUTPUT"

node tools/c5-lcd273-acceptance.mjs --run \
  --runtime-root "$C5_RUNTIME_ROOT" \
  --wasm "$C5_PRODUCTION_WASM" --wasm-sha256 "$C5_PRODUCTION_WASM_SHA256" \
  --rom "$C5_REV100_ROM" --factory "$C5_ORIGINAL_FACTORY" --sd "$C5_ORIGINAL_SD" \
  --out "$C5_ACCEPTANCE_OUTPUT" \
  --wall-timeout-ms 180000 --boot-guest-seconds 15
```

输出目录必须尚不存在，建议位于仓库忽略的 `output/` 中。`--save-uart` 额外
保留 UART/emulator 各自最后 65,536 字符的私有日志；默认结果只保留字节数、
完整事件流摘要、命中的预期 marker 与故障地址等 token。`--save-sd` 可将各
阶段的导出镜像另存至新输出目录，始终不会覆盖原始文件。URL 的实际下载和
HTTPS/CORS 浏览器兼容性是两件事：Node 下载成功不能证明浏览器 CORS 通过。

## 观察与判断

脚本依次检查以下事实；前置阶段受阻时保留已观察到的证据，不补填后续结果：

1. 精确 ROM、factory、64 MiB SD 和 caller pin 的 WASM 摘要；产品与三个
   active-low 按键路由：PREV/G2、NEXT/G3、BOOT/G28。
2. 真实 worker 创建独立 `waveshare-c5-lcd273`、32 MiB Flash、8 MiB PSRAM、
   interpreter、`appDirect:false` 的 handle；创建回执的 `clockHz` 必须为
   240 MHz 标称 CPU/counter 频率，按 kind 0 ROM、kind 5 原始 merged factory
   顺序加载。48 MHz XTAL 是独立的 C5 runtime 约束，创建回执没有 live XTAL
   字段，脚本不声称从该回执验证了晶振。拒绝 app-direct 混跑、
   stubs、实验开关、非原始字节或意外远端 fallback。
3. 启动后 UART0 新出现 `Calling app_main()`，并收到真实 framebuffer。
   RGBA 必须为 320×320、至少有非黑像素与两个 RGB 颜色；原始像素逐阶段
   保存为 `.rgba`，记录其 hash、帧号与采样时 guest clock。alpha 非零不算
   非黑像素。没有帧时保留 `NO_FRAME`/阻塞结果，不生成替代画面。
4. PREV、NEXT、BOOT 分别按下、等待 guest 时间、释放。用现有 probe 已采用
   的 ignored-JSON `{}` queue receipt 采样真实 worker clock，避免把 host
   毫秒当作 guest 持续时间；该回执不能证明 firmware 消费输入。记录真实
   按住时长、按下/释放回执、新 UART 摘要及输入前后真实像素差。
5. 暂停并导出真实 SD 后端字节，与原卡逐 sector 比较；零变化只说明未观察
   到写回。若有变化，记录改变 sector 数与前 16 个 sector 的前后摘要，
   不保留文件名或内容。随后执行真实 ROM reset 与新 worker reload，比较
   完整导出镜像。reset 会运行客体，因此 reset 后不同也可能是新的 guest
   写入；单独报告，不擅自解释为丢数据。reload 比较发生在新客体启动前。

`result.json` 的 `hardwareDiagnostics.observations` 逐项保留 public runtime
发出的确切 C5 冷快照：每个 worker/card epoch 的首次发布（启动受阻也保留）、
load/boot、每次真实 capture、暂停、导盘、reset/reload 和最后清理观察。
帧、导盘及启动记录用 `hardwareDiagnosticsSequence` 指向相应快照。该快照
来自现有 publication，不是强制即时采样，与 framebuffer 或 guest clock
不原子；worker 仍按既有节流和去重发布。

`epoch.workerSequence` 标识实际 worker；`epoch.cardAttachSequence` 仅在
真实 `sd-attach` 收到成功回执后更新，新的 attach 或新 worker 都不能沿用
旧卡增量。对同一 epoch 且能绑定到真实发布的相邻已观察快照，helper 用
BigInt 计算下面三个 u64 十进制字符串的差，结果仍为字符串，绝不转 Number。
跨 worker/card、epoch 或发布归属未知、缺少快照、无卡、格式不符、计数倒退
均明确返回 unavailable / invalid / not-comparable，不计算差值。重用同一
publication 时差值可为 `"0"`，`newPublication:false` 明确指出没有新的发布；
这个零不能证明期间没有传输活动。新 worker reload 到 `ready` 尚未发布
diagnostics 时会保留缺失原因，不补零或复制旧快照。

按键输出含周期动画/日志时，单纯“输入后 hash 变了”不足以确定按键语义。
可传 `--expectations <json>`，填写从**同一固定原厂固件**取得的具体 marker
与排除动画的屏幕区域。以下是未填写语义 marker 的结构示例，不含假定的
厂商日志；`null` 会保留为 `partial`，不会通过按键语义检查：

```json
{
  "schemaVersion": 1,
  "factorySha256": "cf10a4358d03898458c16390f3f16ceb0306befe2260160de0919d755b920942",
  "appMainIncludes": "Calling app_main()",
  "controls": {
    "prev": {"serialIncludes": null, "region": [0, 0, 320, 320]},
    "next": {"serialIncludes": null, "region": [0, 0, 320, 320]},
    "boot": {"serialIncludes": null, "region": [0, 0, 320, 320]}
  }
}
```

按键 `firmwareResponseVerified:true` 要求：输入后新 UART 命中专属 marker、
同一区域输入前 0.3 guest 秒内保持静止、输入后像素发生变化、真实按住时长
不超过 0.5 guest 秒。未配置 marker、动画持续变化、仅排队、无输出或过长
按住均不足以通过这一检查。这个规则可能低估仅输出日志的真实按键行为；
这是已明确的保守限制，不应为得到通过而改固件或填造 marker。

## TF 观测能力与缺口

当前 C5 LCD2.73 runtime 除 `sdDelta`、`sd-export`、`sd-flush` 外，还可通过
既有 `hardwareDiagnostics` 冷快照观察卡状态和三个真实累计计数，所有 u64
均用 `counterEncoding:'u64-decimal'` 的规范十进制字符串保留完整精度：

| 字段 | 能证明的事实与限制 |
| --- | --- |
| `nativeCommandCalls` | 内层 `SdCard::command` 被调用，包括失败调用；SPI wrapper 特判的 CMD0/58/59/9/10 等未计入，不能当全部线上 CMD 或成功命令数。 |
| `storageBlocksLoaded` | backing store 的完整块加载成功；发生在 reply 完整送出和有效 MISO 路由确认之前，不能当 guest-completed reads。 |
| `storageBlocksWritten` | backing store 完整 512B 写入成功；不证明 guest 已消费响应、FAT mount 或文件 flush。 |
| LCD 状态 | 已锁存的 reset/sleep/on/backlight/format/MADCTL、像素/RAMWR/reset 与板级 rejected 计数；不是实机光学验证，也不产生额外画面。 |

`sd.present`、`spiMode`、`cardState` 仅描述卡对象和数字卡状态，没有
guest FAT mount 状态。`sdCard.mounted` 由 host attach 设置，prepared card、
累计计数非零和 UART “mount/write success”都不能各自替代完整传输、guest
文件读取或挂载证据。旧 WASM/runtime 没有新 snapshot 时，结果明确记录
`status:'unavailable'` 和原因，保留既有真实帧/串口/导盘范围，不造数据。

原卡与客体执行后导出盘不同，可以证明本轮 guest 后端发生字节写回；脚本
没有在启动后向卡写入 host 数据。reset/reload 后导出盘相同可以证明这个
Node memory backend 保留了这些字节。这些事实仍不能证明 guest 读过某个
sector、FAT 挂载成功或浏览器关闭后持久化。若客体写后又恢复原字节，导盘
可能与输入相同，但 `storageBlocksWritten` 仍可记录实际 store write；两类
证据分别保留，不能将最终盘相同误写成没有写操作。当前完整 TF 项仍保留为
`partial`/`unavailable`，`acceptancePassed` 始终为 `false`。

要补齐独立验收，仍需按实际传输独立核对有效 MISO、完整数据响应与 guest
消费；现有 getter 没有 LBA/read-write 数据摘要或 guest-read-complete 字段，
helper 不会从上述计数推导这些事实。guest 挂载/读取还需与固定原厂真实输出、已知卡内数据
或可核对的 guest readback 对应。跨进程持久化应另用现有 browser IndexedDB
路径验证。此脚手架不修改桥接实现，也不编造这些观测字段。

## 结果和清理

`result.json` 的 `status` 为：`inputs-checked`（仅静态输入检查，退出 0）、
`error`（输入/hash/ABI/脚手架错误，退出 1）、`blocked`（未到达可验证的启动
输出，退出 2）或 `partial`（已有真实运行观察，仍缺必要验收证据，退出 3）。
本脚本没有产品验收成功的退出码；现有能力无法覆盖的项不会被自动降级。

初始提交 `3958c8b0` 将 worker `created.clockHz` 错当成 48 MHz XTAL，导致
启动客体前误报 `WORKER_ABI`。后继修复保留该提交历史，改为核对实际 API 的
240 MHz nominal counter，并将期望 48 MHz XTAL 单独记录为
`liveObserved:false`。频率依据是 `esp32c5/src/bus.rs` 的 `CPU_HZ`、
`soc.rs` 的 `Soc::CPU_HZ` 以及 `wasm/src/machine.rs` / `lib.rs` 的
`cpu_hz` / `esp32sim_cpu_hz`；它们与 C5Bus 的 `xtal_hz` / `set_xtal_mhz`
是不同契约。脚本没有为满足断言修改运行时或改变 guest 时间换算。

所有 task-owned worker 在 `finally` 中处置并等待退出，结果记录
`cleanup.workersExited` 和仍存活数。不打开浏览器，不接触用户 browser
session，不向远端发消息或发布产物。此新增脚本的静态检查为：

```sh
node --check tools/c5-lcd273-acceptance.mjs
node tools/c5-lcd273-acceptance.mjs --help
node --input-type=module -e "await import('./tools/c5-lcd273-acceptance.mjs')"
```

下面的纯 helper fixture 只校验 u64 差值和 epoch 边界，不读取输入镜像、
不创建 runtime/worker，也不代表真实固件结果：

```sh
node --input-type=module <<'NODE'
import assert from 'node:assert/strict';
import {c5CounterDelta} from './tools/c5-lcd273-acceptance.mjs';
import {c5Lcd273Snapshot} from './tools/fixtures/c5-lcd273-diagnostics.mjs';
const row=(sequence,workerSequence=1,cardAttachSequence=1)=>({status:'observed',sequence,
  epoch:{workerSequence,cardAttachSequence},publicationSequence:sequence,snapshot:c5Lcd273Snapshot()});
const a=row(1),b=row(2);
a.snapshot.state.sd.card.nativeCommandCalls='9007199254740993';
b.snapshot.state.sd.card.nativeCommandCalls='9007199254740994';
assert.equal(c5CounterDelta(a,b).nativeCommandCalls,'1');
assert.equal(c5CounterDelta(a,row(2,2)).reason,'worker-changed');
assert.equal(c5CounterDelta(a,row(2,1,2)).reason,'card-changed');
assert.equal(c5CounterDelta(a,{...b,publicationSequence:null}).status,'unavailable');
assert.equal(c5CounterDelta(a,{status:'unavailable'}).status,'unavailable');
assert.equal(c5CounterDelta(a,row(3)).reason,'counter-decreased-within-epoch');
const reused=c5CounterDelta(a,structuredClone(a));
assert.equal(reused.nativeCommandCalls,'0');assert.equal(reused.newPublication,false);
const invalid=structuredClone(b);invalid.snapshot.state.sd.card.nativeCommandCalls='18446744073709551616';
assert.equal(c5CounterDelta(a,invalid).status,'invalid');
console.log('PASS: pure counter/epoch assertions; no runtime started');
NODE
```
