# 浏览器 Wi-Fi、SoftAP 配网与网络服务

更新：2026-10-28。本文取代此前的实施计划；浏览器内 SoftAP 客户端、公共中继协议、可选本机助手、工作台 UI 和 agent API 已接通。默认配网页无需安装程序；外网默认使用公共中继，网站维护者需先按 [部署说明](public-network-relay.md) 部署服务，并填写 `web/platform/network-config.json` 的 `publicRelayUrl`。当前仓库默认值为空，表示尚未提供公网出口。本机助手见 [启动说明](network-helper.md)。

## 默认自动配网

工作台默认开启“自动配网”，虚拟路由器默认为 `esp32sim` / `esp32sim-pass`，固件 manifest 中的显式 Wi-Fi 配置优先。助手连接地址只属于当前会话，不进入分享链接或便携录制。公共中继地址为空时显示“公共中继尚未部署 · 配网页可用”；浏览器内配网和外网连接分别显示状态。

固件已有凭据时正常关联虚拟路由器，经选定中继/助手的原生 NAT 访问网络；TLS 由固件处理。固件进入 SoftAP 时，S3 虚拟客户端完成真实 802.11 关联，默认由浏览器内客户端向固件发送 DHCP 请求；本机助手模式使用助手客户端。配网页通过真实 TCP 连接读取，不伪造 HTTP 响应、不直接写 NVS。

自动配网先等待外网传输连接，避免没有出口就退出 AP。当前自动适配 `esp-wifi-connect` 的网页协议：检查原页面包含 `/scan`、`/submit`、SSID 和密码字段，提交测试路由器；若原页面指定 `/done.html`，读取成功页并按其中的 `/exit` POST 请求退出配网。只有原服务接受配置才记为已提交，取得 STA 地址才记为联网。未知协议保留原页面及 HTTP API，报告需要手动或专用客户端；尚未实现通用 ESP-IDF protocomm / PoP 自动适配。

关闭“自动配网”保留虚拟路由器、网络助手和固件已保存的配置；已发出的请求不能撤回，尚未发出的自动步骤会取消。“断开外网”只关闭公共中继/助手连接，默认浏览器模式的 SoftAP 仍可用。关闭自动配网不会清空 NVS，也不强迫已联网固件进入 AP。

## 手动配网和服务端口

底部网络区显示虚拟路由器、STA 地址、SoftAP SSID/地址和已知服务。手动模式下，虚拟客户端取得 AP DHCP 地址后自动打开 HTTP 80 候选服务；用户可以关闭网页或取消映射。默认在不允许 same-origin 的隔离 iframe 中显示固件 HTML；相对静态资源、fetch/XHR、常规表单及受支持的跳转由浏览器转入虚拟 TCP。192.168.4.1 是固件虚拟地址，不能在电脑地址栏直接打开。独立的旧式 `referrer no-referrer` 策略保留并补充标准 no-referrer；本地存储替换为隔离的有界会话内存，不写入工作台存储。遇到其他 CSP、X-Frame-Options、HTTPS、WebSocket 或不支持的动态导航会明确报错，可切到本机助手获得独立代理 origin 和打开入口；不删除安全策略强行嵌入。

可以手工添加 STA/AP 上的 HTTP、HTTPS、TCP、UDP 服务。默认模式可在浏览器内打开 SoftAP HTTP（包括非 80 端口）；STA 服务及其他协议需要在“高级连接”选择本机助手。助手的 HTTP 通过独立代理页面打开，HTTPS 使用 TCP 映射并保留浏览器证书检查；非 HTTP 服务显示本机 `127.0.0.1:<port>`，本机 MQTT、SSH 或其他客户端按原协议访问。取消、助手断线、固件切换和会话销毁会释放相应映射。

服务来源和验证状态分别显示；端口 80 只是候选，不证明服务存在。尚未实现 mDNS/DNS-SD 发现或全端口扫描，也不能自动知道所有静默监听端口。现阶段用手工声明补充应用服务。TCP/UDP 按端口映射已经实现；TUN、原始二层桥、组播跨网转发及绑定 `0.0.0.0` 不属于这个入口。

## Agent 与嵌入调用

人、agent 和嵌入父页共用同一 runtime：

```js
await espTester.configureNetwork({mode:'public-relay', autoProvision:false});
// 外网测试须配置实际可用的 relayUrl；本机模式改为 mode:'local-helper',endpoint:helperWebSocketUrl。
// 正常 configure / loadFirmware / start；凭据在加载前配置。
const state = await espTester.readNetwork();
// 未知原页面可以按实际协议请求，也可用浏览器操作 iframe。
const page = await espTester.requestNetworkHttp({interface:1,path:'/',method:'GET'});
const mapping = await espTester.mapNetworkPort({interface:1,protocol:'http',guestPort:80});
// 默认返回 browser:true，在工作台 iframe 内打开；本机映射另返回 url/hostPort。
await espTester.unmapNetworkPort({mappingId:mapping.mappingId});
```

另有 `connectNetwork`、`provisionNetwork`、`addNetworkService`。配置中的密码和助手 token 不出现在 `observe()` / `readNetwork()` 中。详见 [skill API](../../skills/esp-online-tester/references/api.md)。

## 验证边界

- 官方 M5Stack StopWatch 小智 2.2.6（ESP-IDF 5.5.2）原始 BIN：浏览器 ROM 启动，原 SoftAP 关联、固件 DHCP、原始 HTML 和扫描响应、配置保存、退出 AP、STA DHCP、证书校验及真实 HTTPS 服务响应已联通。没有绑定私人账户或完成云端语音对话。
- 网络助手测试覆盖实际 HTTP、验证证书的 TLS、UDP 出站、HTTP 代理和 UDP 入站映射、鉴权与销毁；不能将包级夹具测试扩展为所有固件兼容。
- S3 SoftAP 开放热点有真实固件证据；WPA2 虚拟客户端握手有单测，真实 WPA2 SoftAP 固件尚未验收。其他芯片的 STA 能力仍以对应固件为准；SoftAP 对端目前仅 S3。
- 原生 CLI 的 `--wifi ... --net nat` 仍可独立使用。内部 ICMP 回应不能作为外网证据。浏览器本地网络权限或证书限制时，使用助手提供的本机工作台链接或可信 WSS。

协议与性能证据见 [SoftAP 核心记录](../evidence/softap-client-2026-10-28/README.md)，工作台验收见 [浏览器联调记录](../evidence/network-audio-platform-2026-10-28/README.md)。音频接口见 [PCM 与浏览器设备](audio-runtime.md)。
