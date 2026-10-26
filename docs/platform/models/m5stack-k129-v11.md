# M5Stack Capsule v1.1 · K129-V11 外观与知识入库

带 250mAh 电池的 Stamp-S3A 微型控制器，集成 BMI270、麦克风、RTC、红外、蜂鸣器及存储接口。

来源：[官方产品文档](https://docs.m5stack.com/en/core/Capsule_v1.1)，M5_Hardware 固定 `e29c139cbc7c80b5ccc5557e5ab8025a7a19a109`。逐源 URL、UTC、SHA-256、下载状态见 [knowledge](../../../web/platform/knowledge/m5stack-k129-v11.json) 与 [source audit](../evidence/m5stack-k129-v11/source-audit.json)。旧抓取保留原始时间，不伪造刷新日期。

Source0/3/4 are assembled white cap, shell and black Stamp-S3A cover; source1/2/5 are detached printing duplicates. Rigid Z180deg centers manufacturer cap at viewer top; Z-8.1mm. No scaling.

[model-plan.json](../../../web/platform/assets/m5stack-k129-v11/model-plan.json) 记录组件选择、刚体变换、照片裁切、纹理投影与估算表面；厂家几何不缩放。STL 保留源三角形；STEP 使用 OpenCascade7.8.1.1 以0.05mm弦偏差/0.2rad角度细分。UNKNOWN 是中性占位，未编造 PCB 走线或未核实电子元件。

| 检查 | 实测或误差 mm | 目标 mm | 结果 |
| --- | ---: | ---: | --- |
| button-cap source-fidelity | 3.8146972691777137e-07 | 0 | 通过 |
| shell source-fidelity | 3.889203075146952e-07 | 0 | 通过 |
| stamp-cover source-fidelity | 9.536743128535363e-08 | 0 | 通过 |
| width | 24.0 | 24 | 通过 |
| height | 40.0 | 40 | 通过 |
| depth | 16.200000762939453 | 16.2 | 通过 |

实际平台 `manufacturer-model.js` 的 headless 渲染与原照片对照在 [verification.json](../evidence/m5stack-k129-v11/verification.json)：

- front: fail; IoU 0.8925768316067696
- back: fail; IoU 0.8967438147520406
- side: qualitative-only; IoU 0.234298867935489

正反视图只有照片确属近正投影且原始型号一致时才用0.95轮廓门槛。侧视/斜视保留为定性检查，不以轮廓掩盖标签、组件或装配差异。引用照片的纹理不是独立验证依据；未通过/缺失项不会改写成通过。

厂家 CAD 保留 MIT；官方照片及其裁切纹理版权归厂家，未验证独立开放图片许可，不套用项目 MIT。屏幕照片中的固件像素不作为模拟输出。runtime 为 unavailable，官方固件入口仅参考，未验证版本/hash/镜像不填猜测值。

重生成：

```sh
python tools/onboard-lane-01.py prepare --ids m5stack-k129-v11
python tools/onboard-lane-01.py build --ids m5stack-k129-v11
```

通过受跟踪的 headless 浏览器打开 `web/platform/review-lane-01.html?id=m5stack-k129-v11`；按 model reviewViews 保存800×800正反侧画布到 `output/playwright/m5stack-k129-v11-front.png` 等，再运行：

```sh
python tools/onboard-lane-01.py verify --ids m5stack-k129-v11
python tools/onboard-lane-01.py publish --ids m5stack-k129-v11
```

当前限制：

- Manufacturer exterior CAD only; board/display/peripheral runtime unavailable.
- No physical hardware measurements; colors are photograph-informed approximations, not calibrated materials.
- PCB has no audited complete board layout and is represented only by a visible neutral UNKNOWN placeholder.
- Model uses the no-label front configuration explicitly pictured by the manufacturer; supplied loose label is not falsely printed into source geometry.
- USB/Grove mouths are source-photo-informed standard visual approximations; shell outlines remain manufacturer CAD.
- Capsule v1.1 changes core module to Stamp-S3A; earlier Capsule firmware/PCB is not assumed identical.
