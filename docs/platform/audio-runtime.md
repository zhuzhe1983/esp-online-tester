# 浏览器音频与 PCM 流接口

产品工作台和嵌入播放器共用同一套 PCM 接口。浏览器麦克风、扬声器、文件测试和 agent 调用都经过真实 I²S 模型；浏览器设备权限由页面处理，WASM 不直接访问声卡。

## 支持范围与接线边界

先检查 `await espTester.capabilities()` 中的 `audio`，加载固件后再看 `(await espTester.observe()).audio.available`。当前逻辑输入支持 ESP32-S3、ESP32 的 I²S 0/1，以及 ESP32-C3/C6 的 I²S 0；ESP32-P4 暂不支持。实际操作仍以加载的 WASM 返回结果为准。输出流目前由 S3 实现，格式为单声道有符号 PCM16。

输入路径标为 `logical-i2s-port`：主机 PCM 进入指定 I²S 控制器的源，固件仍需配置 RX 时钟、格式和 DMA 才能读到数据。此入口绕过物理 GPIO 接线与 ES8311 模拟增益、静音、供电等行为，不是完整 codec 或模拟麦克风电路。单声道输入复制到左右两条数字声道；源采样率与固件采样率不同时，底层按其各自时钟转换，不以修改语速来追赶宿主机。

StopWatch 的 ES8311 `0x18` 控制寄存器初始化与 PCM 流是两个验证层面。官方小智初始化成功不证明语音唤醒、ASR、云端对话或整条物理音频链路已经适配。参见 [ES8311 范围](stopwatch-es8311.md)。

## 页面控制

3D 画面右上角分别显示麦克风和扬声器按钮，初始均关闭，状态分为关闭、开启中、开启及错误。

- **麦克风：** 固件运行后点击开启，浏览器才申请权限。使用 `getUserMedia` 和 AudioWorklet，以 AudioContext 实际采样率产生单声道 PCM16，提交到逻辑 I²S 0。未开启回声消除、降噪或自动增益，也不把麦克风直接接到扬声器。
- **扬声器：** 点击后解锁 AudioContext，将固件输出交给 AudioWorklet。按源采样率播放，必要的固定比例重采样保持原始时长、语速和音高；不会设置变速播放来补偿模拟器速度。
- **队列与释放：** 浏览器输入/播放队列各有两秒上限；溢出会显示错误并停止对应设备，供给中断单独计数。关闭麦克风、暂停固件、切换/重新加载固件、重置或离开页面会释放相应采集资源；关闭扬声器只停止本地播放，不关闭 runtime 的 PCM 捕获。切换会话后须重新开启按钮。

需要 HTTPS 或 localhost。跨来源 iframe 的父页面还需授予麦克风权限；操作系统/浏览器拒绝权限时会显示错误。按钮开启只说明浏览器通路已启用，固件是否消费输入仍须检查其实际输出。

## Agent API

以下三个方法同时存在于 `window.espTester` 和经过来源/会话握手的 iframe RPC 中。它们不申请浏览器麦克风权限，不依赖本机声卡；Node 或 MCP 适配器可以直接传递相同的 PCM 数据。

| 方法 | 行为 |
| --- | --- |
| `audioConfigure({input?,output?})` | 已加载固件后配置输入或捕获开关，返回音频状态。输入为 `{enabled,port?,sampleRate?,channels?}`；输出为 `{enabled}`。 |
| `pushAudio({sampleRate,channels=1,port=0,samples})` | 运行中提交 PCM16；返回 `{acceptedFrames,guestTime,input}`。需要先启用相同端口/格式的输入。 |
| `readAudio({clear=true}={})` | 刷新已启用输入的 WASM 计数，然后返回输出块、输入状态及输出统计。默认读后清空输出历史；`clear:false` 保留历史。 |

输入采样率是 8000–96000 的整数，声道数为 1 或 2，`samples` 为交错排列的 `Int16Array`，也可使用取值 −32768…32767 的整数数组。每次提交 1–16384 个完整帧；一个帧包含所有声道的样本。输入端口必须在当前芯片的能力范围内。

启用输入会替换该端口之前的主机源并清空旧队列/计数。更换活动端口时先停用旧端口；关闭时须指定当前端口。输出默认启用，`output.enabled:false` 只停止页面的事件与历史捕获，不改变固件 I²S 时钟，也不自动清空已捕获历史。

### 提交短 PCM 测试片段

```js
const api = window.espTester;
// 先选择正确设备、加载并运行需要录音的固件。
await api.audioConfigure({input:{enabled:true,port:0,sampleRate:16000,channels:1}});
const samples = Int16Array.from({length:1600}, (_, i) =>
  Math.round(4000 * Math.sin(2 * Math.PI * 440 * i / 16000)));
const receipt = await api.pushAudio({port:0,sampleRate:16000,channels:1,samples});
const audio = await api.readAudio({clear:false});
// receipt.acceptedFrames 是入队数量；用固件实际识别结果、串口或 DMA 证据验证后续处理。
```

WASM 输入队列容量为声明采样率的两秒。队列满时只接受可容纳的前缀，保留已排队数据；`pushAudio` 以 `AUDIO_INPUT_OVERFLOW` 拒绝，并在 `error.details` 中给出 `acceptedFrames`、`requestedFrames` 和输入状态。文件发送器应根据刷新后的 `queuedFrames/capacityFrames` 分块发送，遇到部分接收时仅重试未接收的后缀，不可整块重发。使用有截止时间的等待，避免固件未启用 RX 时无限等待。

