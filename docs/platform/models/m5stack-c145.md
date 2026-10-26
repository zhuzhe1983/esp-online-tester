# M5Stack Tab5 · C145 外观与知识入库

以ESP32-P4为主控、ESP32-C6提供无线连接的5英寸触摸终端，具备摄像头、音频和工业扩展接口。C145不含电池，K145套装另列。

官方依据：[Tab5](https://docs.m5stack.com/en/core/Tab5)。目录固定到 M5_Hardware `e29c139cbc7c80b5ccc5557e5ab8025a7a19a109`；逐文件地址、真实UTC、SHA-256和许可在 [knowledge](../../../web/platform/knowledge/m5stack-c145.json) 与 [source audit](../evidence/m5stack-c145/source-audit.json)。旧抓取记录按原时间复用，没有伪造新抓取。

Components 0/3/4/7/8 share assembled bounds at x0..128,y0.889717..80.889717,z-12..0. Duplicates have large negative Y translations. Common rigid 90-degree Z rotation centers portrait body; CAD display-facing side orientation will be checked in browser.

模型选择、刚体变换、排除组件、照片裁切/遮黑区域、投影范围和估算表面写入 [model-plan.json](../../../web/platform/assets/m5stack-c145/model-plan.json)。原厂家三角形不缩放。原始 CAD 按厂家 MIT 保留；公开照片与衍生纹理版权归厂家，未验证独立开放图片许可，不套用项目 MIT。所有照片里的 LCD 固件内容均排除，预览屏幕为空白。

| 检查 | 实测或误差 mm | 目标 mm | 结果 |
| --- | ---: | ---: | --- |
| battery-lock source-fidelity | 1.0205078382341526e-07 | 0 | 通过 |
| source-part-3 source-fidelity | 1.0205078382341526e-07 | 0 | 通过 |
| source-part-4 source-fidelity | 1.9073486328125e-06 | 0 | 通过 |
| battery-rail source-fidelity | 8.516235325828347e-07 | 0 | 通过 |
| shell source-fidelity | 1.9073486328125e-06 | 0 | 通过 |
| width | 80.0 | 80 | 通过 |
| height | 128.0 | 128 | 通过 |
| depth | 12.014999389648438 | 12 | 通过 |

[正面](../evidence/m5stack-c145/front-comparison.webp)、[背面](../evidence/m5stack-c145/back-comparison.webp)、[侧面](../evidence/m5stack-c145/side-comparison.webp)对照和实际数值见 [verification.json](../evidence/m5stack-c145/verification.json)。正反外轮廓采用预置0.95 IoU门槛；侧面保留差异，只做定性检查。标签使用同张照片裁切，因此标签相似不是独立证据。相机按官方照片轮廓与独立尺寸标定，不能变形模型来拟合。

内部主板为可见 `UNKNOWN` 空白占位，尺寸和位置是壳体空间估算；没有编造PCB走线或组件。屏幕、键盘、触摸、传感器与射频没有接入可执行板级模拟器，本条 runtime 为 unavailable；未做实机测量。固件入口仅保留官方发布来源，未验证的版本、镜像布局与hash不填写猜测值。

重生成（Python依赖见 tools/requirements-model.txt）：

```sh
python tools/onboard-lane-01.py prepare --ids m5stack-c145
python tools/onboard-lane-01.py build --ids m5stack-c145
```

启动静态服务器后，在 `web/platform/review-lane-01.html?id=m5stack-c145` 打开受跟踪的headless浏览器；按正反侧视保存800×800画布到 `output/playwright/m5stack-c145-front.png` 等路径，再运行：

```sh
python tools/onboard-lane-01.py verify --ids m5stack-c145
```

检查只是外观、尺寸和证据链，不证明固件兼容。剩余偏差：

- Only manufacturer exterior geometry is imported; board, display, keyboard, touch and RF runtime unavailable.
- No physical hardware measurement performed; official photographs may include shipping films and uncalibrated perspective.
- PCB is an explicitly estimated UNKNOWN placeholder without invented components or traces.
- Source CAD does not supply calibrated material properties; colors are source-photo-informed display approximations.
- P4 CPU/board unsupported; C6 wireless coprocessor is not a substitute main controller. C145 model excludes K145 NP-F550 battery.
- Front glass is an independent photo-informed flat surface, not manufacturer glass CAD; inset/radius +/-0.5mm, optical stack and thickness unknown.
