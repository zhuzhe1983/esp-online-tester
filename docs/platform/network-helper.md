# 浏览器网络助手

网络助手在本机提供真实 TCP/UDP 出站连接、虚拟 SoftAP 的 DHCP 客户端，以及固件服务到本机端口的映射。前端和固件仍在浏览器中执行；助手不运行另一份模拟器。GitHub Pages 仍只发布静态文件。

## 启动

在仓库根目录执行：

```sh
cargo build --release -p esp-network-backend
node tools/network-helper.mjs
```

打开终端显示的本机工作台 URL；它携带本次随机连接凭据，自动使用同一助手。本机工作台需要先按平台运行说明准备 WASM 与 ROM。默认仅监听 `127.0.0.1:8787`，停止命令会关闭其网络会话和端口映射。

使用 github.io 工作台时，把终端显示的 `Pages connection endpoint` 填入网络连接地址，或将它 URL 编码后放入工作台的 `#network=` 片段。凭据仅属于这次助手进程，不应写入仓库、分享链接、录制或截图。浏览器可能要求本地网络访问授权；如果它不允许 HTTPS 页面连接本机助手，使用终端显示的本机工作台 URL，或配置可信的 WSS。

```sh
node tools/network-helper.mjs --port 8787 --cert /path/to/trusted-cert.pem --key /path/to/key.pem
```

证书必须被浏览器信任并匹配所访问的主机；不关闭证书验证。TLS 参数同时用于助手 HTTPS/WSS 与 HTTP 服务的独立代理页面。不同来源的开发工作台可通过重复的 `--allow-origin https://your-origin` 显式加入允许列表。远程部署应使用有认证的 TLS 反向代理；助手自身保持回环监听。

## 地址、配网和服务

- 接口 `0` 是固件 STA 所连接的虚拟路由器：网关 `10.0.2.2`，默认 DHCP 地址 `10.0.2.15`。DNS、TCP、UDP 通过原生 NAT 访问实际主机网络；TLS 握手仍由固件执行。
- 接口 `1` 是连接固件 SoftAP 的虚拟客户端。关联完成后，助手发送真实 DHCP DISCOVER/REQUEST，并从固件 DHCP ACK 学得客户端和服务端地址。不会把静态 `192.168.4.1` 当成所有固件的固定地址。
- HTTP 服务的“打开”入口使用独立回环端口的代理 origin，支持相对路径、表单、HTTP 请求和 WebSocket Upgrade；同设备重定向转为该代理内的路径。保留固件 CSP / X-Frame-Options 等响应头，禁止嵌入的页面仍需要独立窗口。固件脚本中硬编码的 IP 访问不保证能被代理重写。
- HTTPS 服务使用原始 TCP 映射，浏览器仍验证固件证书，主机名不匹配会正常报错。TCP/UDP 映射都只绑定回环地址，且仅属于当前网络会话；断开、重新加载固件或停止助手后释放。
- 映射不能证明应用协议可用；请检查实际 HTTP 响应、MQTT 握手或目标协议的数据往返。现有 NAT 的 ICMP 响应是本地合成结果，不能用 ping 作为外网验收。

## 传输协议 v1

WebSocket 路径 `/network?token=<临时凭据>`，校验允许的 Origin 与随机 token。每个连接独立维护两个网络接口；最多八个连接、每个会话最多八个公开映射。没有相应 token 的网页不能用助手访问本机网络。

服务端首先发送：

```json
{"type":"hello","version":1,"interfaces":[0,1],"capabilities":{"ethernet":true,"tcp":true,"udp":true,"http":true,"https":true,"maxFrameBytes":1518}}
```

二进制消息为 `[interface:u8][Ethernet II，无 FCS，14–1518 字节]`，双向相同。接口 1 在收到 `interface` 启用消息前不发送 DHCP。文本控制消息有 `id`；响应为 `{id,result}` 或 `{id,error:{code,message}}`。

| 请求 | 用途 |
| --- | --- |
| `{id,op:'interface',interface:1,enabled:true,mac:'02:53:49:4d:00:03'}` | SoftAP 关联后启动虚拟客户端 DHCP；关闭后重建该接口并释放映射 |
| `{id,op:'status'}` | 读取接口与当前映射 |
| `{id,op:'map',interface:0,protocol:'tcp'|'udp'|'http'|'https',guestPort:80,hostPort:0}` | 创建回环映射；0 表示让系统分配端口；HTTP 返回隔离代理 URL |
| `{id,op:'unmap',mappingId}` | 释放映射及其连接 |
| `{id,op:'http',interface:1,guestPort:80,path:'/',method:'GET',headers?,body?,bodyBase64?}` | 在真实虚拟 TCP 连接上向固件发 HTTP 请求；响应含 `status,headers,body,bodyBase64`；`body` 是解码后的 UTF-8 文本，`bodyBase64` 保留原始 HTTP 实体字节 |

状态事件为 `{type:'network-status',interface,enabled,ready,leaseReady,address,guestAddress,addressSource,counters,mappings}`。HTTP 请求体最多 32 KiB、响应最多 2 MiB、超时 90 秒；文本控制消息最多 64 KiB，最多 16 个进行中的请求。支持 gzip、deflate 和 br 响应解压；压缩实体与解压结果分别限为 2 MiB。原始响应头保留，编码响应另附 `decodedBodyBase64`；无效编码、未知编码或解压超限返回 `HTTP_DECODE_ERROR`。大音频流应走 PCM 接口，不经过这个 HTTP 控制接口。

`/network-info` 为本机同源工作台返回 `{version,endpoint}`；它不向跨来源网页公开 token。助手仅提供调用者明确选择的网络访问，不进行全端口扫描或自动建立局域网共享。

## 验证与来源

```sh
cargo test -p esp-soc --lib
cargo build --release -p esp-network-backend
node tools/network-helper-test.mjs
```

可通过 `ESP_NETWORK_BACKEND=/absolute/path/to/esp-network-backend` 指定测试二进制。测试用虚拟 Ethernet/TCP 对端穿过实际 WebSocket 和原生 NAT，访问本机真实 HTTP、TLS、UDP 服务，并检查 HTTP 代理、UDP 入站映射及释放。TLS 测试显式信任临时测试 CA，保持证书验证；这些包级测试不代替实际固件配网验收。

入站 TCP 状态机复用 [上游 PR #278](https://github.com/mikroverk/esp32sim/pull/278)，原提交 `5e7d8bc728462ca37d1c60df660a7f5a4b6b7a03`；增加了独立接口网关地址、解除映射与连接释放、SYN-ACK 校验和有界接受队列。[上游 PR #318](https://github.com/mikroverk/esp32sim/pull/318) 的传输 trait 已评估：本助手在原生进程使用现有 socket NAT，不另写一份同用途 trait。
