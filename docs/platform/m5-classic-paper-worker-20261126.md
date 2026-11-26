# Paper / CoreInk 公开运行链输入与像素验证

基线 `7e9c140e`。只新增原创诊断程序、集成测试和本报告/回执，没有修改 Rust 或 runtime 行为，也没有构建新 WASM。测试读取集成后的生产 `esp32sim.wasm` 与固定 ESP32 rev300 mask-ROM，通过公开 `createPlatformRuntime`、真正的 Worker 模块及其生产 WASM 后端执行。

| 产品 | 公开输入 → 真实客体 GPIO | 原创客体产生的像素变化 |
| --- | --- | --- |
| Paper v1.1 K049-B / `m5paper-v11` | `rotary-press` 按下/释放 → G38低/高 | 540×960竖屏，初始全白；按下后(0,0)…(0,3)四枚像素变黑，其余仍白；释放后全白 |
| CoreInk K048当前D67 / `coreink-d67` | `button-user` 按下/释放 → G5低/高 | 200×200，初始全白；按下后(0,0)…(7,0)八枚像素变黑，其余仍白；释放后全白 |

程序用 LX6 指令读取 GPIO 输入，再通过真实 SPI MMIO 发送 IT8951 或 SSD1681 窗口、RAM与刷新命令，轮询 BUSY 完成后输出串口标记。宿主只调用公开的产品按键输入；没有直接写 framebuffer 或跳过 Worker 注入引脚。每次状态变化都核对完整 RGBA 帧，回执保留变化坐标及原始像素缓冲 SHA-256。未声明的按键 ID 通过公开 API 被 `UNSUPPORTED_CONTROL` 拒绝。

Classic ESP32 的 `SocBus::boot_app` 当前明确要求从 ROM 启动。因此使用正常 ROM 启动：原创 SRAM 诊断镜像置于 bootloader 位置0x1000，配固定 mask-ROM、原创分区表与声明的同源诊断 app。mask-ROM实际读取镜像、加载 SRAM 后跳到0x40081000，原始串口日志包含 ROM 加载行与程序的 `BUTTON UP/DOWN`、`FRAME COMPLETE`。这不是完整 SDK 启动验证，也没有把原创诊断程序当作厂家固件。`appDirect:false`、空 stubs 与此路径一起断言。

复现：

```sh
ESP32SIM_WASM=/path/to/production/esp32sim.wasm \
ESP32SIM_ROM_DIR=/path/to/roms \
M5_CLASSIC_PAPER_WORKER_RECEIPT=/tmp/paper-coreink-worker-receipt.json \
node --test tools/platform-m5-classic-paper-worker-test.mjs
```

ROM目录需要 `esp32_rev300_rom.elf`。测试动态生成诊断镜像，不提交或镜像第三方固件/ROM二进制。[回执](evidence/m5-classic-paper-worker-20261126/receipt.json)记录实际生产 WASM、ROM、诊断源码、测试源码及每个生成镜像的 SHA-256、Node版本、串口和全帧摘要。程序源码是 [m5-classic-paper-probe.mjs](../../tools/fixtures/m5-classic-paper-probe.mjs)，可离线重新生成。

这补足此前板级 LX6 单元测试与101个SKU构造检查之外的公开JS输入路由证据。四向旋转、错误引脚等原有板级用例继续作为独立协议覆盖。本集成测试不宣称浏览器 GPU/3D点击区域、RTC/无线/触摸/SD/音频/模拟波形、完整厂家固件或实机验证通过；CoreInk仍限D67批次。产品与固件目录的 `partial` 和 `simulationVerified:false` 均未改变。
