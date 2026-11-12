# PaperS3 EDCBook 验收入口与当前阻挡

[EDCBook 官网](https://edcbook.cn/)推荐的 3.1.2 在官方 M5Burner API 中是 COMMUNITY 项目 `1897200000000000015` 的 PUBLIC/PUBLISHED 版本 `2113660506860756993`，文件名 `edcbook_20261023.bin`，发布时间 `2026-10-24T12:43:07+08:00`。项目和版本的 `supportedDevices` 均明确列出 `PAPER_S3` 与 `PAPER_MONO`；详见[元数据来源和下载结果](evidence/papers3-edcbook/sources-and-blocker.json)。这说明同一镜像适用于两个型号，不能把 PaperMono 的运行结论继承给 PaperS3。

固定镜像为 16 MiB，历史下载 SHA-256 为 `06eb3b595fc47bc5d35f760a16cdc5795ea8c0fad6295bcee8db5268cf906986`（[原有收据](../evidence/s3-light-sleep-edcbook/receipt.json)）；官方元数据未提供发布方 SHA。2026-11-12 访问该固定下载地址返回 HTTP 400 / `BURN_CAPTCHA_INVALID`，随后按现有站点接口调用 OCR，识图步骤在 20 秒上限内未完成（`CAPTCHA_TIMEOUT`），仍未取得镜像。**PaperS3 的 EDCBook 启动、阅读、翻页、局刷和联网均尚未验收。** 官方 v0.5 的黑白显示、TF 和 RTC 结果只属于 v0.5。

`tools/probe-papers3-edcbook.mjs` 使用真实页面 worker/WASM、PaperS3 板型和隔离 Flash/TF 存储，先核对镜像 SHA，再从 ROM 启动。镜像可通过调用方提供的本地文件输入；原始远程出处仍须显式传入：

```sh
node tools/probe-papers3-edcbook.mjs \
  --url 'https://burner.m5stack.com/api/v1/firmwares/1897200000000000015/versions/2113660506860756993/download' \
  --sha256 06eb3b595fc47bc5d35f760a16cdc5795ea8c0fad6295bcee8db5268cf906986 \
  --file <local-image.bin> \
  --output <local-output-directory>
```

出现 `READY` 后，从标准输入逐行发送 JSON 动作；`--actions <file.json>` 可改为读取动作数组。坐标必须依据实际启动画面确定，不从 PaperMono 复制。示例动作格式：

```json
{"op":"capture","name":"library"}
{"op":"tap","x":100,"y":200,"hold":0.15,"settle":2}
{"op":"wait","seconds":3}
{"op":"capture","name":"reading-page"}
{"op":"observe"}
{"op":"quit"}
```

后续依次核对内置阅读指南、下一页和上一页的实际文字与页码，以及局刷后旧页残留、行缺失、方向和裁切。总帧 hash 改变本身不算显示正确。共享 ET047TC1 仍是黑白数字端点；没有光学标定，不将多脉冲计数伪造成 16 灰阶。

联网验收须由客体真正启动热点/STA 和 HTTP 服务，再观察实际响应。探针的 `http` 动作将 `options` 传给 `runtime.requestNetworkHttp`；需要本机网络 helper 时，可在加载前用 `--network-config-file <private-json>` 读取其中的 `endpoint`，以 `autoProvision:false` 配置。热点请求是否成功、网页内容及读写结果都必须保留独立证据；空扫描、输入回执或配置成功不能代替网络结果。

探针输出的完整串口、截图、HTTP 结果和动作记录仅保存在调用方指定目录，可能包含私人网络或文件信息，提交前须另行精简。无论成功或异常都释放 worker；本轮没有用浏览器运行 EDCBook，也没有把远程镜像提交或镜像到服务器。
