# 浏览器 Wi-Fi 与 AP 配网：现状和实施方向

核对日期：2026-10-27；代码基点 `7d03d240`。本文是实现边界与后续方案，不代表下述配网桥已经完成。

## 当前可用的部分

底层 `esp-soc/src/wifi.rs` 实现虚拟 AP，供固件作为 STA 扫描、关联并取得地址；`esp-soc/src/net.rs` 提供 DHCP、ARP、DNS、SNTP 等虚拟局域网服务。支持范围以各芯片的实际测试为准，不能据此宣称所有 ESP32/IDF 版本均兼容。

原生 CLI 使用 `esp-soc/src/nat.rs` 的主机 TCP/UDP socket 转发出站流量；浏览器 ABI `wasm/src/s3.rs::esp32sim_wifi` 明确传入 `nat=None`，没有这个真实联网能力。离线 DNS 的回答和内部 ICMP 回应不能当作互联网连通证据。现有产品页仅从固件 manifest 的 `wifi` 字符串配置虚拟 AP，完整 BIN 和 flasher_args 上传没有独立的 Wi-Fi 场景配置入口。

固件显示 AP SETUP/SSID，不表示电脑实际广播了 Wi-Fi 热点。目前还缺少模拟手机作为 STA 连接固件 SoftAP 的通路，也缺少从网页访问固件内部 HTTP 服务器的入站桥。底层虽然已有 `wasm/src/ethernet.rs` 的帧收发 ABI，产品 worker/UI 尚未接入；这组 ABI 本身也不能替代 SoftAP 对端、TCP 或浏览器页面路由。

## 纯静态页面内可以实现的配网流程

1. 在场景中定义虚拟路由器及可见 SSID、信号/断开场景，统一用于人机界面和 agent API；使用测试网络信息，不要求用户填真实家庭密码。
2. 补充虚拟 STA，与固件自己的 SoftAP 完成真实协议交换，再从该虚拟客户端建立 TCP/HTTP 请求。目标是调用固件原有 Web 服务，不在前端假造“配网成功”或直接篡改固件 NVS。
3. 提供“打开配网页面”入口。HTTP 适配层把请求转入虚拟网络；Service Worker / 隔离页面负责将 HTML、资源、表单和后续请求路由回虚拟服务。需要明确根路径、跳转、Cookie、WebSocket 和隔离边界；简单 iframe 指向 `192.168.4.1` 不会访问到虚拟设备。
4. 固件收到配置后自行保存、重启并连接虚拟路由器。复用已有按设备和固件内容隔离的 Flash 持久化；不为配网自动清空用户数据。分别显示“连接虚拟路由器”“获得地址”“可访问真实网络”，避免混为一个联网图标。

ESP-IDF 标准 SoftAP provisioning 使用 HTTP/protocomm，未必提供人类可点击的 HTML 页面；有的固件使用自定义配网页面，有的需要匹配的协议客户端。因此通用 HTTP 桥与具体 provisioning 客户端须分开实现，不能承诺一个表单适配所有固件。[ESP-IDF 5.5 provisioning 文档](https://docs.espressif.com/projects/esp-idf/en/v5.5/esp32/api-reference/provisioning/wifi_provisioning.html)

## 访问真实互联网需要单独的通路

普通 GitHub Pages 网页没有任意 TCP/UDP socket。Chrome 的 Direct Sockets 属于安装式 Isolated Web App 能力，不能直接当作 github.io 页面的能力。[Chrome 官方说明](https://developer.chrome.com/docs/iwa/direct-sockets)

保持静态前端的方案是提供可选 WSS 网络中继，连接本机或自部署的 TCP/UDP 网关；设备 TLS 仍在固件内处理，保留证书与时间检查。网页 fetch 只能支持特定 HTTP 接入并受浏览器跨域规则限制，不能透明替代任意 MQTT、原始 TCP/UDP 和固件里的 TLS。

不依赖任何服务的模式则提供浏览器内的测试服务器和可复现响应，用于离线业务流程测试。不得将这些模拟响应标成真实云端结果。中继地址/凭证不默认随分享链接传播。

## 实施与验收顺序

先做虚拟 STA 连接 SoftAP + HTTP 入站，使用内部源码固件验证表单提交、实际 NVS 写入和重启恢复；再接一个用户固件的配网流程。随后补虚拟路由器 UI/scene/API、断连/错误密码等失败场景，最后接可选真实联网中继。每步观察固件事件、串口和协议结果，不能只检查屏幕或主机侧“已发送”。这是共用引擎能力，各产品另核对芯片、驱动版本和专用网络器件。
