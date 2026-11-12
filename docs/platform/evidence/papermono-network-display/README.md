# PaperMono / Lite 显示与联网验收

本页只记录固定官方 UserDemo 1.2.0（SHA-256 `72c290bc2ffa216041b276660277369bd17ecef92315e367d98bb2c96c8428fa`）的实际运行。BMP 和 FAT32 媒体由探针生成，固件从 SDMMC 读取文件并通过 M5GFX 写 SSD1677；主机不替换屏幕内容。联网只使用明确配置的任务自有助手与公开测试路由器，不读取用户网络凭据。

## 四灰阶和方向

探针生成 120×200、24-bit 未压缩 BMP，由官方 `AppTfCard::drawImageViewer` 调用 `drawBmpFile` 放大 4 倍到 480×800。四根竖条使用源值 0、96、160、255；左上白角标和右下黑角标区分方向。固定 M5GFX 的 Bayer 值范围为 -30…30，`(value + Bayer) >> 6` 再限制到 0…3，因此这四个源值分别稳定选择四个数字等级。它们不是实测反射率。

| 输入灰度 | 模型数字等级 | RGB565 后网页 RGBA | 预计像素数 |
| --- | --- | --- | --- |
| 0 | 0 | 0, 0, 0, 255 | 96,000 |
| 96 | 85 | 82, 85, 82, 255 | 96,000 |
| 160 | 170 | 173, 170, 173, 255 | 96,000 |
| 255 | 255 | 255, 255, 255, 255 | 96,000 |

WASM `d72a0a9468263b7c9538d6536c47518c2be90cf62bcdb9cef98e495b9ab57595`（包含 SSD1677 Mode 2 RAM 交换修复 `54de626b`）在完整版和 Lite 上均通过全部 384,000 像素比较；两型号图像 hash 均为 `907e0a951e4e921497a796802572ee6772a4a77f7c184c2b9b657faa5833befb`。实际截图也已检查方向和四个条带。[逐像素回执](four-gray.json)记录输入 BMP/卡镜像 hash、所有区域、计数和不一致数。

## 局部刷新与恢复

官方图片工具栏绘制逻辑顶部 52 行和底部 68 行。M5GFX `_update_transferred_rect` 在旋转后的原生 X 方向按 8 像素对齐，使肖像画面实际覆盖顶部 0…55 行及底部 728…799 行。`epd_fastest` 使用 `new_msb` 选择黑白；因此逻辑范围之外的 52…55、728…731 行也会把原有中灰阈值化。

两型号的实际结果均与源码一致：中部 56…727 行的 322,560 像素全部保持；这 8 条边缘行共检查 3,840 像素，其中原有两种中灰的 1,920 像素变成对应黑白，其余本来黑白的像素不变。第一次按逻辑范围要求中灰不变的断言失败已保留为测试预期修正，不能据此改模型去掩盖固件真正写入的刷新范围。

再次触摸隐藏工具栏时，官方程序从自己的图像缓存恢复并执行 quality 刷新。两型号的全部 384,000 像素和整幅 hash 均恢复到初始图；不是仅检查帧号增加或任意像素变化。

最终构建 `8d8ccce6…4fb20`（源码 `2891d72c`，加入 GPIO I²C STOP 修复）再次通过完整版的所有逐像素、局刷和恢复断言，结果与上面的 `d72` 相同。两型号的 A/B 亮度按钮、触摸亮度加键、静音及恢复也全部通过；[最终回归回执](final-regression.json)保留数值和输入。两型号完整四灰阶验收使用 `d72`，最终 `8d` 另验完整版，构建身份没有混记。

## Wi-Fi 关联、DHCP 与真实 SNTP

最终 WASM `8d8ccce696666eeb7a355cdbe15fd398eaec9ddb9ae9ade85544d91639b4fb20`，构建源码 `2891d72c`，在完整版与 Lite 上均完成实际联网。探针通过原有 FT6336 触摸打开固件 NTP 界面、扫描路由器、用固件屏幕键盘输入公开测试 SSID `eink`（空密码），关闭键盘后点击同步；没有直接写 NVS 或向客体填入连接成功状态。

