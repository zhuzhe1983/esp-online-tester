# C5 IDF 5.5.3 浏览器运行证据

本目录整理已完结的浏览器运行记录。本次只读取既有收据与原始 PNG，离线补算像素数；没有新启动 browser、guest 或 build。结果分列于 [result.json](result.json)。原始截图、UART、磁盘镜像和生成固件仍为私有保留材料，当前没有可核验的远程原件下载链接。

厂家原样 `03_sd_card` 完成 guest FAT 挂载、71 B 写入/读回比较；flush 后普通同源 reload，在同一匿名 context 内只 load 到 ready、没有 start 时，64 MiB 导盘与运行后盘逐字节相同。其 SHA-256 为 `96b5f64b1cda080bd1047055731ef945e52652808b478a134a29661550b3565a`，FAT32 中 test.txt 为 71 B / `b0944679cf35c8ac6566d654a769567cc9292e0812cdf471e4cae1f2cd45a91c`。

厂家原样 `02_lvgl_demo` 产生真实 320×320 帧，内容为 performance monitor 未启用提示。它没有取得 benchmark 或原例三键 UI 资格。独立项目 `c5_lcd273_diagnostic` / 0.1.0（7009 merged）取得三键六边沿、LCD 活跃后再次读相同 TF 文件、host reset 和普通 reload 的结果；它不是厂家示例或 FactoryOnly。三个 merged、WASM、ROM 的完整 hash 与字节数均在 JSON，全部采用 ROM 启动、appDirect=false、configured stubs=[]。

## 六个边沿的已记录结果

每个电平先注册新的 guest UART 条件，得到 DOWN 后才发送 UP；没有用固定墙钟时长充当 guest 按压。UART 只保留解析字段，不复制原日志。下表计数为该键 DOWN/UP；每张最终帧也已在原运行中目视核对最近事件。像素差异是相邻已接受 RGBA 的不同像素数，不单独证明事件语义。

| UART seq | 键/边沿 | 可见计数 | frameNumber | 与前一接受帧不同的像素 |
| ---: | --- | --- | ---: | ---: |
| 1 | PREV DOWN | 1/0 | 19 | 7649 |
| 2 | PREV UP | 1/1 | 77 | 8777 |
| 3 | NEXT DOWN | 1/0 | 175 | 1937 |
| 4 | NEXT UP | 1/1 | 213 | 8664 |
| 5 | BOOT DOWN | 1/0 | 215 | 2107 |
| 6 | BOOT UP | 1/1 | 222 | 17741 |

每帧为 102,400 pixels / 409,600 RGBA bytes；JSON 另保留非黑像素数、颜色数、PNG/RGBA hash、入队响应及原件 hash。PREV UP 的 queue ACK 因后续抓帧 guard 超时未落盘，明确为 null；对应的新 UART 响应与最终实际帧都在。其他 ACK 也只表示入队，消费证据来自 guest UART。

BOOT UP 触发第二次新 TF 读取，prefix 71 B、EOF、guest CRC32 `31b7aae7` 均匹配；host CRC32 一致。诊断输入后的 backing writes=0，导盘与导入盘逐字节相同。输入完成后的 host reset、普通同源 reload 后真实启动，三键都回到 0/0、文件仍匹配；两张 RGBA 均精确等于初始帧 `32af322b20a2170256e26138dbd975acee0a48a8a232ce1e23ad55386d8f0b47`。reload 的 ready 阶段再次导盘相同，未第二次导入卡。

保留范围是 product + board + canonical full-Flash hash namespace，且为每次检查的**同 origin、同 owned ephemeral context、显式 flush 后正常 reload**。第二个匿名 context 是显式导入只读 clone，不是跨 profile 持久化。没有 crash-safe/power-loss、每 byte MISO、物理 panel/RF 或 DOM/3D 按钮命中结论。两次匿名 context 的浏览器和服务器均已关闭，无 owned process/listener 剩余；不列出进程或会话标识。

## 来源与负结果

首 source revision 为 `99d166829e5835d274a8396bbf2f294b4baa0afd`，末为 `a17cc4736cba1be584eecc893d840a8d868d283f`。已完结核验对比了这两版实际服务的 runtime/app/worker/hardware-diagnostics/sd-card-ui 五个文件，字节完全相同，逐文件 SHA 在 JSON。浏览器前后实际 fetch 的 WASM 与 ROM hash 也一致。这些是原运行的来源记录；本次 curation 未重新运行它们，不继承后续显式 WiFi 兼容 profile 的资格。

所有负例仍在 JSON：不存在的 UART 等待词导致 WAIT_TIMEOUT；ready stats 前置检查误用 null 导致 helper 缺失；早抓帧仍显示旧状态；静态 UP 后要求四帧的观察 guard 超时；NEXT 一次局部发布只有新计数而 recent 仍旧。相应原件 hash 保留，最终结果采用后续实际发布并目视核对的帧，没有改固件或 host 生成设备像素。

JSON 只列复现或质疑结论所需的原件身份，其他条目通过完整私有 **155 项 manifest** 的 canonical SHA 及原始完整收据 SHA 追溯。canonical manifest 的序列化规则在 provenance 中给出。原截图、UART、绝对路径、进程/浏览器会话细节、服务器地址以及无关库存均未复制。删减限制是：远程读者目前只能核对身份与结构化结果，独立复查原像素/日志需要取得尚未发布的原件。历史 hash 没有被编辑后文件的 hash 静默替换。

复核这些记录时，需提供 hash 相同的输入，使用项目 Playwright wrapper、新建无持久化 headless context 和调用者提供的静态 origin，按 JSON 的 case/edge/flush/reset/reload 顺序执行。像素计数可从相同 PNG 解码到未调色的 RGBA8 后重算；按每像素四字节不等计差，前一帧依次为记录的初始基线及 seq1…6。本目录仅含两份文本摘要，原件未公开发布。

## 原始与整理后身份

| 记录 | 字节 | SHA-256 |
| --- | ---: | --- |
| 私有原始完整收据 | 50575 | `1c12957f6f7892e7d62df27bed2f59361c130330a996684e82481dc726ea7da1` |
| 整理前 result.json | 34784 | `8429ade0eca009ccd0881dfe19c9c12a6d7afe280364372ad2f84cdb9a8a1c2a` |
