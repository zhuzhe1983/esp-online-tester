# M5Stack Cardputer-Adv · K132-Adv 外观与知识入库

带56键键盘与小型LCD的ESP32-S3便携开发设备，提供音频、IMU、存储卡和扩展接口。

来源：[官方产品文档](https://docs.m5stack.com/en/core/Cardputer-Adv)，M5_Hardware 固定 `e29c139cbc7c80b5ccc5557e5ab8025a7a19a109`。逐源 URL、UTC、SHA-256、下载状态见 [knowledge](../../../web/platform/knowledge/m5stack-k132-adv.json) 与 [source audit](../evidence/m5stack-k132-adv/source-audit.json)。旧抓取保留原始时间，不伪造刷新日期。

Components 0/2/3/6/9/10 share assembled coordinates. Others are translated duplicate printing parts. Common -90 degree Z rotation and -0.8mm Z translation only. Source width 84.06984 and max depth 19.67563 differ from one-decimal 84.0/19.6 targets; predeclared 0.1mm reporting tolerance, no geometry correction.

[model-plan.json](../../../web/platform/assets/m5stack-k132-adv/model-plan.json) 记录组件选择、刚体变换、照片裁切、纹理投影与估算表面；厂家几何不缩放。STL 保留源三角形；STEP 使用 OpenCascade7.8.1.1 以0.05mm弦偏差/0.2rad角度细分。UNKNOWN 是中性占位，未编造 PCB 走线或未核实电子元件。

| 检查 | 实测或误差 mm | 目标 mm | 结果 |
| --- | ---: | ---: | --- |
| shell-back source-fidelity | 2.8610229563241774e-07 | 0 | 通过 |
| keyboard-carrier source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| stamp-cover source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| source-part-6 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| lcd-frame source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| shell-front source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| key-r0-c0 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| key-r0-c1 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| key-r0-c2 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| key-r0-c3 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| key-r0-c4 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| key-r0-c5 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| key-r0-c6 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| key-r0-c7 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| key-r0-c8 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| key-r0-c9 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| key-r0-c10 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| key-r0-c11 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| key-r0-c12 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| key-r0-c13 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| key-r1-c0 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| key-r1-c1 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| key-r1-c2 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| key-r1-c3 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| key-r1-c4 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| key-r1-c5 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| key-r1-c6 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| key-r1-c7 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| key-r1-c8 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| key-r1-c9 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| key-r1-c10 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| key-r1-c11 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| key-r1-c12 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| key-r1-c13 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| key-r2-c0 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| key-r2-c1 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| key-r2-c2 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| key-r2-c3 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| key-r2-c4 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| key-r2-c5 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| key-r2-c6 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| key-r2-c7 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| key-r2-c8 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| key-r2-c9 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| key-r2-c10 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| key-r2-c11 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| key-r2-c12 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| key-r2-c13 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| key-r3-c0 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| key-r3-c1 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| key-r3-c2 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| key-r3-c3 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| key-r3-c4 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| key-r3-c5 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| key-r3-c6 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| key-r3-c7 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| key-r3-c8 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| key-r3-c9 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| key-r3-c10 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| key-r3-c11 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| key-r3-c12 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| key-r3-c13 source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| width | 84.06983947753906 | 84 | 通过 |
| height | 54.0 | 54 | 通过 |
| depth | 19.685001373291016 | 19.6 | 通过 |

实际平台 `manufacturer-model.js` 的 headless 渲染与原照片对照在 [verification.json](../evidence/m5stack-k132-adv/verification.json)：

- front: pass; IoU 0.9900916952761722
- back: pass; IoU 0.9928522992691564
- side: qualitative-only; IoU 0.9306208699263457

正反视图只有照片确属近正投影且原始型号一致时才用0.95轮廓门槛。侧视/斜视保留为定性检查，不以轮廓掩盖标签、组件或装配差异。引用照片的纹理不是独立验证依据；未通过/缺失项不会改写成通过。

厂家 CAD 保留 MIT；官方照片及其裁切纹理版权归厂家，未验证独立开放图片许可，不套用项目 MIT。屏幕照片中的固件像素不作为模拟输出。runtime 为 unavailable，官方固件入口仅参考，未验证版本/hash/镜像不填猜测值。

重生成：

```sh
python tools/onboard-lane-01.py prepare --ids m5stack-k132-adv
python tools/onboard-lane-01.py build --ids m5stack-k132-adv
```

通过受跟踪的 headless 浏览器打开 `web/platform/review-lane-01.html?id=m5stack-k132-adv`；按 model reviewViews 保存800×800正反侧画布到 `output/playwright/m5stack-k132-adv-front.png` 等，再运行：

```sh
python tools/onboard-lane-01.py verify --ids m5stack-k132-adv
python tools/onboard-lane-01.py publish --ids m5stack-k132-adv
```

当前限制：

- Only manufacturer exterior geometry is imported; board, display, keyboard, touch and RF runtime unavailable.
- No physical hardware measurement performed; official photographs may include shipping films and uncalibrated perspective.
- PCB is an explicitly estimated UNKNOWN placeholder without invented components or traces.
- Source CAD does not supply calibrated material properties; colors are source-photo-informed display approximations.
- Sticker masks retain only documented white paper. Shell gray, screws, keys and LCD are never projected. Paper perimeter/position remain image measurements +/-0.25mm.

56键使用原CAD键帽三角形分组，未生成触发方块：见[keyboard-partition.json](../evidence/m5stack-k132-adv/keyboard-partition.json)和[56/56实际渲染拾取](../evidence/m5stack-k132-adv/keyboard-picking.json)。顶面、侧壁、法线及载体全体三角形前后multiset hash完全一致。键帽与纸贴均不参与外壳配色；Fn的主机AltRight映射是平台约定，普通键与修饰键取自官方固定源码和照片。固件行为另行验证。

贴纸与外壳分离的最新12视角结果见[sticker-verification.json](../evidence/m5stack-k132-adv/sticker-verification.json)及[默认/金色/半透图](../evidence/m5stack-k132-adv/sticker-appearance-grid.webp)。CARDPUTER浅刻字保留厂家CAD，在金色直光下反差较低，斜光可辨。
