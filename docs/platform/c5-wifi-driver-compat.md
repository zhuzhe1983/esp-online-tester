# LCD273 Wi-Fi 驱动离线兼容模式

Waveshare ESP32-C5 LCD 2.73 的场景可显式选择 Wi-Fi 驱动兼容模式，以便运行受支持的原厂 IDF 5.5.3 界面。它绕过尚未完整模拟的驱动接口，不代表 Wi-Fi 硬件或原厂整机已通过严格验收。严格硬件模拟仍是默认模式；遇到失败不会自动切换。

只有板型 `waveshare-c5-lcd273` 接受精确 profile：

```js
const scene = structuredClone((await espTester.observe()).scene);
scene.c5WifiDriverCompat = 'waveshare-lcd273-idf553-offline-v1';
await espTester.configure(scene); // 停止并移除当前执行实例。
await espTester.loadFirmware();   // 实际镜像、ROM 和启动方式仍需通过加载校验。
await espTester.start();
```

删除 `scene.c5WifiDriverCompat` 并重新配置，恢复严格模式。`null`、`'strict'`、未知 profile、其他板型和显式 `bootMode:'app-direct'` 都会拒绝。兼容模式要求 ROM 启动，不修改 ROM、BIN，也不代替固件运行、画屏或输入处理。

本地 Files 选择可用 `scene.firmware:null` 保存模式选择，随后把本地 merged BIN 交给 `loadFirmware`。场景校验只能检查可见配置；通过场景校验不证明实际 BIN 支持此 profile，最终加载门槛仍须核对实际产物。

工作台仅在该板型的固件区显示“严格硬件模拟 / Wi-Fi 驱动兼容（离线）”。选择后须点击“应用模式并停止运行”；界面说明需要重新加载固件。它不会把该选项写成全局默认。换设备采用新场景，会去掉旧选项；返回 LCD273 仍默认严格模式。

工作台和嵌入视图都显示醒目的离线标记：“离线模式，无热点/网络连接；Wi-Fi配置仅本次运行保留。”场景选择只显示“已选择，尚未启用”。只有当前已加载实例的 `driverCompatibility` 快照明确返回同 profile 且 `enabled:true`，才显示“本次运行已启用”。客体报错而该模型仍有启用快照时，标记为“本次兼容运行已停止”，不会改称未启用。这也不证明原厂画面、按键或 TF 工作正常，应分别检查真实 framebuffer、固件输入响应和存储证据。

场景导出、分享 URL、窗口 API 和 iframe RPC 都保留显式 `c5WifiDriverCompat`。来源 URL/SHA 匹配或切换固件来源不会删除它。分享接收方能看到离线标记；本地固件字节仍不能放进链接。Wi-Fi 配置属于本次执行实例的 RAM，不能据此声称热点、联网或浏览器跨重载持久化。

录制保存场景与固件 receipt 中的同一 profile，并把模式纳入配置和产物指纹。旧的严格模式录制继续可用；严格与兼容录制不能跨模式回放，场景与 receipt 不一致也会拒绝。

纯配置、UI 与 RPC 合同检查：

```sh
node --test tools/platform-c5-wifi-driver-compat-test.mjs
node tools/platform-config-test.mjs
node --test tools/platform-shared-startup-test.mjs
node tools/platform-recording-test.mjs
```

这些检查使用纯逻辑或传输替身，不运行 WASM、固件或浏览器，也不构成原厂兼容验收。真实浏览器、生产 worker 和原厂固件证据需在集成后另行检查。

兼容模式的 `stats.insns` 沿用 interpreter slot 计数，包含 WFI idle 和显式 HLE setup/return 槽位；`capabilities().execution.counterSemantics` 会注明这一点。`driverCompatibility.virtualSlots` 单列这些合成槽位，不能把它们称为原生退休指令或物理硬件耗时。


当前固定原厂镜像已在原生和生产浏览器中验证菜单、三个 GPIO 按键及 Photo 从 TF 卡读取、解码和显示 JPEG，正常重载保留整卡。见[实际浏览器结果](../evidence/c5-factory-browser-2026-11-25/README.md)。Sensor 页面能打开，但温湿度和 IMU 读数仍列为已知限制；原厂驱动的等待时间与字节序问题见[事务和来源证据](../evidence/c5-factory-sensor-2026-11-25/README.md)。本 profile 不提供热点关联、IP 或完整 Wi-Fi 驱动持久化语义。
