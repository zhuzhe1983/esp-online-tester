# 公共网络中继

公共中继让 GitHub Pages 中的固件经 WSS 访问真实互联网。开发者使用网页时不需要克隆项目、运行程序或安装插件。网站维护者需要在有公网地址的服务器上部署一次中继；GitHub Pages 只托管静态文件，不能运行这个服务。

固件配网页由浏览器内的虚拟网络客户端访问，不经过公共中继。公共中继只处理 station 接口 0 的出站流量；本机助手仍提供局域网访问和入站端口映射，属于高级选项。

## 维护者部署一次

准备一台可以运行 Docker Compose 的 Linux 服务器和一个已指向服务器公网地址的域名。开放 TCP 80、443；服务器需要允许出站 TCP 80/443/1883/8883 和 UDP 53/123。服务器已有占用 80/443 的反向代理时，请把下面的 Caddy 配置合并到已有代理中。

在项目根目录运行，使用自己的实际域名替换示例：

```sh
bash deploy/network-relay/start.sh relay.your-domain.tld
```

脚本生成本地 `.env` 和随机代理密钥，随后构建 Rust 后端、Node 服务并启动 Caddy。Caddy 自动申请 TLS 证书；只有 Caddy 的 80/443 端口对外发布。密钥文件不提交 Git，不发送给浏览器。重复运行脚本复用已有配置。证书和密钥保存在持久卷及 `.env` 中。

部署后检查 `https://自己的域名/health` 返回 `{"ok":true,"version":1,"service":"esp-network-relay"}`，再将网站的公共中继地址配置为 `https://自己的域名`。这里不提供尚未部署的默认公网地址，也不把示例域名当作可用服务。

