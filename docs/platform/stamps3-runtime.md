# Stamp-S3 S007 运行范围

`stamps3` 是基础 Stamp-S3 的部分板级模型：8 MB Flash、无 PSRAM，G0 用户按键和 G21 单颗 WS2812B-2020。不自动启用 PIN1.27 或 PIN2.54。Stamp-S3A 使用独立的 G38 供电模型，见 [S3A 运行范围](stamps3a-runtime.md)。

V0.2 原理图中 G0 经 10 kΩ 上拉、按下接地，G46 默认下拉，WS2812B 的 DIN 接 G21、3V3 供电。固定官方 `m5stack/STAMP-S3` 提交 `6b64d431f930ff5e2bd770a07003ac983c24bc3f` 的 LED 例程同样使用 G0/G21、GRB 顺序。原理图、源码、下载元信息与抓取摘要见 [来源收据](evidence/stamps3/sources.json)。没有新增 SoC 寄存器常量，沿用既有 S3 GPIO/RMT 控制器。

## 官方固件与输入证据

默认固件为官方 UIFlow2 v2.5.3，项目 `1897200000000000020`、版本 `2098280680509362178`。完整镜像 8,384,512 字节，SHA-256 `6e3b8a72ab7c0d46de2cc661cc2c47e178621161fc29fa34f6d0313fd7037d72`；应用描述为 `micropython / V2.5.3 / v5.5.4-dirty`。没有将第三方二进制提交到仓库或网站。

原生及实际页面 WASM worker 均经真实 ROM 启动，收到 G21 RMT 蓝灯。浏览器从设备下拉框默认选中 UIFlow2，点击试运行确认后下载并执行；工作台 20.70、mini 75.40 客体秒时均为 `[0,0,255]`。无屏设备保持 `frame=null`。保留官方启动日志 `i2c.master: this port has not been initialized`；尚未证实官方应用按键行为、联网或 MicroPython 交互，因此固件仍为 `trial / simulationVerified:false`。见 [官方 worker 收据](evidence/stamps3/official-wasm.json)、[浏览器收据](evidence/stamps3/browser.json)。

本轮仍未找到可以证明出厂用途的固定 BIN。官网链接的 Arduino LED 源码是官方示例，单独保留资料入口；UIFlow2 标为官方替代固件，不能冒充 factory。此前的出厂调查也保留在 `research/factory-firmware-audit-2026-10-27.json`，本记录补充后续已验证启动与默认固件的状态。

内部 `tools/fixtures/stamps3-firmware` 使用 ESP-IDF 5.5.1 的 GPIO/RMT 公共驱动。实际 worker 验证红→绿→蓝→灭→红、按下/释放及平台重启释放；错误引脚和不完整帧由板级单测验证。3D 盖板真实鼠标命中产生客体 `STAMPS3_BUTTON 0`、`STAMPS3_RMT 0 255 0`，释放读回 1；键盘 A 再产生蓝灯。内部 fixture 不进入公开固件库、不作为官方兼容证据。见 [fixture 收据](evidence/stamps3/fixture-wasm.json)。

## 模型与限制

保留厂家原始隔热盖板、机械 STEP 的板形/USB/天线/螺丝，未改动三角形。盖板可按区域取 CAD 原始顶面内部，命中平面仅高出 0.02 mm；悬停绿色，鼠标与键盘走相同 G0 输入。

单颗 LED 的 2×2 mm 外形与位置依据官方裸板照片和 18 mm 板宽估算；1 mm 高度不是厂家装配 CAD。它位于原盖板之下，没有新增灯孔或导光件。原厂色、金色、半透明紫色、背面 UNKNOWN、2D 与 mini 已实际检查，LED 色来自客体输出。八个原始几何/来源文件可精确重建；照片的裸板/带盖板配置差异、完整 PCB 未知等旧偏差仍保留，**未宣布整机 1:1 验收通过**。见 [模型收据](evidence/stamps3/model.json)。

不支持或未验证：软件 GPIO 位发送灯时序、FPC 外接屏、所有扩展引脚的器件连接、物理 Wi-Fi/BLE、模拟稳压/电流/供电、芯片级 ROM 下载键的重新上电选择。3V3 虚拟存在；G0 作为运行时用户键，不等于完整实机启动模式模拟。

## 复现与回归

```sh
tools/wasm-build.sh
node tools/probe-stamps3-official.mjs <official-https-url> <sha256>
# 按 tools/fixtures/stamps3-firmware/README.md 构建内部 fixture
node tools/probe-stamps3.mjs --build <fixture-build-directory>
python tools/rebuild-product-model.py build m5stack-s007 --scratch <fresh-scratch>
```

最新 stable Rust 1.99.1、双 Clippy、1434 项 Rust / 132 组（ignored 纳入、external 排除）、虚拟调度、11 个生产 WASM 场景、100 项平台测试、真实录制、BLE/timing、111126 JIT 差分、24 GPIO 波形、基准工具及目录检查通过；没有修改黄金输出。见 [完整检查](evidence/stamps3/checks.json)。CPU 五对对照最终中位数为 S3 0%、C3 +0.431%、C6 0%，指令、周期和 UART 一致；见 [收据](evidence/stamps3/cpu.json)；可能与一次独立 4.22 秒编译重叠的初轮已单独保留且不用于结论。
