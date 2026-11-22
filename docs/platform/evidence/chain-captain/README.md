# Chain Captain C160 · 官方 UserDemo V0.2 真实 ROM 启动验收

结论：未经改动的官方合并镜像 `C160-M5Chain-Captain-UserDemo-V0.2_0x0.bin`（2 571 488 B，SHA-256 `fe8404ad…c67b0`）在本仓库的 `chain-captain` 板型上、使用页面 worker 的实际 WASM JIT 后端完成 ROM 启动，LVGL Launcher 与六个应用菜单出现，240×240 面板首帧非空（63 种颜色，SHA-256 `c1302df064cb7aca…b681`）。三次按键输入均由宿主 GPIO 或板级按键通道送达 guest 并产生可观察后果。两次运行均 `status: pass`，明细见 `runtime-v02.json`。

复现命令（产物写入被 gitignore 的 `output/`，不提交）：

```sh
node tools/probe-chain-captain-factory.mjs --seconds 5 --dump-serial
```

脚本接受调用方给定的固件 URL 与 SHA-256，默认取目录中的官方值；worker 与 WASM 即线上使用的 `web/wasm/worker.js`、`web/wasm/esp32sim.wasm`（SHA-256 `18a414809281dac3…d40b`，5 938 483 B）。

## 建模外设确实被 guest 命中

串口日志逐行确认：M5PM1 在 I2C `0x6E` @100 kHz 读到 `ID=0x50 MODEL=0x20 HW=0x05 SW=0x06` 并回报 Charge Enabled / Boost Disabled；`HAL-FS` 把 `storage` 分区挂载为 FAT（`/storage`，2 477 568 B 可用，首次运行由模拟器格式化）；BMI270、RGB、IR、显示、ES8311 + 22 050 Hz mono I2S 依次初始化；随后 `[Launcher] on create` 与应用列表 Chain IoT / Pomodoro Timer / IMU Bubble Level / IR Copy / Recorder / Settings 出现。

按键证据：G4（KeyB）打开 `Chain IoT` 应用并使 M5PM1 `Boost Enabled`，串口增量 3 620 B、面板帧变化；G5（KeyC）触发 `HAL-CHAIN` 两路扫描，各回报 `index=255 cmd=0xFE received=0 bytes`；G1（KeyA）串口无新增但面板帧变化。电源键走 `transport: "board"`，`runtime.input({type:'button',id:'power',pressed:true})` 生成 `{t:'board-button',id:'power',pressed:1}` 并由 `esp-soc/src/machine/web.rs` 派发到 `bus.board().button_at()`。

## 官网 PinMap 与出厂固件的运行期冲突（保留冲突）

官网 Chain Captain PinMap：`PORT.A` = G6 (UART2_TX) / G7 (UART2_RX)，`PORT.CHAIN` = G18 (UART1_TX) / G17 (UART1_RX)。固件运行时打印：

```
HAL-CHAIN: left/Port A UART ready (TX=7 RX=6)
HAL-CHAIN: right/Port B UART ready (TX=18 RX=17)
```

Port B 与官网一致；Port A 的收发方向与官网相反。目录按官网登记引脚角色，并把这条运行期日志作为固件侧证据保留在产品知识记录中，不擅自改写任一侧。

## 已知偏差与未证实项

- 镜像头声明 `SPI Flash Size : 8MB`，而 guest 的 `spi_flash` 报“实际 16 384 k 大于镜像头声明的 8 192 k，按头声明使用”。目录按厂家规格记 16 MB，验收运行使用的是头声明的 8 MB 视图。
- `W MSPI Timing: tuning fail, best point is fallen back to index 5` 是本平台 PSRAM 时序调优伪影，不作为实物结论；PSRAM 8 MB 与 80 MHz 由 guest 日志确认。
- 未建模外设以 guest 自身错误暴露：`HAL-RTC: RX8130 flag/time read failed: ESP_ERR_INVALID_STATE`（不是 NACK）。
- Wi-Fi 只走到 guest 初始化：驱动与固件版本打印、`HAL-WIFI initialized`，但 `NVS namespace wifi doesn't exist` → station 未启动，Chain IoT 显示 “Wi-Fi unavailable; Chain controls remain offline”。没有无线链路或 Chain 从设备应答的证据。
- 探针的按键串口增量取样窗口重叠，摘录里同一行出现两次；完整捕获（未提交）中每行一次。
- `hardwareDiagnostics` 为 null：`web/wasm/worker.js` 与 `web/platform/runtime.js` 目前只对 `stopwatch`/`corematrix` 开放该通道，`esp32s3/src/board/chain_captain.rs` 已实现的 `hardware_diagnostics()` 尚未接线。
- 未连接真实设备，未刷机，无实体对照；JD9853A 的 GRAM 行数、Grove 端口数量、USB-C 上的 GPIO 编号仍属未知，见产品知识记录的 unknowns。

## 发布渠道状态

- GitHub release V0.2（发布于 2026-10-12）是唯一可直取校验值的官方镜像；其资产响应没有 `Access-Control-Allow-Origin`，浏览器直载被 CORS 拒绝，只能本地导入。
- M5Burner 已登记 `CHAIN_CAPTAIN`（deviceId 2122580583806615554，ESP32-S3）与两个 OFFICIAL 项目：`Chain Captain UserDemo` V0.2（firmwareId 2123365058572402690，versionId 2123365058576596993，`binFileName` 为 `C160-Chain-Captain-UserDemo-Di-V0.2-20260928_0x0.bin`，2026-11-19T18:37:37+08:00 发布）与 `UIFlow2.0 Chain Captain`。本轮 `versions` 与 `firmwares` 两个 JSON 端点取到内容（`sources.json` 记录哈希），但 `/versions/<versionId>/download` 返回 400（同 headers 早前返回 401），按 `binFileName` 猜测的 CDN 路径返回 404，因此未取得可直载的 Burner 文件名与校验值；Burner 的文件名与 GitHub 资产名不同，也不声称二者为同一构建。
