# Waveshare 完整镜像分区表识别

原有应用直启兼容策略只在远程裸 BIN 未明确指定 ROM 启动、且完整镜像检查报 `INCOMPLETE_IMAGE` 时生效。Waveshare P4 LCD3.5 的两个原厂固件指定 `bootMode: 'rom'`，包含完整 16 MiB Flash 内容，分区表却在 `0x9000`。加载器默认只检查 `0x8000`，因而在创建执行环境之前错误拒绝了镜像。原厂烧录说明也明确要求从地址零写入这两份合并镜像：[固定版本 README](https://github.com/waveshareteam/ESP32-P4-WIFI6-Touch-LCD-3.5/blob/3e2fcd48fe8847e83fd2556522b33728fa247d64/firmware/README.md)。

加载器现在识别完整镜像中按扇区对齐的分区表，继续校验分区边界、重叠、启动镜像、应用镜像、芯片及晶圆版本。多个候选分区表仍会拒绝，并要求明确布局。远程 BIN、本地 BIN、单完整镜像清单和烧录计划共用识别结果；原有应用直启规则不变，完整镜像继续走 ROM 启动。实际布局保存在加载回执中，虚拟配置读写也采用这个地址。

## 复现与检查

1. 从固定来源下载两个原厂 BIN，并对照 [checks.json](checks.json) 的 SHA-256。
2. 使用修复前的 `ad2b1eef466a4d90209c32ca9587dd64576cfa7f` 调用 `validateMergedImage(bytes, board, 16*1024*1024)`，两个镜像均报 `INCOMPLETE_IMAGE`；同版本显式传入 `{partitionTableOffset:0x9000}` 即通过。
3. 使用当前 `mergedImageLayout`，两个镜像均识别为启动镜像 `0x2000`、分区表 `0x9000`。
4. 按项目文档准备 WASM 和 ROM，运行 `ESP32SIM_ROM_DIR="$PWD/roms" node --test tools/platform-*-test.mjs`，525 项通过；`ESP32SIM_ROM_DIR="$PWD/roms" node tools/check-waveshare-stat-order.mjs`，4 项通过。新增布局夹具只验证加载和边界，不作为启动证据。
5. 在工作台运行 `tools/fixtures/waveshare-merged-layout-browser-check.js`。该 Playwright CLI 夹具选择目录中的默认原厂固件，点击“下载并尝试运行”，通过真实 Worker/WASM 执行并捕获屏幕；也适用于 Pages 部署后的复验。使用项目包装脚本管理并关闭浏览器会话。

两个原始镜像均实际执行超过 5 秒 guest 时间，产生 320×480 的启动画面和 `Starting Wi-Fi...` 串口输出；未修改固件、替换演示程序或启用 stub。芯片与晶圆版本不匹配仍被拒绝。浏览器截图同时核对工作台中的真实屏幕。

## 限制与证据保留

这次修复消除了镜像布局的误判。板载 C6 无线协处理器仍未实现，原厂固件随后发生 `Instruction access fault` 并重启，完整原厂运行验收仍未通过。产品的 partial、trial 和未验证标记均保留；不声明完整 Wi-Fi、音频、TF 文件系统或实物烧录通过。

紧凑回执保留固定来源、输入与代码哈希、前后检查结果、实际屏幕哈希和负面结果。原始 BIN、完整 UART、原始回执、测试日志与浏览器截图保留在本地 Git 公共目录的 `waveshare-adaptation-20261124/verification/firmware-layout/` 及忽略的 `output/playwright/`，未提交私人路径、会话名称或原始捕获。屏幕受异步刷新和重启时刻影响，不同采样的图像哈希不作为 bit-identical 比较。此次未改变 Rust、WASM 内核或每步执行工作。
