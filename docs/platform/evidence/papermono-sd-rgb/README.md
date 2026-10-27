# PaperMono TF 卡与 RGB 数字输出

验证来源为 M5Stack UserDemo 1.2.0 完整镜像（SHA-256 `72c290bc2ffa216041b276660277369bd17ecef92315e367d98bb2c96c8428fa`），没有修改固件或向 guest 注入文件系统结果。镜像仅从远程读入进程内存。原有显示、触摸和按键控制六个检查点另行回归通过。

[interaction.json](interaction.json) 保留实际 guest 输入回执、帧号/hash、SD 状态与 RGB 输出。其 `wasmSha256` 是该次实验使用的构建，不冒充后来测试构建。图片直接编码真实 guest framebuffer：

- [启动](boot.png)：TF CARD 64.0MB / FREE 62.0MB；实际 ESP-IDF SDMMC 命令、IDMAC 描述符和 FAT32 读盘后显示容量。
- [红关](rgb-red-off.png)、[绿开](rgb-green-on.png)、[蓝开](rgb-blue-on.png)：触摸官方 RGB 开关，板级输出依次为 `[0,0,0]`、`[0,255,0]`、`[0,255,255]`，初始为 `[255,0,0]`。红由 PM1 LED_EN 控制，绿/蓝由 IOE PWM2/PWM1 控制；不是前端替代绘图。
- [目录](sd-directory.png)：官方 TF 浏览器读出测试工具生成的 `HELLO.TXT`；文件内容只有公开的 `ESP SD read test`。没有提交或导出用户 SD 数据。
- [重建后](sd-after-reset.png)：重新创建 worker/模拟器后恢复同一个导入媒体，卡重新挂载。原生 Rust 的 IDMAC 写→芯片重启→读回测试另证实实际写入块保持不变。

复现（全程远程固件进程内存，输入镜像在测试中生成）：

```sh
ESP32SIM_PROBE_CLOCK_TIMEOUT_MS=60000 node tools/probe-official-papermono.mjs \
  https://m5burner-cdn.m5stack.com/firmware/376080df0d31b0026e9a67a75d02d571.bin \
  72c290bc2ffa216041b276660277369bd17ecef92315e367d98bb2c96c8428fa \
  output/papermono-sd-rgb 15 sd-rgb
```

[negative-results.json](negative-results.json) 保留 M5Burner CrossPoint 1.5.0 完整镜像在 WASM JIT 和解释器两条路径的启动失败：PC `0x4203b032` 的异常后重启，发生在能证明 SD 挂载/帧输出之前。因此不能把它归因于 FAT32、宣称 CrossPoint 可读书或通过该失败认定镜像损坏。原始串口和 receipt 的 hash 已保留，删除不相关的设备标识与存储 key。

[source receipts](sources.json) 固定 ESP-IDF 5.5.1（与官方 UserDemo 启动串口一致）SDMMC 寄存器、IRQ30、GPIO 矩阵与 SYSTEM 时钟/复位，M5IOE1 PWM 寄存器，以及 PaperMono 适配版 CrossPoint/FreeInk SDK 来源。上游 open PR 检查时为 #290、#278、#263、#262；#263 修改通用 Flash 持久化、没有 SD 主机或 Board 插槽接口，本次不复制其 Flash 功能。

没有进行实机验证。数字 SD 事务同步完成，不模拟真实读卡速度、磨损或信号完整性；RGB 输出是 PWM 占空比映射，不是实测发光强度或 PWM 光学波形。默认卡仅 64 MiB，是方便浏览器资源和自动化测试的虚拟容量，不代表真实 SDHC 最小标称容量。

浏览器存储边界另有结构回归：导入基图和脏页分开保存；清空后的旧 epoch 拒绝写回；跨页面清空发生在读取或 SHA-256 校验期间，也阻止旧卡挂载；dispose 不允许挂起的导入/清空操作重建 worker。IndexedDB 延迟打开后关闭连接，不创建遗留广播通道。`tools/platform-sd-card-test.mjs` 的生命周期用例使用人工头信息与回执 worker，不执行固件。
