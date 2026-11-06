# CoreMatrix C158 运行与固件验证

CoreMatrix 使用独立 ESP32-C61 运行器，160 MHz、16 MiB Flash、8 MiB PSRAM，芯片 ID 为 20、模拟修订号为 1.0。加载原厂 C61 ROM、原始 bootloader、分区表和应用，不使用 C6 别名、app-direct、启动桩或替换后的镜像。模型与外观的来源、尺寸及限制见[模型报告](models/m5stack-c158.md)。

## 官方固件与启动限制

默认条目是 M5Stack GitHub 发布的 UIFlow2 v2.5.4，文件 `uiflow-e437118-esp32c61-16mb-corematrix-v2.5.4-20261030.bin`，SHA-256 为 `8d5b8d4e22ebc293937086b2f9f218aa5a5ebda27b5ccb32b3f8f580ee02939b`。它不是已确认的出厂恢复固件。版本、M5Burner 型号映射和未找到独立出厂/小智镜像的调查记录保留在固件目录及 watchlist。

原版 GitHub 2.5.2、2.5.3、2.5.4 均能启动 MicroPython，但其启动脚本导入不存在的 `M5Things`，随后停在 REPL。镜像内文件系统和冻结模块清单均未找到该模块；不通过空模块或修改 `boot.py` 隐藏错误。因此条目保持 `trial`、`simulationVerified: false`。M5Burner 2.5.4 是另一构建，下载需要官方验证码，尚未取得其镜像，不能继承 GitHub 版本的结论。

