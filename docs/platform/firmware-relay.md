# 固件下载中继

GitHub Release 的下载地址及其重定向目标不保证提供 CORS 响应头；`github.io` 与 `github.com` 也不是同源。Meshtastic 的单板 BIN 地址有相同限制。浏览器能点击下载这些文件，但网页 JavaScript 可能无法读取它们。

配置固件中继后，工作台、mini、agent 加载和 Web Serial 准备流程会将这些**固定 SHA-256 的原始 BIN**交给中继下载。中继连接发布方，流式发送原始字节，浏览器仍检查 SHA-256、芯片、容量和布局，再按原始 URL + hash 缓存。来源、版本、分享配置和下载进度保留发布方 URL。中继没有二进制文件写入或磁盘缓存；站点仍为静态部署。

## 部署与配置

已有[公共网络中继](public-network-relay.md)提供同一域名下的 `/firmware` 路由；重新构建容器即可带上这项功能及当前固件目录。在 `web/platform/network-config.json` 中设置 `publicRelayUrl` 后，固件下载默认复用它。

也可以独立运行 Node.js 22+ 服务，无需 Rust 网络后端。下面是本地预览示例：

```sh
FIRMWARE_RELAY_ORIGINS=http://127.0.0.1:8080 node tools/firmware-relay.mjs
python3 -m http.server 8080 --directory web
```

预览时将 `web/platform/network-config.json` 配置为：

```json
{
  "publicRelayUrl": "",
  "firmwareRelayUrl": "http://127.0.0.1:8789"
}
```

生产部署需要实际 HTTPS 域名：设置 `FIRMWARE_RELAY_PUBLIC_URL=https://自己的中继域名`，在前面使用有有效证书的反向代理并保留 `Host`。默认只监听回环地址；容器中可设置 `FIRMWARE_RELAY_HOST=0.0.0.0`，端口用 `FIRMWARE_RELAY_PORT` 指定。`FIRMWARE_RELAY_ORIGINS` 是逗号分隔的精确页面 Origin，默认仅允许本站的 GitHub Pages Origin。独立服务按 socket 对端 IP 限额；反向代理后的用户共享代理 IP 配额。需要按实际用户 IP 区分配额时，使用带代理密钥校验的公共网络中继集成。

生产网站的 `firmwareRelayUrl` 填入该 HTTPS Origin。省略这个字段会复用 `publicRelayUrl`，显式设为空字符串则关闭固件转发。中继地址不从 scene、分享链接或固件清单读取。仓库不附带未经部署验证的公共服务地址。

中继启动时读取 `web/platform/catalog/firmwares.json`。网站更新固件版本后，中继也须更新目录并重启／重建；旧目录不会自动允许新 URL 或新 hash。容器部署已包含该 JSON 及模块依赖。

## 请求与限制

`GET /firmware?url=<编码后的发布方URL>&sha256=<64位小写hash>`，只允许目录中登记的 URL 和 hash 配对。支持固定版本的 GitHub Release `.bin` 与 `release.meshtastic.org` 的固件 BIN。已有 CORS 的厂商 CDN、未固定 hash 的输入、ZIP、清单和其他自定义来源仍走原来的直接下载链路。

默认限制：单个文件 64 MiB、单次请求 60 秒、全局并发 4、每 IP 并发 2、每 IP 每分钟 12 次。允许 GitHub 发布资产 CDN 的 HTTPS 重定向，最多 3 跳；每次连接解析并固定一个经过检查的公网 IPv4，保留 TLS 证书验证。拒绝私网、回环、链路本地、保留地址、用户凭据和任意主机跳转。IPv6 出站尚未实现。

客户端断开会取消上游请求。流式读取遵循下游背压，校验实际字节数和 hash，响应不转发上游 cookie、Location 或 Content-Disposition。服务器可能在流式校验完成前发送部分字节，因此**浏览器最终 SHA-256 校验始终不可省略**。中继错误响应返回 `X-Firmware-Relay-Error`；上游 404/410 保持原 HTTP 状态，不伪装成 CORS 问题。配额或中继不可用时返回明确错误，可使用[下载后导入的恢复入口](local-firmware.md#远程-bin-下载失败后恢复)。

Origin 限制不能代替限额或出站过滤，因为非浏览器客户端可以伪造 Origin。服务不是通用下载代理，也不接收用户的私有 URL、登录凭据或 GitHub token。

## 验证

```sh
node --test tools/platform-firmware-relay-test.mjs tools/network-relay-test.mjs
node --test tools/platform-config-test.mjs tools/platform-runtime-test.mjs tools/platform-input-test.mjs tools/platform-view-test.mjs tools/platform-flasher-test.mjs tools/platform-firmware-recovery-test.mjs
```

自动化检查使用注入的可观察上游，覆盖来源配对、CORS、重定向、DNS 地址固定、取消、超时、限额、内容错误，以及进入运行时前的 hash 校验；不把测试替身当作真实外网连通性证据。网络中继的真实 TCP/DNS 测试仍需调用方指定目标。

2026-10-28 真实发布方经本地中继流式读取成功，响应允许 GitHub Pages Origin，未保存固件文件：Meshtastic T-Deck 2.7.26.54e0d8d 为 2,306,304 字节，SHA-256 `f55469e588ea79ded5fa4fce58900bc0252021ab02e79329dc77bedfce40ab13`；Bruce T-Deck 1.16.1 为 4,262,592 字节，SHA-256 `16b57f1371756df6ee6bbc4500bc5ae4c288f6d78a8c25edb55062d7ad120664`。均与目录固定值一致。上线仍需实际部署目标与 HTTPS 页面验收。

同日无头 Chromium 检查使用 `python3 -m http.server 8790 --bind 127.0.0.1 --directory web` 和 `FIRMWARE_RELAY_ORIGINS=http://127.0.0.1:8790,https://zhuzhe1983.github.io node tools/firmware-relay.mjs`。浏览器仅替换 `network-config.json` 响应为本地中继配置，固件请求访问真实发布方，未提供模拟固件响应。工作台选择上述 Meshtastic，mini 页面选择上述 Bruce，点击运行并确认后，两者均经一次 `/firmware` 请求完成下载和校验，浏览器未向原始 BIN 地址直接发起请求，也未出现手动恢复对话框。`observe().firmware.artifacts` 保留以上原始 URL、字节数和 hash；`waitFor({status:'running',serialIncludes:'ESP-ROM'})` 通过。Bruce 的 `capture()` 返回 320×240、frameNumber 253，截图显示 Wi-Fi 菜单。此结果验证真实下载与执行入口，不代表全部外设功能兼容或实机刷写通过。