### 读取和导出输出

每个输出块为 `{sampleRate,channels:1,samples:Int16Array,sequence,guestTime}`。`sequence` 在当前 worker 内递增；`guestTime` 是 worker 发布时的模拟秒数，并非逐样本时间戳。固件控制输出采样率，不同块可能不同。

```js
const result = await espTester.readAudio();
if (result.overflowed || result.droppedFrames)
  throw new Error('捕获历史不完整；应更频繁地读取或订阅 PCM。');

const {encodePcm16Wav} = await import(new URL('./audio.js', location.href));
const first = result.chunks[0];
if (!first) throw new Error('固件尚未输出 PCM。');
if (result.chunks.some(c => c.sampleRate !== first.sampleRate || c.channels !== first.channels))
  throw new Error('采样格式改变；请分段导出，不要改速拼接。');
const samples = new Int16Array(result.chunks.reduce((n,c) => n + c.samples.length,0));
let offset = 0;
for (const chunk of result.chunks) { samples.set(chunk.samples,offset); offset += chunk.samples.length; }
const wavBytes = encodePcm16Wav({...first,samples}); // Uint8Array，交给文件保存工具或下载入口。
```

`encodePcm16Wav` / `decodePcm16Wav` 位于 `web/platform/audio.js`，处理单/双声道、8–96 kHz 的无压缩 PCM16 WAV，不是 `espTester` 方法。导出 PCM 上限为 128 MiB；压缩音频和多个 `data` 块被拒绝。文件输入可使用 `decodePcm16Wav(new Uint8Array(await file.arrayBuffer()))`，随后按上述输入队列规则分块提交。runtime 能保留固件较宽范围的有效输出采样率；超出浏览器/WAV 帮助函数支持范围时应保留原数据并报告，不能重标采样率改变声音。

输出历史最多保留最近十秒，以完整块为单位淘汰。`readAudio` 返回 `enabled`、`receivedFrames`、`droppedFrames`、`overflowed`、`queuedSeconds`、`queuedChunks`、`capacitySeconds`、`input` 和 `chunks`。默认清空后，丢帧标记/计数也清零，`receivedFrames` 仍累计到 worker 更换为止。长期录音应持续排空或订阅数据，不能依赖十秒历史保存整段音频。S3 原生 CLI 的历史 WAV 捕获不受浏览器流模式影响。

### 状态和订阅

`observe().audio` 包含 `available`、`input` 和 `output`；它是最近一次操作的快照。需要最新输入进度时调用 `readAudio({clear:false})`，该方法会向 worker 查询，而不是仅返回前端缓存。

输入状态包含 `enabled`、`configured`、`port`、`sampleRate`、`channels`、`routing`，以及 `queuedFrames`、`capacityFrames`、`pushedFrames`、`consumedFrames`、`underrunFrames`、`rejectedFrames`、`clearedFrames`。未配置或已关闭时部分格式字段可能不存在。`consumedFrames` 表示数字源时钟消耗的帧数，不证明应用完成识别或处理。

```js
const unsubscribe = espTester.subscribe(event => {
  if (event.type === 'audio') {
    // 立即交给有界文件/网络写入器；live 数据独立于十秒输出历史。
    // event: {type,sampleRate,channels,samples,sequence,guestTime}
  } else if (event.type === 'audio-state') {
    // 配置、worker 重置或捕获溢出状态：event.audio
  }
});
// 结束时 unsubscribe()。跨来源 iframe 没有 subscribe RPC，可定期 readAudio()。
```

调用输入配置或提交 PCM 会结束当前便携交互录制/回放；PCM 不写入场景分享链接或按钮/触摸录制。worker 更换会清空输入、输出历史和计数；固件自身的芯片重启可保留主机输入源，不能与平台 `reset()` 重建 worker 混为一谈。暂停会发布剩余 TX 数据；浏览器麦克风停止，脚本配置的源则保留到显式关闭或 worker 更换。

## 错误与验证

常见 API 错误：`AUDIO_UNSUPPORTED`（未加载或 ABI/芯片不支持）、`INVALID_AUDIO`（格式错误）、`AUDIO_INPUT_DISABLED`、`AUDIO_FORMAT_MISMATCH`、`AUDIO_PORT_MISMATCH`、`NOT_RUNNING`、`AUDIO_INPUT_OVERFLOW`、`AUDIO_COMMAND_FAILED`（worker 拒绝）。会话更换还可能拒绝未完成的命令。浏览器层另报告权限失败、AudioWorklet 故障、输入/播放溢出，不把失败包装成正常录音。

`node --test tools/platform-audio-browser-test.mjs` 验证 PCM 时长/频率、边界跨块、立体声、WAV 往返、背压、权限取消及资源释放。真实 Chromium 合成 440 Hz MediaStream 已通过 AudioWorklet 输入；16000 个 16 kHz 输出样本渲染为 48000 个 48 kHz 样本，未改变一秒时长。桌面和 375/320 px 布局检查通过。这些是数字通路与浏览器生命周期验证，不代替物理麦克风、扬声器主观试听或某款固件完整语音功能验收。