固件日志出现 `init → auth → assoc → run` 与对应 SSID 的连接，助手取得虚拟 DHCP 地址 `10.0.2.15`。两次运行的最终 NAT 计数均为 2 个 UDP flow、出站 78 字节、入站 142 字节。原固件的 `configTime` 指向 `pool.ntp.org`、`time.nist.gov`、`time.google.com`；探针没有合成 SNTP 响应。实际 framebuffer 出现 `TIME UPDATED`，状态区域 hash 均为 `0cef249c…022928d`；完整版返回仪表盘后实际显示 2026.11.12 / 20:27。应用层成功由固件 `SNTP_SYNC_STATUS_COMPLETED` 条件及其画面确认，既不是仅看助手连接，也不是仅看 DHCP。

[完整版回执](network-full.json)和 [Lite 回执](network-lite.json)保留真实输入时间、计数、状态和截图 hash。原固件此流程不使用 HTTP，未声称 HTTP 服务兼容。SNTP 日期写入 RX8130 寄存器 RAM 不等于已实现 RTC 自主走时、掉电保持或唤醒。

联网复现沿用上面的固件参数，将场景改为 `network`，并通过环境变量传入调用者自己的私有助手 JSON 路径。两型号分别执行，文件格式为 `{"endpoint":"ws://localhost:<port>/network?token=<private>"}`；不要把真实文件放入 Git 或分享链接。

## 已知负面结果

[负面记录](negative-results.json)保留以下结果：

- 首次网络探针在选择 SSID 后没有先关闭固件自动弹出的密码编辑器，因此未触发连接；已按实际界面修正。
- 后续运行中启动仪表盘能看到虚拟路由器，但 NTP 对话框的一次扫描返回 `NETWORK SCAN FAILED`。助手连接不代表客体已联网；该次没有 DHCP 或 NAT 帧。不能把此结果概括为全部扫描均失败。
- 原尺寸 480×800、1.1 MiB BMP 能列入官方目录，但打开时主任务栈溢出、客体软件复位。此首次探针在运行期间遇到磁盘 WASM 替换，旧脚本只在退出时读 hash，故该记录明确标为 artifact 身份不确定，不能归因到某个构建。后续脚本改为对实际 fetch 返回的 WASM 字节计算 hash。小图放大通过不能证明原尺寸图已兼容；没有修改固件或断言实机也会溢出。

## 复现

```sh
ESP32SIM_PROBE_CLOCK_TIMEOUT_MS=60000 node tools/probe-official-papermono.mjs \
  https://m5burner-cdn.m5stack.com/firmware/376080df0d31b0026e9a67a75d02d571.bin \
  72c290bc2ffa216041b276660277369bd17ecef92315e367d98bb2c96c8428fa \
  output/papermono-gray-check 15 gray-display m5stack-papermono
# Lite 使用 m5stack-c153-lite；同样只启动一个任务自有 Worker。
```

联网探针通过调用者提供的 `ESP32SIM_NETWORK_ENDPOINT_FILE` 读取包含 `endpoint` 的私有 JSON，值只用于运行实例，不写回执。`network` 使用固件屏幕键盘输入公开测试 SSID `eink`、空密码，再执行连接与 SNTP，并同时断言关联、DHCP、双向 UDP 及固件的 `TIME UPDATED` 实际像素。`network-inspect` 保留为探索入口，只有 DHCP 断言。官方出厂程序没有此路径上的 HTTP 客户端或服务，不编造 HTTP 测试结果。

固定官方 `app_clock.cpp`、`app_wifi_scan.cpp`、`app_tf_card.cpp` 与 M5GFX 来源、抓取时间和 hash 见 [sources.json](sources.json)。所用 UserDemo 源码固定于 `e55877dc`，二进制自报版本为 `c78f6c5-dirty`；源码用于核对可观察行为，没有声称能逐字节重建该二进制。截图和原始 UART 保留在忽略的输出目录；Git 只留输入、数值、hash、失败及可复现步骤。原始回执 hash 保留于各记录，整理后的 JSON hash 见 [receipt-sha256.json](receipt-sha256.json)。所有 Worker 由 `runtime.dispose()` 在 `finally` 中关闭；共享助手由调用者管理。

灰阶是数字模型的四级输出，不模拟光学电压、反射率、温度、残影或实机刷新时长。电源、触摸和传感器的其他边界仍以 [PaperMono 运行范围](../../papermono-runtime.md)为准。