2.5.4 还有 [TLS 证书日期的上游缺陷](https://github.com/m5stack/uiflow-micropython/issues/120)：校验时间多加了 946684800 秒。测试先复现正确 RTC 日期被拒绝，再在客体测试程序中显式调整日期，保留 `CERT_REQUIRED` 和主机名校验。模拟器没有更改 RTC 或跳过证书检查；该 workaround 不能解释成原版应用无需处理即可联网。

GitHub Release 下载受浏览器 CORS 限制。可导入官方原始 BIN，或由部署方配置[固定目录来源的流式中继](firmware-relay.md)。测试使用任务专属本地中继，从原始 Release 地址加载并验证 SHA；第三方镜像未提交到仓库或本站。公开站点未配置中继时不保证该地址可直接下载。

## 已验证的客体行为

验证程序经 USB raw REPL 上传到原版 MicroPython 的 RAM 中，不替换固件或安装为开机脚本。LED、音频和传感器结果来自实际客体 I²C/SPI/寄存器操作。页面工作台、独立播放器和生产 Worker 使用同一运行器。

| 功能 | 验证内容 |
| --- | --- |
| TM1680 白色矩阵 | 256 点全亮；跨四列/半区边界的 8 个不对称像素；亮度调节、关闭/恢复；错误地址 NACK；IOE1 断电清除输出，恢复后须重新写帧 |
| 3D / 2D | 256 个独立 LED 对应 `y × 16 + x`；16×16 framebuffer；小尺寸矩阵放大显示；工作台、独立播放器、iframe 与父页面 RPC 得到相同客体输出 |
| 三个按键 | PM1 A/B/C 独立按下、释放后由 `M5.update()` / `isPressed()` 读取 |
| BMI270 | 三种虚拟姿态改变真实传感器寄存器；M5Unified 的 `[rawY, -rawX, rawZ]` 转换核对；ODR、数据就绪和运动中断由客体时钟驱动 |
| TF 卡 | GPIO25/27/26/28 实际 SPI + C61 GDMA v2 双向传输；FAT32 挂载、写入、读回；平台 reset、固件重载，以及真实浏览器页面刷新后从 IndexedDB 恢复文件 |
| 蜂鸣器 | IOE1 PWM channel 2 / IO11；48 kHz PCM 从客体频率和占空比产生；供电关闭后静音；原库 `M5.Speaker.tone()` 可输出 |
| 电源 | PM1 电压读回、IOE1 各支路供电；电源键双击断电时 CPU 停止执行、单击重新 ROM 启动；保留 L1 时 BMI270 INT1 经 PM1 GPIO4 唤醒 |
| Grove | G0/G1 实际 I²C 路由；ENV-IV 的 CRC/补偿、DLight 的原始照度、被动 Hub 共线；错误引脚/地址、IO5 断电及恢复；原生测试另覆盖拔插和复位 |
| 内部 Flash / RTC | 文件写入、flush/rename/sync 及硬复位后逐字节读回；RTC 日期设置、计时与复位保持；见[内部系统回执](evidence/corematrix-system-20261106/receipt.json) |
| Wi-Fi STA / SoftAP | 原固件 open/WPA2 扫描、关联、DHCP、DNS、NTP、HTTP 上传/下载和重连；错误密码拒绝；SoftAP 双模式 ARP/UDP；生产 Worker 经浏览器本地网络栈完成 DHCP、TCP 和 4096 字节 HTTP 响应比对 |
| TLS / 密码运算 | TLS1.2 ECDHE-ECDSA-AES128-GCM-SHA256，4096 字节响应比对与错误主机名拒绝；109 组客体 SHA/MD5/HMAC/P_SHA256/AES 答案核对；证书日期 workaround 见上文及[网络回执](../evidence/c61-wifi/runtime.json) |
| C61 定时睡眠 | `lightsleep(200)` 返回约199 ms；睡眠期间 CPU、寄存器与数字计时器保持；`deepsleep(200)` 经原厂 ROM 重启并读回深睡复位原因；原生和生产 Worker 均执行通过 |
| BLE 广播 / 被动扫描 | 原版控制器经真实描述符、DMA 与 IRQ 收发；37/38/39 三信道广播字节与100–110 ms间隔；扫描回调的地址、RSSI、广播内容一致；移除附近设备后无扫描结果；原生、生产 Worker 和真实浏览器均通过 |

`observe().hardwareDiagnostics` 显示主电源、矩阵供电/使能/PWM、蜂鸣器频率/占空比/供电、Grove 5V 和 TF 在位。这是客体寄存器的观测值，不能替代固件实际输出验收。

默认 `A/B/C/P` 对应三个按键和电源键。电源键由 PM1 处理，不能用普通 GPIO 高低代替。IMU 可通过 `setImu()` 注入明确的虚拟轴；模型的物理安装轴尚未实物校准，因此没有开启拖动模型即改变传感器的隐式映射。

生产 Worker 的14项原版固件检查见 [WASM 回执](evidence/corematrix-runtime/wasm.json)；工作台、独立播放器、iframe、父页面接口、BLE 和页面刷新后的 TF 保存见[浏览器回执](evidence/corematrix-runtime/browser.json)。BLE 的 `configureBle({nearby:[...]})` / `readBle()` 使用同一运行内核，具体参数与限制见[接口说明](../../skills/esp-online-tester/references/api.md#corematrix-legacy-ble)。没有通过主机直接插入扫描回调或修改点阵像素。

CI 对应命令在本地通过：1,881 项 Rust 测试、354 项平台测试、两组禁止警告的 Clippy、13 个生产 WASM 固件场景及 111,126 个 WASM 差分用例。命令、工具版本、原始日志哈希及已修正的旧测试假设见[检查回执](evidence/corematrix-runtime/checks.json)；共享代码的现有芯片性能对照见[EX251](../evidence/corematrix-cpu/README.md)。

## 复现

```sh
tools/fetch-rom-elfs.sh roms
tools/wasm-build.sh
node tools/probe-corematrix-uiflow.mjs /path/to/original-corematrix.bin \
  8d5b8d4e22ebc293937086b2f9f218aa5a5ebda27b5ccb32b3f8f580ee02939b \
  --receipt /tmp/corematrix-runtime.json
```

程序使用独立虚拟存储并检查原始镜像 hash。`--checks matrix,buttons,imu,sd,buzzer,power,grove,network,ble,sleep` 可缩小检查范围。SD 分支会验证 reset 与重新加载后的文件读回；没有验证突然终止宿主进程时的数据完整性。`network` 使用客体 RAM 中的单次 HTTP 服务与浏览器本地网络栈，不连接外部服务器。

浏览器检查使用 `tools/fixtures/corematrix-browser-check.js`，通过 Playwright wrapper 在 headless 工作台或分享后的 embed 页运行，并通过 `?test-relay=` 指定自有流式中继地址。原厂启动错误会单独记录，不能把 RAM 测试程序成功解释为 UIFlow 启动应用完整兼容。

## 边界

没有实机刷写、RF、模拟电压/耗电、声音响度或光学校准。蜂鸣器是数字 PWM 波形，音频原始时基不变。PM1 外部断电/唤醒与 ESP32-C61 内部 light/deep sleep 是不同机制；SoC 目前实现定时唤醒子集，GPIO/USB/UART/BLE 唤醒及详细掉电保持未覆盖。尚不支持 BLE 连接、GATT 和配网，不能由同系列 C6/C3 的结果代替。

M5PM1/IOE1/BMI270 使用有来源的寄存器子集。FIFO、计步等未覆盖的 BMI 功能、任意第三方固件和全部外部 M-Bus 模块不属于当前兼容承诺。内部 PCB 占位、连接器隐藏深度、贴纸光照及 LED 扩散仍有估算，模型不声明各视角 100% 实物等价。
