# 纯静态 ESP 仿真网站可行性

核查日期：2026-10-26。参考硬件为 PaperMono C153、StopWatch C152、GPS Unit；GPS 的具体代次尚未确认。

## 结论

产品目录、配件目录、固件目录、DIY 编辑器、3D 交互、浏览器内固件执行、分享链接和自动化测试入口，都可以采用静态部署。服务器只提供 HTML、JS、WASM、JSON 和模型资产，CPU 仿真在访问者浏览器内运行。

需要区分三件事：WASM 是把仿真程序放进浏览器的运行形式；QEMU 或其他模拟器负责执行 ESP 指令和芯片寄存器；板级模型负责真实产品的屏幕、触摸、供电和外设。Three.js 只呈现外观、显示像素并收集操作，不会自动让固件获得设备驱动。

**建议先验证 esp32sim 的 StopWatch＋GPS，再补 PaperMono；不把 esp-emulator 设成唯一后端。** 这是一项基于代码和接口检查的选型建议，不是这两款设备的固件兼容性认证。

## esp-emulator 实际提供了什么

[官方仓库](https://github.com/espressif/esp-emulator)描述的是 Rust 模拟器，并非 QEMU 的浏览器包装。截至本次检查，最新发行版为 [v0.51.0](https://github.com/espressif/esp-emulator/releases/tag/v0.51.0)，发布于 2026-10-26 06:50 UTC。支持列表包含 ESP32-S3 和 C3/C5/C6/H2/P4/S31，不能据此推断支持经典 ESP32、S2 或全部 ESP 系列。

[官方网页](https://espressif.github.io/esp-emulator/)已经在 GitHub Pages 上运行。[浏览器指南](https://github.com/espressif/esp-emulator/blob/main/docs/guides/BROWSER.md)说明它使用静态文件、Web Worker 和 WASM；输入是 merged flash 镜像。

公开仓库 commit `61d71cef2b1f13a936173c6d282a52c093137d6a` 有文档、安装器、测试固件源码和辅助工具，没有模拟器的 Rust 源码和 Cargo 构建工程。[维护者在 #10 的回复](https://github.com/espressif/esp-emulator/issues/10)所在讨论明确表示，源码发布尚未决定。文档中的 `wasm-pack build` 不能理解成克隆当前公开仓库就能重新编译核心。仓库标有 Apache-2.0，但工程计划仍需以实际取得的源码和发行包附带声明为准。

### 本次实际检查

下载官方 `esp-emu-0.51.0-wasm.tar.gz`，核对 GitHub Release 的 SHA-256，再在 Node 中编译、实例化其 WASM，创建 `esp32s3` 对象并载入嵌入 ROM。检查成功；没有运行用户固件，没有进行浏览器或实机性能测试。

- 压缩发行包：2,369,090 字节；WASM：4,940,328 字节。
- 实际 memory buffer 为 `ArrayBuffer`，该包初始化不依赖共享内存。
- 可以看到装载固件、分批执行、读取寄存器/内存、GPIO 输入、GPIO 当前状态、串口输入和网络帧接口。
- `uart_input(data)` 没有 UART 端口参数，不能把原生 CLI 的 UART1 功能等同于浏览器能力。
- 没有公开 SPI/I²C 外设回调、指定 UART1/2 收发接口、ADC 设置、屏幕帧导出、PSRAM 容量设置或完整快照导出方法。
- `gpio_state()` 是当前状态读取，不是逐边沿且与总线事务有序的事件流，不能靠低频轮询可靠重建高速 SPI。

完整方法名和哈希见 [检查记录](evidence/esp-emulator-wasm-audit.json)。公开接口缺口与 [外设桥接请求 #5](https://github.com/espressif/esp-emulator/issues/5)、[WASM SPI/GPIO/ADC 请求 #38](https://github.com/espressif/esp-emulator/issues/38)一致。原生 `--i2c-dev` 是寄存器读写式临时替身，不能充当完整触摸、电源或传感器模型。

## 为什么优先评估 esp32sim

[`mikroverk/esp32sim`](https://github.com/mikroverk/esp32sim)提供 Rust 源码、MIT 许可、WASM 构建和 GitHub Pages 演示。核查 commit 为 `ddf753a5c400ff79d3aa6a88a007eccd044e838d`。

检查了 [StopWatch 源文件](https://github.com/mikroverk/esp32sim/blob/ddf753a5c400ff79d3aa6a88a007eccd044e838d/esp32s3/src/board/stopwatch.rs)和板型注册代码，确实存在 `stopwatch` / `m5stack-stopwatch`，不是只在 README 中列出的计划。其模型覆盖 CO5300、CST820、M5PM1、M5IOE1 及 A/B 按键。[板级接口](https://github.com/mikroverk/esp32sim/blob/ddf753a5c400ff79d3aa6a88a007eccd044e838d/docs/boards.md)包含 SPI、I²C、UART 路由、GPIO 边沿、显示和输入钩子，适合添加 GPS 与 PaperMono。

这些是 Rust 扩展点，不是已经存在的任意 JS 插件 API。第一版宜把审核过的模型编进 WASM，通过配置实例化；需要另行增加组合板型或桥接层。

现有 StopWatch 模型不是完整产品：TE 时序是合成的，PMIC 关机请求没有真正停止 CPU；板级文档中的整机启动报告依赖未公开的固件，不能作为我们的验收证据。RTC、IMU、音频、完整供电行为、GPS 和用户自己的固件仍需逐项验证。[模型说明](https://github.com/mikroverk/esp32sim/blob/ddf753a5c400ff79d3aa6a88a007eccd044e838d/docs/boards.md)

### 后端比较

| 后端 | 可利用的基础 | 关键缺口 | 建议用途 |
| --- | --- | --- | --- |
| esp32sim | 可修改源码、WASM、StopWatch 模型、板级扩展接口 | PaperMono/GPS 与动态组合需开发，仍须测实际固件 | 首选验证对象 |
| esp-emulator 0.51.0 | 官方发行、S3 CPU/ROM、WASM、串口/GPIO、原生联网 | 核心源码未公开，浏览器外设桥接不足 | 第二适配器、CPU/启动/串口对照 |
| Espressif QEMU | 原生调试、可修改模型、ESP-IDF 集成 | 浏览器移植和目标板模型要另做 | 后续特殊芯片或原生验证后端 |

[ESP-IDF 的 QEMU 文档](https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-guides/tools/qemu.html)描述的图形设备是虚拟 framebuffer，应用需要相应驱动，不能自动代替 PaperMono 的 SSD1677 或 StopWatch 的 CO5300。浏览器显示远程 QEMU 画面属于服务器方案；真正纯静态需要 CPU 仿真本身在 WASM 中执行。浏览器移植还需处理动态翻译、线程和系统调用，不能把原生 QEMU 二进制直接塞进网页。

## 首批设备及模型范围

| 对象 | 已核对的硬件 | 首期需要的行为 | 当前判断 |
| --- | --- | --- | --- |
| PaperMono C153 | S3R8，16 MB Flash、8 MB PSRAM；480×800 SSD1677；FT6336G；M5PM1、M5IOE1 | 显示命令/显存/BUSY、触摸、A/B 键、必要供电初始化 | 新增板型；不能沿用老 Paper 或 PaperS3 屏幕模型 |
| StopWatch C152 | S3R8，16 MB Flash、8 MB PSRAM；466×466 可视圆屏 CO5300 QSPI；CST820B | 复用并校验现有屏幕/触摸/按键模型，加 UART 配件路由 | 第一验证设备 |
| GPS Unit U032 | AT6558，UART 默认 9600 | NMEA 回放、无定位/有效定位、失联/恢复 | 单独产品修订版 |
| GPS Unit U032-V11 | AT6668，UART 默认 115200 8N1 | 同类场景，独立默认串口与协议配置 | 不静默当成 U032 |

来源：[PaperMono](https://docs.m5stack.com/en/core/PaperMono)、[StopWatch](https://docs.m5stack.com/en/core/StopWatch)、[GPS](https://docs.m5stack.com/en/unit/Unit_GPS)、[GPS v1.1](https://docs.m5stack.com/en/unit/Unit_GPS_v1.1)。

StopWatch 外接 HY2.0-4P 的信号为 GPIO10/11，电源为 5V，外部供电受 PMIC 控制。标注 PORT.A 不代表只能使用 I²C：是否可接 UART 要看引脚复用、供电、实际线序和固件的串口映射。GPS 数据应进入其 UART 接收端，不能送进调试控制台冒充成功。

PaperMono 官方公开 PinMap 没有列出一个可直接套用 StopWatch 接法的 Grove 口。本轮不指定 PaperMono＋GPS 的物理接线；先用 StopWatch 验证 GPS，PaperMono 先独立验证显示和操作，若需要 DIY 转接再核对原理图和实物。

StopWatch 官方可视尺寸为 466×466，而 esp32sim 当前 CO5300 显存模型为 468×466、列偏移 6。元数据必须分别表达面板 RAM、stride、可视裁剪和触摸变换，不能为了表格统一直接改成相同值。最终变换由实际驱动、测试图和实机触点共同确认。

## 纯静态的边界

| 需求 | 是否可以全静态 | 实施条件 |
| --- | --- | --- |
| 三类目录、搜索、3D、配件编辑 | 可以 | 构建期生成索引；个人数据保存在浏览器和导出文件 |
| 下载远程固件并执行 | 有条件 | HTTPS、可读取的 CORS 响应、完整镜像和兼容硬件模型 |
| 分享配置 | 可以 | URL 内配置，或公开的带哈希配置文件；大文件单独分发 |
| 短链接、跨设备私有云同步 | 无法只靠静态文件自动提供 | 首版不依赖；改用配置文件、长链接或用户自己的存储 |
| 固件任意 TCP/UDP/MQTT 联网 | 无法原样通过普通网页直连 | 默认虚拟网络/离线场景；可选网关后不再是零后端运行 |
| 真无线电、功耗、精确波形 | 不作为软件仿真验收 | 保留用户硬件进行对照 |

浏览器下载固件与固件自己联网是两条路径。即使 `fetch()` 能下载镜像，也不表示镜像里的 Wi-Fi/MQTT/TLS 客户端能访问外网。esp-emulator 的浏览器外网路径需要 WebSocket/TAP 代理；esp32sim 的浏览器模式也没有原生版的 socket NAT。[esp-emulator 浏览器说明](https://github.com/espressif/esp-emulator/blob/main/docs/guides/BROWSER.md)、[esp32sim WASM 说明](https://github.com/mikroverk/esp32sim/blob/ddf753a5c400ff79d3aa6a88a007eccd044e838d/docs/wasm.md)

跨域固件需要源站授权读取；下载链接可在新标签打开，不等于 JS 能读取。`no-cors` 只得到 opaque 响应，Service Worker 也不能绕过这一限制。首版提供 CORS 错误提示和本地上传；知名固件可由构建任务在许可允许时镜像到同源静态路径。[MDN CORS](https://developer.mozilla.org/en-US/docs/Web/HTTP/Guides/CORS)

基线选择不依赖 SharedArrayBuffer 的 Worker 构建，避免把跨源隔离作为上线前置条件。以后若选用 WASM threads，应单独验证 COOP/COEP 和所有资源的兼容性，不能用 meta 标签冒充响应头；也不要把 Service Worker 注入隔离头作为第一版默认依赖。[SharedArrayBuffer](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/SharedArrayBuffer)

[GitHub Pages](https://docs.github.com/en/pages/getting-started-with-github-pages/github-pages-limits)当前发布站点上限 1 GB、月流量软限制 100 GB。页面和核心资源适合 Pages，大量固件版本和高精度模型应按需加载、分开存放，并分别验证 CORS。构建时用 Actions 不影响网站运行时的纯静态属性。
