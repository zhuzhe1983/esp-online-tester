# 模拟器内写入 UIFlow2 配置

`inspectFirmwareConfiguration()` 检查当前已加载设备的实际 Flash 分区表，返回可写状态、分区范围和本次加载的 `sessionId`。它不读取或返回已存 Wi-Fi 密码。只有完整 ROM 启动镜像、容量正确、分区未加密且未重叠，并存在唯一 `nvs` 分区（`0x9000`、`0x6000` 字节）时才允许写入。应用直启、未知布局、越界或损坏分区表都会拒绝。

`writeFirmwareConfiguration(profile, {sessionId})` 需要调用者明确提供 UIFlow2 配置，格式沿用[烧录配置](flash-configuration.md)的 `{name,type:'uiflow2',server,ssid,password}`。可选启动设置沿用同一个编码器。默认加载固件不会写配置。界面应先说明：操作会重建整个 24 KiB NVS 分区，替换其中全部原有命名空间和键；其他 Flash 分区及 TF 卡保留。再让用户选择配置并执行。

```js
const target = await espTester.inspectFirmwareConfiguration();
if (!target.available) throw new Error(target.reason.message);
// profile 是用户明确选择的私有配置；不放入场景、URL 或日志。
await espTester.writeFirmwareConfiguration(profile, {sessionId: target.sessionId});
await espTester.start();
```

写入先暂停客体并保存其 Flash、TF 卡改动，再重新检查当前分区表，用现有 UIFlow2 NVS v2 编码器生成 `uiflow` 命名空间。一次带版本检查的存储事务替换 NVS 的六个扇区，其他已修改页面原样保留。之后重建模拟器并返回完整状态，状态为 `ready`；启动由调用者单独执行。不会连接 Web Serial，也不会写实体设备。

源文件、下载缓存、固件来源及 artifact SHA-256 不变。存储键仍由产品、板型、芯片、容量和原始完整 Flash 的 SHA-256 决定，因此同源固件重新加载、模拟器重启、页面重新打开均按原有设备存储机制恢复配置。IndexedDB 不可用时，状态会明确显示 `memory-only`，只能保证当前页面会话内恢复。

`sessionId` 在每次加载或重建后失效，防止检查后更换设备仍写入；省略它时，底层 API 也会捕获调用时的设备实例，阻止排队任务写入后来的设备。另一页面写入或清除导致版本冲突时返回 `STORAGE_CONFLICT`，不会部分替换 NVS。

返回状态中的 `firmwareConfiguration` 只含 `{type,partition,namespace,writtenAt}`，不含名称、服务器、SSID 或密码。`inspectFirmwareConfiguration().lastWrite` 返回同一脱敏回执；回执保留在当前运行时内，重新打开页面后为 `null`，不影响已存 Flash。配置内容不进入场景分享、录制、固件来源或配置操作事件。原始固件若自行通过串口打印信息，仍属于客体输出。

写入已经提交、重新初始化尚未完成时，`cancelPendingLoad()` 可取消重启，错误为 `FETCH_ABORTED` 且 `details.configurationCommitted === true`。这时配置已经保存，下一次加载同源镜像会恢复；取消不撤销已提交的事务。会话替换或页面关闭与提交竞争时，同样不会将数据写入新的设备 scope。明确清除该设备存储才恢复原始 NVS。

## 验证

```sh
node --test tools/platform-virtual-firmware-configuration-test.mjs tools/platform-flash-configuration-test.mjs tools/platform-device-storage-test.mjs tools/platform-sd-card-test.mjs
node tools/probe-virtual-uiflow-configuration.mjs <original-corematrix.bin> <sha256> --boot-option 2 --receipt /tmp/virtual-configuration.json
```

专属测试覆盖实际分区结构、当前表被修改、加密/重叠/越界/容量拒绝、原镜像不变、VFS/TF 保留、重复写入、重启和重新加载、排队换板、旧 sessionId、版本冲突、提交后取消及凭据隔离。生产 WASM 探针需要已构建的 `web/wasm/esp32sim.wasm` 和 C61 rev100 ROM，运行原始 CoreMatrix UIFlow2 镜像；通过 USB raw REPL 的 `esp32.NVS.get_str/get_u8` 断言实际值，客体文件读写验证 VFS 和 TF 保留，并检查原 `boot.py` 哈希。探针只保留断言名称与哈希，不保存串口内容或凭据。NVS 读回通过不代表 M5Things、UIFlow 云服务或实体网络已经可用。

CoreMatrix 原版 UIFlow2 2.5.4（SHA-256 `8d5b8d4e22ebc293937086b2f9f218aa5a5ebda27b5ccb32b3f8f580ee02939b`）与部署用生产 WASM（`2a399f6347d719b2324336ff2e7807f3f43d735f72498b955506e8ab637e53c2`）的[实测回执](evidence/virtual-firmware-configuration/corematrix-production.json)记录了上述验证。`bootOption:2` 时，原始启动程序自动连接与配置匹配的虚拟 WPA2 路由器，客体读到 DHCP 地址 `10.0.2.15`；测试代码只等待和检查网络状态，没有调用 `WLAN.connect()`，未修改 `boot.py`。同一行为在 host reset 和同源镜像重新加载后再次通过。

`tools/fixtures/virtual-configuration-browser-check.js` 在实际网页点击内置配置的“写入并重启”，验证真实客体自动关联、DHCP、重启与整页刷新后的 IndexedDB 恢复。每一步都通过原版固件 `esp32.NVS` 读回，不通过直接调用 `WLAN.connect()` 帮助联网。浏览器检查另覆盖默认不写入、源身份不变、无凭据回执、未加载的 iframe 禁用写入和 390px 布局；见 [浏览器收据](evidence/virtual-firmware-configuration/browser.json)。
