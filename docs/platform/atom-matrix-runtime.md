# Atom-Matrix C008-B 运行适配

原版 Atom-Matrix 已接入经典 ESP32 板型 `atom-matrix`：G39 按键、G27 的 25 颗 RGB 链，以及 SDA25/SCL21 上的 MPU6886 轮询寄存器。官方 Factory Test v1.0 能从 ROM 启动并驱动灯阵，但仍停在 Wi-Fi 扫描；目录保持 `partial/trial`，没有把完整出厂测试标为通过。

## 来源和型号

[官方 C008-B 产品页](https://docs.m5stack.com/en/core/Atom-Matrix)给出 ESP32-PICO-D4、4 MB Flash、G39、G27×25 WS2812C-2020、G12 红外和 MPU6886 `0x68`。原版与 v1.1 分开处理，本次不启用 v1.1。官网 IMU 轴图以 USB 朝下时 X 右、Y 上、Z 出正面，与当前模型注册方向一致；没有实机轴向、噪声或时间标定。

官方分发完整 BIN 为 2,166,576 字节，SHA-256 `24892fcf5d5f8c97ee30a5ffb4fec166dca1c254cc6f54861af28abc8732ba2f`，镜像应用描述为 `arduino-lib-builder`，使用 ESP-IDF 4.4.3（`6407ecb3f8`）；只临时下载验收，不镜像到站点。抓取时间、地址、摘要与 M5Atom `ebbf736396f9459bf1adc7b588a7f1825b366844` 的接线/LED 源码见 [sources.json](evidence/atom-matrix/sources.json)。现有官方 UIFlow1/2 固定条目保留可尝试状态，本轮未执行它们；出厂镜像仍为默认第一项。

## 真实运行结果与限制

- 原生官方程序运行 60 客体秒，生产 WASM 运行 10 秒，均出现 `Atom Matrix Factory test` 和 `Wifi scan start`，并产生多色 25 灯 RMT 动画。工作台真实下载按钮、确认弹窗，以及分享 mini 自动启动也已观察。扫描没有结束，不能宣称联网或全部出厂流程兼容。
- 独立 ESP-IDF 5.5.1 源码测试通过真实 GPIO/RMT/I²C 驱动：G39 按下时首灯从红变绿，释放变红，末灯保持蓝色。鼠标点击 3D 盖板与键盘 A 均已实测。该测试不发布到固件库，不作为官方应用通过的替代。
- 真实鼠标拖拽后，模型姿态为 `[0.3024484,0.1620412,0.0648311,0.9370510]`，固件通过 MPU6886 读到加速度原始值 `[3597,13249,-8943]`，对应约 `[0.2195183,0.8086438,-0.5458086] g`。WASM 导出增加经典 ESP32 冷路径派发，不执行客体指令或改变客体时钟；非支持板型继续拒绝。
- 传感器沿用 DS-000193 rev1.1 的已有轮询子集：标识、量程、休眠/复位和一致突发采样；不支持 FIFO、中断、ODR 或模拟物理惯性。两个可能的硬件 I²C 控制器共享同一传感器状态，限定到真实 SDA25/SCL21 引脚；本阶段未实现该型号软件位操作 I²C。
- 原厂外壳 CAD 保留；25 个可发光区域位于已有扩散盖板表面，是光学显示近似，不是浮在玻璃上的实体 LED 封装。约 3.65 mm 间距和 2 mm 发光区来自具体型号照片，序号沿官方源码 `x+y*5`；逐灯物理方位未实机标定。原色、金色、紫透、悬停、拖动、2D 和 mini 已检查。背部连接器仍按旧有照片估算，PCB 保持 `UNKNOWN`，未声称完整内部 1:1。
- 修正既有连接器说明中误写的 VoiceS3R 名称；来源 ID 实际一直指向 C008-B 照片。Grove 的逻辑信号已知，但逐针物理朝向尚未确认，因此连接线仍禁用。红外、模拟供电、物理 Wi-Fi/BLE、Grove 夹具和全部官方交互未验证。

## 验证与复现

前盖是机械按键而非触摸屏。原版和 v1.1 的模型显式启用“短点击／原地按住操作按键，拖动旋转”：按下后的 220 ms 用于区分手势，移动超过 6 个 CSS 像素则转为旋转；取消、失焦不补发点击，已按住后转为拖动会释放按键。其他按键和真正的触摸屏不改变行为。浏览器检查两版前盖拖动均改变姿态且没有按钮事件；原版官方固件运行时，短点击产生一对输入，按住／释放改变 G39 状态。官方程序仍停留在 Wi-Fi 扫描，这次检查没有扩大它的应用兼容声明。回归覆盖 `platform-scene-test.mjs`、`platform-manufacturer-model-test.mjs` 及现有输入、触摸、编码器和真实 worker 测试。

```sh
node tools/probe-atom-matrix-official.mjs \
  https://m5burner-cdn.m5stack.com/firmware/100dd8d63b862f38d8c9c1b96a4706ce.bin \
  24892fcf5d5f8c97ee30a5ffb4fec166dca1c254cc6f54861af28abc8732ba2f
# ESP-IDF 5.5.1, target esp32; artifacts remain in ignored output.
idf.py -C tools/fixtures/atom-matrix-firmware -B "$PWD/output/atom-matrix-fixture-build" build
node tools/probe-atom-matrix.mjs --build output/atom-matrix-fixture-build
python tools/rebuild-product-model.py build m5stack-c008-b --scratch <independent-output>
```

[官方 WASM](evidence/atom-matrix/official-wasm.json)、[原生 60 秒](evidence/atom-matrix/native.json)、[源码驱动测试](evidence/atom-matrix/fixture-wasm.json)、[浏览器输入及截图](evidence/atom-matrix/browser.json)分别保留。首次 classic IMU ABI 拒绝、第二处 scene guard 遗漏、照片重建合同差异与本机 IDF 环境选择问题均有记录，未掩盖为一次通过。

[完整检查](evidence/atom-matrix/checks.json)包含双 Clippy、1436 Rust/132 组、11 WASM、100 平台测试、真实录制回放、111126 JIT/24 GPIO 与 CI 时序/历史检查；golden 未变。模型独立重建匹配 9 个资产及 model.json。[五对 CPU 对照](evidence/atom-matrix/cpu.json)相对固定 main `84e67571` 的 hello 中位用户 CPU 时间：S3 0.00%、C3 +1.30%、C6 +1.29%，UART、指令数和客体周期逐对一致；这是跨版本回归检查，百分之一秒计时粒度和宿主噪声限制微小差异的解释。没有改动 S3 核心，因此不要求 Pocket 对照；不把宿主测量波动解释为优化。任务浏览器和服务均已关闭。
