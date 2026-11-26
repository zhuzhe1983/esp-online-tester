# M5Stack CoreS3-Lite 外观与知识入库

ESP32-S3 主机，16MB Flash、8MB PSRAM、2英寸触摸屏与GC0308摄像头。模型仅按Lite壳体源装配，未套用标准CoreS3的DIN底座。

来源：[官方产品文档](https://docs.m5stack.com/en/core/CoreS3-Lite)，M5_Hardware 固定 `e29c139cbc7c80b5ccc5557e5ab8025a7a19a109`。逐源 URL、UTC、SHA-256、下载状态见 [knowledge](../../../web/platform/knowledge/m5stack-k128-lite.json) 与 [source audit](../evidence/m5stack-k128-lite/source-audit.json)。旧抓取保留原始时间，不伪造刷新日期。

Original manufacturer components selected from assembled cluster; other components excluded with reasons. One rigid global transform, no scaling.

[model-plan.json](../../../web/platform/assets/m5stack-k128-lite/model-plan.json) 记录组件选择、刚体变换、照片裁切、纹理投影与估算表面；厂家几何不缩放。STL 保留源三角形；STEP 使用 OpenCascade7.8.1.1 以0.05mm弦偏差/0.2rad角度细分。UNKNOWN 是中性占位，未编造 PCB 走线或未核实电子元件。

| 检查 | 实测或误差 mm | 目标 mm | 结果 |
| --- | ---: | ---: | --- |
| rear-shell source-fidelity | 2.8610229563241774e-07 | 0 | 通过 |
| front-shell source-fidelity | 1.9073486345888568e-07 | 0 | 通过 |
| width | 54.0 | 54 | 通过 |
| height | 54.0 | 54 | 通过 |
| depth | 16.514999389648438 | 16.5 | 通过 |

实际平台 `manufacturer-model.js` 的 headless 渲染与原照片对照在 [verification.json](../evidence/m5stack-k128-lite/verification.json)：

- front: pass; IoU 0.992960624450847
- back-oblique: qualitative-only; IoU 0.40192869995501573
- side: qualitative-only; IoU 0.9481212638770282

正反视图只有照片确属近正投影且原始型号一致时才用0.95轮廓门槛。侧视/斜视保留为定性检查，不以轮廓掩盖标签、组件或装配差异。引用照片的纹理不是独立验证依据；未通过/缺失项不会改写成通过。

厂家 CAD 保留 MIT；官方照片及其裁切纹理版权归厂家，未验证独立开放图片许可，不套用项目 MIT。屏幕照片中的固件像素不作为模拟输出。runtime 为 unavailable，官方固件入口仅参考，未验证版本/hash/镜像不填猜测值。

重生成：

```sh
python tools/onboard-lane-01.py prepare --ids m5stack-k128-lite
python tools/onboard-lane-01.py build --ids m5stack-k128-lite
```

通过受跟踪的 headless 浏览器打开 `web/platform/review-lane-01.html?id=m5stack-k128-lite`；按 model reviewViews 保存800×800正反侧画布到 `output/playwright/m5stack-k128-lite-front.png` 等，再运行：

```sh
python tools/onboard-lane-01.py verify --ids m5stack-k128-lite
python tools/onboard-lane-01.py publish --ids m5stack-k128-lite
```

当前限制：

- Manufacturer exterior CAD only; board/display/peripheral runtime unavailable.
- No physical hardware measurements; colors are photograph-informed approximations, not calibrated materials.
- PCB has no audited complete board layout and is represented only by a visible neutral UNKNOWN placeholder.
- Official rear-photo SKU text reads CORE S3 SE; relation to Lite listing is retained as a source discrepancy. No hidden board inferred from this photo.

## 现行 Lite 官网照片与黄背盖

2026-11-26 10:39–10:40 UTC 重新抓取 Lite 官方页及 13 张图库原图，逐项保留 URL、最终 URL、媒体类型、字节数、时间和 SHA-256。当前第 02/03 图明确为 LITE 且带摄像头，第 05 图可见黄色背盖边缘，第 06 图标 BMI270/BMM150；第 07 图却与早期保留的 SE 字样黄背照片逐字节相同。该矛盾没有被改写成统一型号：保留 Lite 自身原厂 CAD、原生三角形及变换，不替换成 CoreS3-SE CAD，不从背贴文字推导内部硬件或固件身份。

依据 Lite 自身图库把后壳恢复为估算黄色，白纸只取照片实际纸边；SE 字样照原图保留并明确记录冲突。独立纸基底按原 CAD 0.2 mm 凹槽深度估算厚度，跨槽部分有照片连续纸面依据；它不是新增塑料底板，不补猜 PCB、螺钉或连接器。纸、纸基底与外壳材质分离，透明模式中原 UNKNOWN 仍保留。

[抓取证据、最终哈希、离线比较和 CPU 三材质背面/斜背/近侧视](../evidence/m5-rear-paper-20261126/m5stack-k128-lite/receipt.json) 同时列出限制。648,691 个 alpha≥128 纹素均有几何交点、无穿透，间隙中位及 99 百分位约 0.025 mm，边界最大约 0.225 mm，仍属未校准配准残差。纸贴逐字节离线重建通过；完整通用二进制/PNG/纹理记录比较仍失败，原生三角形位置与法向经明确量化和排序后全相同。未声称校准照片、完整机械装配或实物配合通过。
