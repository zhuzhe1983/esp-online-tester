# CloudZao 独立发布验证

本轮在 `9e0f09c3f5507e0c50be1bb69d5653a40b42a3d4` 上独立导入 CloudZao；适配源码提交为 `fa323de036b8a04377eaf873b83b9ca65c5a4477`，不含 Waveshare 001–029 批。原包校验、共享文件合并与旧 INK 下载源保留证明见 [导入记录](../../cloudzao-publish-import-20261124.md)。

[检查收据](checks.json)保留命令、结果、生产 WASM 的字节数与 SHA-256，以及实际浏览器的帧校验值。两项 Clippy 均在最新 stable Rust 下以 `-D warnings` 通过；Claw 1.54 的三个真实客体测试和两个板内测试通过。490 项平台测试（含真实 Worker/WASM 的 CloudZao 用例）、真实录制回放及 WASM section 检查均通过。静态发布目录为 766,742,796 字节，未超过项目的 1 GB 预算。

浏览器使用生产 Worker/WASM 加载原创第一方 SRAM 诊断，实际观察新的 UART 标记及全部 240×240 RGBA 像素：初始全红，BOOT 按下全绿，释放后与初始帧逐字节一致。390px 嵌入页能恢复产品且无横向溢出。Claw4 的实际渲染器加载、展开复位、固定部件材质隔离及 UNKNOWN 主板标记通过；其几何来源仍为照片测量近似，板级执行仍为 unavailable。

复现浏览器检查时，先按项目文档构建 WASM、提供 ROM 并启动本地静态服务。通过 `tools/fixtures/cloudzao-claw154-probe.mjs` 生成同一份第一方诊断；打开带 `?firmware=<诊断URL>` 的工作台，再用任务自有的 Playwright wrapper session 执行：

```sh
~/.codex/skills/playwright/scripts/playwright_cli.sh run-code --filename tools/fixtures/cloudzao-claw154-browser-check.js
~/.codex/skills/playwright/scripts/playwright_cli.sh run-code --filename tools/fixtures/cloudzao-claw4-model-check.js
```

Claw4 检查脚本移除了末尾分号，使 wrapper 可以直接将其作为函数表达式载入；产品逻辑未因此改变。第一方诊断的 SHA-256 固定为 `666d7b2e553d6788aafc48282ebfec1fb121b76849295437d098b8f55d4ca4f8`。浏览器检查等待真实帧到达后再 capture，UART 完成标记本身不作为帧到达的证明。脚本中的临时嵌入页在 `finally` 中关闭，最外层 session 由调用者按项目生命周期规则关闭。

本地预览因 Origin 不在生产中继允许列表内而出现预期的联网 CORS 拒绝，未据此宣称网络可用。没有执行官方应用的完整验收或连接实物；原有 trial 与其他限制继续保留。截图与原始日志仅保存在忽略的本地输出目录，没有提交私有地址、访问凭据或浏览器会话信息。
