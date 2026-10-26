# Arduino Nesso N1 · DK001 外观与知识入库

M5Stack与Arduino合作的ESP32-C6开发设备，结合LoRa、触摸LCD、IMU与电池，提供多种低功耗无线通信与扩展接口。

来源：[官方产品文档](https://docs.m5stack.com/en/core/Arduino_Nesso_N1)，M5_Hardware 固定 `e29c139cbc7c80b5ccc5557e5ab8025a7a19a109`。逐源 URL、UTC、SHA-256、下载状态见 [knowledge](../../../web/platform/knowledge/m5stack-dk001.json) 与 [source audit](../evidence/m5stack-dk001/source-audit.json)。旧抓取保留原始时间，不伪造刷新日期。

Components 13 front shell,8 back shell and5 button share original assembly. Common translation only. Separate antenna geometry appears in two orientations/locations; excluded from body view to match official photos09/11 with antenna detached, not misrepresented as simultaneous mounted parts.

[model-plan.json](../../../web/platform/assets/m5stack-dk001/model-plan.json) 记录组件选择、刚体变换、照片裁切、纹理投影与估算表面；厂家几何不缩放。STL 保留源三角形；STEP 使用 OpenCascade7.8.1.1 以0.05mm弦偏差/0.2rad角度细分。UNKNOWN 是中性占位，未编造 PCB 走线或未核实电子元件。

| 检查 | 实测或误差 mm | 目标 mm | 结果 |
| --- | ---: | ---: | --- |
| shell-front source-fidelity | 9.5367431640625e-07 | 0 | 通过 |
| shell-back source-fidelity | 9.5367431640625e-07 | 0 | 通过 |
| front-button source-fidelity | 4.76837158203125e-07 | 0 | 通过 |
| width | 24.00003458291576 | 24 | 通过 |
| height | 48.00001741690721 | 48 | 通过 |
| depth without 0.9mm button protrusion | 21.100000381464028 | 21.1 | 通过 |

实际平台 `manufacturer-model.js` 的 headless 渲染与原照片对照在 [verification.json](../evidence/m5stack-dk001/verification.json)：

- front: pass; IoU 0.9791997361314395
- back: pass; IoU 0.976912700486606
- side: qualitative-only; IoU 0.8953974314474141

正反视图只有照片确属近正投影且原始型号一致时才用0.95轮廓门槛。侧视/斜视保留为定性检查，不以轮廓掩盖标签、组件或装配差异。引用照片的纹理不是独立验证依据；未通过/缺失项不会改写成通过。

厂家 CAD 保留 MIT；官方照片及其裁切纹理版权归厂家，未验证独立开放图片许可，不套用项目 MIT。屏幕照片中的固件像素不作为模拟输出。runtime 为 unavailable，官方固件入口仅参考，未验证版本/hash/镜像不填猜测值。

重生成：

```sh
python tools/onboard-lane-01.py prepare --ids m5stack-dk001
python tools/onboard-lane-01.py build --ids m5stack-dk001
```

通过受跟踪的 headless 浏览器打开 `web/platform/review-lane-01.html?id=m5stack-dk001`；按 model reviewViews 保存800×800正反侧画布到 `output/playwright/m5stack-dk001-front.png` 等，再运行：

```sh
python tools/onboard-lane-01.py verify --ids m5stack-dk001
python tools/onboard-lane-01.py publish --ids m5stack-dk001
```

当前限制：

- Only manufacturer exterior geometry is imported; board, display, keyboard, touch and RF runtime unavailable.
- No physical hardware measurement performed; official photographs may include shipping films and uncalibrated perspective.
- PCB is an explicitly estimated UNKNOWN placeholder without invented components or traces.
- Source CAD does not supply calibrated material properties; colors are source-photo-informed display approximations.
- ESP32-C6 CPU/board emulation unavailable. Antenna is detached in selected exterior view; complete antenna placement remains pending.
- Front glass is an independent photo-informed flat surface, not manufacturer glass CAD; inset/radius +/-0.5mm, optical stack and thickness unknown.
- Back printed paper is a separate exterior surface; its outline/rounding are photo-derived +/-0.35mm. Mechanical pockets behind the photographed paper are covered rather than textured as visible holes.
- Rear silver fasteners have exact CAD bore centers but photo-estimated head/rim/socket shapes; fastener standard, thread and internal attachment remain unknown.

贴纸修复复核：见 [sticker-verification.json](../evidence/m5stack-dk001/sticker-verification.json) 与 [三种材质正背斜视](../evidence/m5stack-dk001/sticker-appearance-grid.webp)。12张实际浏览器截图、非外壳材质不变和源图alpha/表面检查通过；[旧/新背面对照](../evidence/m5stack-dk001/sticker-before-after.webp) 保留被拒绝的整面投影。Adv刻字保留真实厂家CAD，金色直光下反差低、斜光可辨；Nesso银色紧固件采用独立金属材质，孔位依据CAD，头部尺寸依据照片估算。
