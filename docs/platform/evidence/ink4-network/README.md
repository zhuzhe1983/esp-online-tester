# 普通 INK4 官方 2.0.53 自动配网与真实 OTA 响应

2026-11-12，普通 `metalio-ink4`。固定官方镜像在 Node WASM 与无头 Chromium 中，均从干净 Flash 自动完成 SoftAP 配置、固件重启、STA 获取地址，并收到真实外部 OTA 响应。固件随后显示待账户激活的主页；没有执行激活，也没有把助手连接成功当作应用联网成功。

固定远程固件与来源沿用 [TF 证据](../ink4-tf/README.md)；本次实际 WASM 为 `8d8ccce696666eeb7a355cdbe15fd398eaec9ddb9ae9ade85544d91639b4fb20`。运行时采用 `b6d133dd` 的完成页识别修复，本分支等价 cherry-pick 为 `9409a71f`。来源、负例、状态、TCP 字节计数、帧与原始记录 hash 见 [receipt.json](receipt.json)。

旧流程保存凭据后返回 `PROVISIONING_UNSUPPORTED`。实际固件成功页 HTTP 200 的脚本是 `fetch('/reboot', { method: 'POST' })`，旧识别器只支持 `/exit`。诊断时按真实页面发出一次 `/reboot`，客体随即正常重启并联网；修复后正式验收完全自动执行，不再手动请求。运行时仅接受成功页明确声明的同源 `/exit` 或 `/reboot` POST，不猜测任意重启路径。

Node 验收在 30 客体秒记录 6 条 TCP 连接、0 次拒绝、发送 3,230 字节、接收 1,584 字节；浏览器在 33.74 客体秒记录 7 条连接、0 次拒绝、发送 3,542 字节、接收 1,765 字节。两者 `connection`、`provisioning` 均为 `connected`，STA `leaseReady=true`，并由固件日志确认 OTA 响应解析和待激活状态。真实浏览器帧为 480×800，标题、时钟、日期与六个应用清晰，无旧标题叠字；更严格的切页像素回归另见 [显示证据](../ink4-display/README.md)。

复现须先按项目说明准备 WASM/ROM，并启动自己的本地网络助手。私有 JSON 仅需 `endpoint` 字段，不要把含 token 的地址放入命令、文档或版本库：

```sh
ESP32SIM_HELPER_CONFIG=/path/to/private-helper.json \
node tools/probe-ink4-network.mjs \
  https://cloudzaoai.oss-rg-china-mainland.aliyuncs.com/OTA/xiaozhi/eca923cb-7e07-45d6-9e83-0aefe6e48a46.bin \
  1d02cba522fe1aec5b2c160973c6a15d89cbf0e13a5794a42faf6e3f15587919 \
  output/ink4-network 30
```

浏览器在助手同源的 `/platform/` 页面，通过公开 `espTester` API 配置普通 INK4 和固定远程固件，加载前设置 `configureNetwork({mode:'local-helper',endpoint,autoProvision:true})`，然后正常加载、启动，等待固件 OTA 输出，再检查 `readNetwork()` 和实际 `capture()`。测试完成后 dispose 运行时、关闭专属 wrapper 会话，并核实其已退出。首次 TF 浏览器测试中的公共 relay 跨本地 Origin 失败不构成该 relay 的可用性验收；这里仅验收配置正确的本地助手链路。

探针输出包含原始串口与真实截图，可能含设备标识和激活码，只保留在 ignored `output/`。提交收据去掉这些内容、helper 地址及网络标识，保留原始文件 hash 和数值证据。未验证账户激活、登录后语音/对话、音频、BLE、4G、RF、物理硬件或其他固件版本；未修改官方镜像，也未更新公开固件兼容级别。
