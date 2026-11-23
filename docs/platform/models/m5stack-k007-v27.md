# M5Stack FIRE v2.7 外观与知识入库

ESP32-D0WDQ6 主机，16MB Flash、8MB PSRAM、三按键、2英寸屏幕和十颗RGB灯。保留厂家v2.7组件，图库文件名仍沿用v2.6。

来源：[官方产品文档](https://docs.m5stack.com/en/core/Fire_v2.7)，M5_Hardware 固定 `e29c139cbc7c80b5ccc5557e5ab8025a7a19a109`。逐源 URL、UTC、SHA-256、下载状态见 [knowledge](../../../web/platform/knowledge/m5stack-k007-v27.json) 与 [source audit](../evidence/m5stack-k007-v27/source-audit.json)。旧抓取保留原始时间，不伪造刷新日期。

Original manufacturer components selected from assembled cluster; other components excluded with reasons. One rigid global transform, no scaling.

[model-plan.json](../../../web/platform/assets/m5stack-k007-v27/model-plan.json) 记录组件选择、刚体变换、照片裁切、纹理投影与估算表面；厂家几何不缩放。STL 保留源三角形；STEP 使用 OpenCascade7.8.1.1 以0.05mm弦偏差/0.2rad角度细分。UNKNOWN 是中性占位，未编造 PCB 走线或未核实电子元件。

| 检查 | 实测或误差 mm | 目标 mm | 结果 |
| --- | ---: | ---: | --- |
| battery-base source-fidelity | 2.8610229563241774e-07 | 0 | 通过 |
| side-button source-fidelity | 1.9073486257070726e-07 | 0 | 通过 |
| front-red-shell source-fidelity | 1.9073486257070726e-07 | 0 | 通过 |
| right-light-diffuser source-fidelity | 4.7683716530855236e-08 | 0 | 通过 |
| button-c source-fidelity | 1.9073486257070726e-07 | 0 | 通过 |
| button-a source-fidelity | 1.9073486257070726e-07 | 0 | 通过 |
| left-light-diffuser source-fidelity | 4.7683716530855236e-08 | 0 | 通过 |
| button-b source-fidelity | 1.9073486257070726e-07 | 0 | 通过 |
| middle-shell source-fidelity | 1.9073486257070726e-07 | 0 | 通过 |
| bottom-shell source-fidelity | 4.2915344167226976e-07 | 0 | 通过 |
| width | 54.19251251220703 | 54 | 未通过 |
| height | 54.000003814697266 | 54 | 通过 |
| depth | 28.614999771118164 | 28.6 | 通过 |

实际平台 `manufacturer-model.js` 的 headless 渲染与原照片对照在 [verification.json](../evidence/m5stack-k007-v27/verification.json)：

- front: pass; IoU 0.9905979556252299
- back: pass; IoU 0.9908632680405793
- side-oblique: qualitative-only; IoU 0.3533106054736262

正反视图只有照片确属近正投影且原始型号一致时才用0.95轮廓门槛。侧视/斜视保留为定性检查，不以轮廓掩盖标签、组件或装配差异。引用照片的纹理不是独立验证依据；未通过/缺失项不会改写成通过。

厂家 CAD 保留 MIT；官方照片及其裁切纹理版权归厂家，未验证独立开放图片许可，不套用项目 MIT。屏幕照片中的固件像素不作为模拟输出。

## 模拟器运行范围

FIRE v2.7 与 Basic 的 SPI 屏幕、三枚实体键和 TF 卡接线一致，复用 `m5stack-basic` 板型。固件通过 GPIO39、38、37 读取 A、B、C 三枚低有效按键。工作台 3D 模型使用对应的实体键帽。运行配置为 16 MiB Flash 和 4 MiB PSRAM，硬件规格为 8 MiB PSRAM。 [官方引脚与规格](https://docs.m5stack.com/en/core/Fire_v2.7)

官方 FIRE UIFlow2 v2.5.5 通过 ROM 启动，在真实浏览器 WASM 中输出 320×240 画面。原生模拟中，A 键让界面从 DEVELOP 切换到 APP.RUN，输入无关的 GPIO34 后画面与无输入时完全一致。UART0 的 MicroPython REPL 使用 `machine.SDCard(slot=2, sck=18, mosi=23, miso=19, cs=4)` 挂载 64 MiB FAT32 卡并读写 `PROOF.TXT`。同一测试中的 `slot=3` 构造返回 `ESP_ERR_NOT_FOUND`。 [运行收据](../evidence/m5stack-k007-v27/runtime-v27.json)

目录默认选择官方 EasyLoader 出厂恢复入口。本地提取的出厂镜像进入应用后发生整数除零并重启。需要运行模拟时，可选择固定版本的官方 FIRE UIFlow2 试运行项。MPU6886、灯带、音频、电源管理与外接端口仍待适配。未实机验证。 [官方固件来源与镜像布局](../evidence/core-family/README.md)

重生成：

```sh
python tools/onboard-lane-01.py prepare --ids m5stack-k007-v27
python tools/onboard-lane-01.py build --ids m5stack-k007-v27
```

通过受跟踪的 headless 浏览器打开 `web/platform/review-lane-01.html?id=m5stack-k007-v27`；按 model reviewViews 保存800×800正反侧画布到 `output/playwright/m5stack-k007-v27-front.png` 等，再运行：

```sh
python tools/onboard-lane-01.py verify --ids m5stack-k007-v27
python tools/onboard-lane-01.py publish --ids m5stack-k007-v27
```

当前限制：

- No physical hardware measurements; colors are photograph-informed approximations, not calibrated materials.
- PCB has no audited complete board layout and is represented only by a visible neutral UNKNOWN placeholder.
- Official current v2.7 page embeds filenames fire_v2.6_*. Exact cosmetic revision correspondence is not independently proven. Rear magnets/POGO contacts and some external connectors are incomplete.
