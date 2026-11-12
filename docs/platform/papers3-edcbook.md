# PaperS3 EDCBook 显示与阅读验收

[EDCBook 官网](https://edcbook.cn/)推荐的 3.1.2 在官方 M5Burner API 中是 COMMUNITY 项目 `1897200000000000015` 的 PUBLIC/PUBLISHED 版本 `2113660506860756993`，文件名 `edcbook_20261023.bin`，发布时间 `2026-10-24T12:43:07+08:00`。项目和版本的 `supportedDevices` 均明确列出 `PAPER_S3` 与 `PAPER_MONO`；详见[元数据来源和下载结果](evidence/papers3-edcbook/sources-and-blocker.json)。这说明同一镜像适用于两个型号，不能把 PaperMono 的运行结论继承给 PaperS3。

固定镜像为 16 MiB，下载 SHA-256 为 `06eb3b595fc47bc5d35f760a16cdc5795ea8c0fad6295bcee8db5268cf906986`（[原有收据](../evidence/s3-light-sleep-edcbook/receipt.json)）；官方元数据未提供发布方 SHA。2026-11-12 访问该固定下载地址返回 HTTP 400 / `BURN_CAPTCHA_INVALID`，随后按现有站点接口调用 OCR，识图步骤在 20 秒上限内未完成（`CAPTCHA_TIMEOUT`），当时未取得镜像。随后调用方提供了相同 SHA-256 的原始完整 BIN，按原有 ROM 启动链完成了本页的实际验收；早期下载失败保留在来源记录中。官方 v0.5 的 TF、RTC 和联网结果仍只属于 v0.5。

2026-11-12 的固定镜像在 CPU 修复 `43792396` 与显示适配 `583d9a8d` 后，真实页面 Worker 已显示 540×960 首页，并通过触屏进入内置“快速入门指南”、翻到“按键功能”、返回第一页。四次观察的累计扫描数为 76、148、197、246，始终只有一次 ROM 启动，没有客体停机或 panic。[运行收据](evidence/papers3-edcbook/rgb-runtime-20261112.json)保留镜像/WASM/截图 hash、指令数、输入及负例；截图和原始串口仅保留在忽略的输出目录。

原故障分为两层：`EE.LDXQ.32` 未实现使固件停在 `0x420a0da0`；补齐指令后，固件已输出 LCD_CAM RGB 流，但 PaperS3 只有 i80 显示入口。EDCBook 使用 EPDiy 的 RGB+RMT 接口，与已验收官方 v0.5 的 M5GFX i80 不同。显示适配依据厂家文档固定的 [EPDiy d84d26eb](https://github.com/vroland/epdiy/blob/d84d26ebebd780c4c9d4218d76fbe2727ee42b47/src/output_lcd/lcd_driver.c)：每行一个 dummy 字节加 240 个四像素驱动字节，540 行向 8 对齐为 544 行；LCD D8..D15 按物理两位组路由至面板，PCLK/HSYNC/DE 分别接 G16/G15/G13，G18 接有限循环 RMT CKV。S3 GPIO/RMT 值按 IDF 5.5.1 检查，并与这份固件的 5.5.2 头文件交叉核对。

适配从真实 DMA 字节更新既有 ET047TC1 黑白端点。LCD START 时核对供电、SPV 扫描起点、物理引脚与正在运行的有限 CKV 时钟，并在整帧完成后提交像素；没有改动固件、强制板型或用主机绘制替代页面。该入口不模拟逐边沿相位、扫描期间停止/改配 RMT 的竞态或模拟电压积分。

**显示仍有明确限制。** 返回第一页的文字、版式、方向及裁切与初次阅读一致，但 518,400 个像素中有 388 个不同，范围为 x35..507/y92..328，位于正文的字形区域。原因未单独隔离，不能据此认定局刷像素一致；16 灰及光学残影没有标定。默认 64 MiB FAT32 卡只是已附加的输入，本轮未证明 EDCBook 的文件系统挂载或文件读写。其启动先尝试 SDMMC 时仍有报错；内置指南不构成 TF 验收。热点、STA 和 HTTP 联网也没有从本轮阅读结果继承通过。

随后 Chromium 重载暴露了独立的电池输入缺口：屏幕显示“请及时充电”，扫描停在 82 次，触摸不再进入阅读。原模型把充电状态 G4 保持为高（未充电），却未给电池分压 ADC 提供输入；未驱动的 ADC 默认 0V。厂家 C139 V1.0 原理图的 R24/R25 均为 22k，电池电压经 2:1 分压接到 GPIO3；固定 M5Unified 源码对应 ADC1_CH2 和 2.0 倍还原。重载后为何更早进入低电分支尚未单独隔离，不能把 NVS 持久化或前一次渲染修复当成已证实的原因。

`f9ef5fe0` 为 PaperS3 声明了可覆盖的标称 4.0V 电池：在板级装配时，仅向未驱动的 G3 提供 2.0V 模拟源，经既有 SAR 转换和客体校准得到电压；不强制充电，也不改固件的低电阈值或 NVS。显式 host 电压、波形流和原始码（包括零）不会被默认值覆盖，重挂、客体重启和保留式轻睡唤醒均维持已有输入。两个定向测试覆盖实际 ADC 读数与这些边界；[电池来源和负例收据](evidence/papers3-edcbook/battery-default-20261112.json)记录详情。它是固定电压的模拟初始状态，没有实现充放电曲线、容量消耗、压降或独立电源控制器；浏览器首启及重载结果由整合验收补充。

`tools/probe-papers3-edcbook.mjs` 使用真实页面 worker/WASM、PaperS3 板型和隔离 Flash/TF 存储，先核对镜像 SHA，再从 ROM 启动。镜像可通过调用方提供的本地文件输入；原始远程出处仍须显式传入：

```sh
node tools/probe-papers3-edcbook.mjs \
  --url 'https://burner.m5stack.com/api/v1/firmwares/1897200000000000015/versions/2113660506860756993/download' \
  --sha256 06eb3b595fc47bc5d35f760a16cdc5795ea8c0fad6295bcee8db5268cf906986 \
  --file <local-image.bin> \
  --output <local-output-directory> \
  --boot-seconds 12 \
  --actions docs/platform/evidence/papers3-edcbook/reading-actions.json
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

内置指南的首页→下一页→上一页动作已记录在 [reading-actions.json](evidence/papers3-edcbook/reading-actions.json)：进入阅读点击 (270,753)，下一页 (480,480)，上一页 (55,480)。本轮截图没有显式页码，按实际章节文字区分页面。进一步的局刷验收仍需核对旧页残留、行缺失及像素差异；总帧 hash 改变本身不算显示正确。共享 ET047TC1 仍是黑白数字端点；没有光学标定，不将多脉冲计数伪造成 16 灰阶。

联网验收须由客体真正启动热点/STA 和 HTTP 服务，再观察实际响应。探针的 `http` 动作将 `options` 传给 `runtime.requestNetworkHttp`；需要本机网络 helper 时，可在加载前用 `--network-config-file <private-json>` 读取其中的 `endpoint`，以 `autoProvision:false` 配置。热点请求是否成功、网页内容及读写结果都必须保留独立证据；空扫描、输入回执或配置成功不能代替网络结果。

探针输出的完整串口、截图、HTTP 结果和动作记录仅保存在调用方指定目录，可能包含私人网络或文件信息，提交前须另行精简。无论成功或异常都释放 worker。探针默认等待 30 个客体秒，可用 `--boot-seconds` 指定 0–300 秒之间的正数；失去 running 状态时保留实际 lastError 和时间/帧数。合并串口保存在 `serial.log`，单独 UART0 保存在 `serial-uart0.log`，避免把 USB/UART 的重复日志误认成重启。上面的收据来自 Node 中的真实生产 Worker；浏览器验收由整合记录单独补充。原始固件未提交或镜像到服务器。
