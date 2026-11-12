# 普通 INK4 TF 与录制验收

2026-11-12，普通 `metalio-ink4`；官方 2.0.53 地址、SHA-256、ROM/WASM hash、负面基线和检查结果见 [receipt.json](receipt.json)。本轮只开放已有板级 SDMMC 的网页存储能力，未改 CPU、SDMMC、显示器或物理引脚；INK4 Plus/S31 不开放此能力。

官方固定源码的 `SdCardManager::MakeSlotConfig` 使用 CLK38/CMD40/D0=39、1-bit，D1–D3 不参与传输，也未配置机械卡检测。GPIO46 是 DAT3/CD 上拉，避免卡误进 SPI；不能以“插卡”为由拉低。TCA9555 的 MAIN_PWR/P0.6 与 SCREEN_SOCKET_PWR/P0.5 均开启才接通卡座。来源收据见 [sources.json](sources.json)。

未开放时，实际官方固件在 0.847 客体秒尝试挂载，0.867 秒以 `ESP_ERR_TIMEOUT` 失败。开放后，同一固件在 0.877 秒挂载 64 MiB FAT32，列出预置 `PROBE.TXT`，并创建 `/metalio/e-ink` 及 recordings、books、fonts、a2ui_cache、chat_log、wallpaper 子目录。导出卡有 5 个修改页；reset 与重新加载后，预置文件 hash 和目录均保留。这里验证的是文件读取与目录元数据写入，不将它扩写为任意应用文件写入、USB MSC 或实机兼容。

首次探针错误地按较新源码期待上传 README。2.0.53 实际未生成此文件；保留失败收据的 hash，并将断言改为该固定二进制可观察的目录行为。没有通过替换固件、伪造文件或调整内存容量消除失败。

Headless Chromium 从工作台“下载并尝试运行”入口运行官方镜像。`backend=indexeddb`，首次写入后整页重载；相同固件恢复 `restored=true`，完整卡 hash 保持 `e71fe561b99a25f35f4f9e378ba2548542c2af21a2139cebc236ed7e430c6cde`，随后官方再次挂载成功。工作台与 mini 均提供导入、导出、清空三控件；未执行清空。任务浏览器已关闭并确认退出。公共 relay 的 `/session` 请求被 CORS 拒绝，联网不计为通过。

录制从另一干净进程的 ready 状态开始，以实际 worker 队列时钟等待开机；包含触摸设置、Home 按下/释放、音量增加，共 6 个事件。另一干净进程回放，固件分别输出设置、返回主页与音量日志，最终帧 SHA-256 相同。录制不包含 Flash/TF 私人内容；没有为回放清除已写存储。时刻并非周期精确，最大延迟保留在收据中。

## 复现

准备本仓库 WASM 与官方 S3 ROM。使用目录中的固定官方地址/hash，调用者显式提供远程目标：

```sh
node tools/probe-ink4-sd.mjs "$INK4_URL" "$INK4_SHA256" output/ink4-sd 8
node tools/probe-official-ink4.mjs "$INK4_URL" "$INK4_SHA256" output/ink4-record 8 record
node tools/probe-official-ink4.mjs "$INK4_URL" "$INK4_SHA256" output/ink4-replay 8 replay output/ink4-record/recording.json
node --test tools/platform-sd-card-test.mjs tools/platform-device-storage-test.mjs
node --test tools/platform-config-test.mjs tools/platform-runtime-test.mjs tools/platform-input-test.mjs tools/platform-view-test.mjs
node tools/platform-recording-test.mjs
```

Node 探针使用独立测试进程的内存存储；浏览器 IndexedDB 结论来自上述真实页面测试。原始输出只保留在 ignored `output/`，不提交串口、设备标识、会话信息或完整截图。当前设置页顶部标题叠字仍能复现；光学灰阶、残影与触摸扫描节奏也未校准。
