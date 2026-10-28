# StopWatch 官方 UIFlow2 与小智启动验证

2026-10-27，用未修改的官方 M5Burner 镜像验证了以下启动路径。源码基线为
`a9375446`，没有新增启动替代、固件补丁或硬件 ACK 桩。

| 固件 | 实际结果 | 尚未验证或缺失 |
| --- | --- | --- |
| UIFlow2 v2.5.3 | 原生解释器与浏览器 WASM 从 ROM 启动到 OFFLINE / DEVELOP；Button B 切换到 WiFi/server 设置页，CST820 触摸改变 server 选项高亮 | 云端连接、程序部署及完整 MicroPython 外设 API；日志有一次 `i2c.master` 尚未初始化端口的错误，未阻止所测操作 |
| 小智 v2.2.6 | 原生解释器与浏览器 WASM 从 ROM 启动到“配网模式”；客体启动 SoftAP、DNS、DHCP 和 Web server，浏览器观察到 70.46 客体秒仍在运行 | ES8311 编解码器初始化失败；麦克风、声音、唤醒和云端对话不作为可用能力；浏览器没有访问客体 SoftAP 的 HTTP 桥，不能用电脑直接访问显示的地址完成配网 |

本轮浏览器执行模式都是 `wasm-interpreter`，`appDirect=false`，`stubs=[]`。
官方 User Demo 仍排第一并作为默认项；这两项继续保留 trial 和运行前确认。
启动及少量界面操作通过不代表所有功能适配，也没有实机烧录或硬件比较。

目录固定的 SHA-256 来自本次实际下载，不是发布者签名。版本更新仍按官方
watchlist 规则重新变成未验证候选，不能继承本轮结论。镜像只在测试进程内存或
测试浏览器缓存中使用，没有加入网站、仓库或服务器固件目录。

[收据、分区信息、来源及画面](evidence/stopwatch-official-alternatives/verification.json)
保留原始失败/警告。截图中的 MAC/SSID 是模拟器确定的虚拟身份，不是真实设备身份。
UIFlow 当前公开源码的 A/B 页面导航与触摸路径也已核对，但公开源码提交不冒充
下载镜像内 `21928ff1` 的完整可复现构建来源。

## 复现

2026-10-28 更新：StopWatch 补齐 ES8311 的 I²C 控制寄存器子集后，同一小智
v2.2.6 镜像在浏览器中输出 `Es8311AudioCodec initialized` 和
`Open codec device OK`。上表保留修复前的历史结果；当前边界和回归证据见
[ES8311 修复记录](stopwatch-es8311.md)。播放、麦克风、唤醒词和云端对话仍未验证。

先按平台 README 构建 WASM、获取匹配 ROM，并启动静态服务器。在独立测试浏览器
打开平台，运行同一公开 API；不要清除用户原有 Flash 或 TF 卡数据。

```js
const {loadCatalog, createDefaultScene} = await import('./catalog.js');
const catalog = await loadCatalog();
const scene = createDefaultScene(catalog, 'm5stack-stopwatch');
// 第二轮替换为 m5stack-stopwatch-xiaozhi。
scene.firmware = catalog.firmwares.find(f => f.id === 'm5stack-stopwatch-uiflow2').source;
await espTester.configure(scene);
await espTester.loadFirmware();
await espTester.start();
// 等待真实画面后检查 capture()，不能只依赖 start() 回执。
await espTester.press('button-b', {durationMs:250});
// UIFlow 的设置页下拉区域；随后检查选项高亮的真实帧变化。
await espTester.input({type:'touch', x:358, y:125, down:true});
await new Promise(resolve => setTimeout(resolve,300));
await espTester.input({type:'touch', x:358, y:125, down:false});
await espTester.pause();
await espTester.capture({format:'png'});
```

小智只验收启动及持续配网页面，不对上面的 UIFlow 输入步骤作同样承诺。输入时长
是宿主机时长；本轮触摸实际客体按住约 0.3013 秒，不代表周期精确输入。

原生复现可将收据中的远程 URL 临时下载到用户指定路径，再运行：

```sh
target/release/esp32sim --chip s3 --board stopwatch --flash-mb 16 --psram-mb 8 \
  --boot rom --rom web/wasm/fw/esp32s3_rev0_rom.elf --flash-image "$FIRMWARE_BIN" \
  --console both --max-seconds 8 --no-reboot --no-jit --tft-png "$FRAME_PNG"
```

本轮仅更新固件目录与证据，未改 Rust、worker 或 runtime，不重新运行 CPU 性能门禁。