使用已有代理时，必须支持 WebSocket、保留原始 `Host`，并覆盖 `X-Relay-Client-IP` 为连接方 IP、`X-Relay-Proxy-Token` 为服务器端密钥。不要信任客户端传入的同名头。缺少代理密钥时中继按实际 socket 对端限额，所有通过代理的访客会共享该 IP 的配额。Caddy 的转发与日志配置依据 [reverse_proxy 文档](https://caddyserver.com/docs/caddyfile/directives/reverse_proxy)和 [log filter 文档](https://caddyserver.com/docs/caddyfile/directives/log)。不要记录 `/network` 的 token 查询参数。

## 本地预览与验证

```sh
cargo build --release -p esp-network-backend
RELAY_ORIGINS=http://127.0.0.1:8080 node tools/network-relay.mjs
```

默认监听 `127.0.0.1:8788`；在同源为 `http://127.0.0.1:8080` 的本地平台预览中使用该地址。其他开发服务器端口须相应修改 `RELAY_ORIGINS`。这用于维护者验收，不是最终使用者必须安装的程序。HTTPS 网页连接本地 HTTP 地址可能受浏览器本地网络权限限制；公开部署使用 HTTPS/WSS。

```sh
node --test tools/network-relay-test.mjs
node tools/network-helper-test.mjs
```

第一组验证会话令牌、Origin/IP 绑定、限额、目的地址拦截和禁止入站操作，使用可观察的测试后端，避免把测试流量发送到第三方。第二组用真实 Rust 后端验证 TCP、TLS、UDP 和本机助手兼容性。两组都不能代替部署后的真实公网连通性检查。

维护者可以另外显式提供公网目标，运行真实中继的 TCP 建连和 DNS/UDP 检查（未设置这些变量时相应测试跳过）：

```sh
ESP_RELAY_TEST_TARGET=自己的公网IPv4:443 ESP_RELAY_TEST_DNS=公共DNS的IPv4 node --test tools/network-relay-test.mjs
```

TCP 检查验证原生后端成功建连，DNS 检查验证真实解析响应；完整固件 TLS/HTTP 请求仍应在部署后的网页中验收。

## 浏览器与 agent 契约

向 `<relayBase>/session` 发送空 body 的 `POST`。浏览器正常发送 `Origin`，无需 cookie、长期密钥或登录。允许的 Origin 为精确匹配列表；iframe 内使用平台实际 Origin。成功返回 HTTP 201：

```json
{
  "version": 1,
  "endpoint": "wss://relay.your-domain.tld/network?token=<one-use-token>",
  "expiresAt": "<ISO-8601 token deadline>",
  "sessionExpiresAt": "<ISO-8601 session deadline>",
  "capabilities": {
    "publicRelay": true,
    "interfaces": [0],
    "ethernet": true,
    "tcp": true,
    "udp": true,
    "http": false,
    "https": false,
    "inbound": false,
    "tcpPorts": [80, 443, 1883, 8883],
    "udpPorts": [53, 123],
    "maxFrameBytes": 1518
  },
  "limits": "<object containing the active numeric limits>"
}
```

`http` / `https` 能力为 false 指没有助手的 HTTP RPC 和端口映射；固件经 TCP 80/443 发送 HTTP/TLS 流量仍可用。HTTP、MQTT、TLS 加密内容由固件生成，转发器不解密 TLS。

使用返回的 endpoint 建立 WebSocket。令牌 30 秒内有效，只能用一次，且绑定签发时的 Origin 和客户端 IP。连接后的 `hello` 使用 v1 协议；二进制消息格式仍是一个接口编号字节 `0` 加完整 Ethernet frame。只有 JSON `{"id":"...","op":"status"}` RPC 可用；`http`、`map`、`unmap`、`interface` 均返回 `PUBLIC_RELAY_OUTBOUND_ONLY`。浏览器配网页使用浏览器内客户端，本机端口映射使用本机助手。

配额不足时 `POST /session` 返回 429，`error.code=RELAY_BUSY`；Origin 不匹配返回 403。会话时间、流量或速率超限时以 WebSocket 1008 关闭。前端应显示原因并提供重试，不要无限立即重连。每次重连先申请新会话，不能复用旧 token。endpoint 和 token 只保留在当前会话，不能写入分享链接、录制文件或公共日志。

## 默认范围与资源上限

| 项目 | 默认值 |
| --- | --- |
| TCP 目标端口 | 80、443、1883、8883 |
| UDP | 有界 A/AAAA DNS 查询；48 字节 NTP 客户端查询 |
| DNS 上游 | 固定公共 IPv4 resolver `1.1.1.1`，不读取宿主私有 DNS |
| 同时会话 | 全局 16；每客户端 IP 2（待用 token 也占名额） |
| 申请频率 | 每 IP 每分钟 6 次 |
| 会话期限 / 空闲期限 | 10 分钟 / 90 秒 |
| 双向总流量 | 每会话 64 MiB |
| 双向速率 / 突发 | 256 KiB/s / 512 KiB |
| 入站消息频率 | 每会话 1000 条/秒 |
| 新出站连接 | 每会话每分钟 120 个不同流 |
| 原生 NAT | 每会话最多 64 TCP 流、64 UDP 流，最多 8 个并行阻塞连接尝试 |

使用 `RELAY_TCP_PORTS`、`RELAY_UDP_PORTS` 显式扩大端口范围，使用 `RELAY_DNS` 更换公共 IPv4 resolver。自定义 UDP 端口转发真实 UDP 数据；默认不提供任意 UDP、IPv6、原始 IP、入站连接或私网访问。UDP 53 始终只接收有界 DNS 查询，UDP 123 始终只接收 NTP 客户端查询。较大 DNS 响应需要 TCP 重试时，默认策略会拒绝 TCP 53。

每个出站 IPv4 包都检查实际目的 IP，拒绝 loopback、RFC1918、CGNAT、link-local、metadata 地址、保留/文档/组播地址以及 IPv4 分片和选项。DNS 响应即使将域名指向私网，也不能绕过后续逐包检查。DNS 仅有虚拟地址 `10.0.2.3:53` 的固定公共 resolver 例外，DHCP 在虚拟路由器内处理。地址范围依据 [IANA IPv4 Special-Purpose Registry](https://www.iana.org/assignments/iana-ipv4-special-registry/)，额外保守拒绝全部组播。

Origin 限制方便网站隔离，并不能阻止非浏览器客户端伪造 Origin，因此公共部署必须保留限额、出站过滤和进程隔离。本实现不提供匿名无限量出口。启动脚本提供基本有界部署；大规模开放时维护者还需按运营需要增加防滥用、预算告警和服务器防火墙规则。
