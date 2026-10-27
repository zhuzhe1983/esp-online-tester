# Stamp-S3A S007-V033 运行范围

`stamps3a` 是 S007-V033 的部分板级模型：8 MB Flash、未配置 PSRAM、G0 用户按键、G21 单颗 WS2812B 和 G38 数字供电控制。它依赖基础 Stamp-S3 的实现，但保留独立产品、板型和存储空间；没有自动启用其他 PIN 版本。

v0.3.3 原理图将 GPIO38 / DISP_BL 接到 AW35122FDR EN，BL_3V3 给 RGB 供电。官方 Arduino Quick Start 明确 `LED_EN 38` 并驱动高电平。只有 G38 作为输出且为高时，G21 的新 RMT 帧才可更新灯色；断电清除颜色，再上电不恢复旧帧。G38 改为输入也会断开虚拟灯电源。没有新增 SoC 寄存器常量，沿用既有 S3 GPIO/RMT 路由。

官网规格中的主稳压器是 MUN3CAD01-SC，原理图标 JW5712，保留这一差异。上述数字供电条件不模拟真实电流、时延、漏电、掉电电压或寄生供电。原始来源的实际抓取时间、URL 和 SHA-256 见 [来源收据](evidence/stamps3a/sources.json)。

## 官方固件与实际输入

S3A 官网的 UIFlow2 Quick Start 指向通用 Stamp-S3 教程，因此复用官方 Stamp-S3 UIFlow2 项目的远程发布检查，并增加独立 S3A 目标。默认版本为 2.5.3，项目 `1897200000000000020`、版本 `2098280680509362178`，镜像 SHA-256 为 `6e3b8a72ab7c0d46de2cc661cc2c47e178621161fc29fa34f6d0313fd7037d72`。8,384,512 字节镜像的应用描述为 `micropython / V2.5.3 / v5.5.4-dirty`。固件不提交到仓库或网站。

原生板型及实际 WASM worker 均经 ROM 启动，观察到 G38 输出高与 G21 RMT 蓝灯。实际产品下拉选择后的默认固件、确认按钮、下载执行、3D 工作台及 mini 页面均检查，工作台 28.14、mini 29.20 客体秒时为 `[0,0,255]`、无运行时错误。无屏设备不伪造 framebuffer。官方启动的 I2C 未初始化日志仍保留，完整应用按键与联网尚未验证，因此 UIFlow2 保持 `trial / simulationVerified:false`。官方启动统计中的 LED 更新数也包含关电清除事件，不能据此推断物理闪烁次数。见 [官方 worker](evidence/stamps3a/official-wasm.json)、[浏览器收据](evidence/stamps3a/browser.json)。

没有找到能证明出厂用途的固定 BIN，UIFlow2 明确是官方替代默认固件；官方 Arduino 示例不能冒充 factory 镜像。首轮平台回归发现旧同步映射只登记了 S007，已用官网教程关联补齐 S3A 目标并重新通过检查。

独立 ESP-IDF 5.5.1 fixture 经真实 GPIO/RMT 驱动，验证关闭供电时白/蓝帧被忽略、再上电不恢复旧色、必须发送新帧，以及输入方向断电、G0 按下/释放、平台重启。3D 盖板实际鼠标点击产生 G0=0、客体绿色 RMT、释放 G0=1；键盘 A 再使客体关闭 G38 并发送蓝色，观察到灯灭而不是蓝色。fixture 仅用于测试，不入公开固件库，也不作为官方完整兼容证据。见 [fixture 收据](evidence/stamps3a/fixture-wasm.json)。

## 模型与未实现范围

保留具体 S3A SKU 的厂家 STL 盖板和原照片，不采用旧 S3 的天线/按键 PCB STEP。六个来源、参考和几何文件可精确重建。盖板按钮命中面在原顶面上方 0.02 mm；绿色悬停位于盖板可按区域。新增 2×2 mm LED 的位置和 1 mm 高度依据官方产品照估算，未取得装配 CAD，不增造灯孔。金色、不透明材质、半透明紫色、背面 UNKNOWN、2D 与 mini 已实际查看。完整 PCB 缺失，保留中性 UNKNOWN 占位，**整机 1:1 验收仍为部分完成**。见 [模型收据](evidence/stamps3a/model.json)。

不支持或未验证：软件 GPIO 位发送灯时序、预留 FPC 屏、其余外接器件、物理 Wi-Fi/BLE、模拟电源、完整 ROM 下载模式和真实上电启动键。G0 在这里是运行时用户键。不能将 UIFlow2 启动蓝灯扩展成所有功能兼容。

## 复现与回归

```sh
tools/wasm-build.sh
node tools/probe-stamps3a-official.mjs <official-https-url> <sha256>
# 按 tools/fixtures/stamps3a-firmware/README.md 构建
node tools/probe-stamps3a.mjs --build <fixture-build-directory>
python tools/rebuild-product-model.py build m5stack-s007-v033 --scratch <fresh-scratch>
```

stable Rust 1.99.1、双 Clippy、1435 项 Rust / 132 组（包含 ignored、排除 external）、11 个生产 WASM 场景、100 项平台、实际录制、BLE/timing、111126 JIT 差分、24 GPIO 波形、目录与基准工具回归通过；未修改黄金输出。[完整检查](evidence/stamps3a/checks.json)；生产路径五对 CPU 对照的中位数为 S3 −4.000%、C3 +1.732%、C6 0%，指令、周期与 UART 完全一致；短任务时间量化和机器波动不构成性能优化结论。见 [CPU 收据](evidence/stamps3a/cpu.json)。
