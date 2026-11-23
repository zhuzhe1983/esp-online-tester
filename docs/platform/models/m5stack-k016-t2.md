# M5Stack StickT2 外观与知识入库

ESP32-PICO-D4 热成像手持设备，含 FLIR Lepton 3.0、135×240 屏幕和旋钮。厂家壳体包含安装耳，不能缩放到不含耳的标称宽度。

来源：[官方产品文档](https://docs.m5stack.com/en/core/StickT2)，M5_Hardware 固定 `e29c139cbc7c80b5ccc5557e5ab8025a7a19a109`。逐源 URL、UTC、SHA-256、下载状态见 [knowledge](../../../web/platform/knowledge/m5stack-k016-t2.json) 与 [source audit](../evidence/m5stack-k016-t2/source-audit.json)。旧抓取保留原始时间，不伪造刷新日期。

Original manufacturer components selected from assembled cluster; other components excluded with reasons. One rigid global transform, no scaling. Global180-degree Y turn places actual LCD aperture on+Z and mounting ears on photo-left; no independent shell relocation.

[model-plan.json](../../../web/platform/assets/m5stack-k016-t2/model-plan.json) 记录组件选择、刚体变换、照片裁切、纹理投影与估算表面；厂家几何不缩放。STL 保留源三角形；STEP 使用 OpenCascade7.8.1.1 以0.05mm弦偏差/0.2rad角度细分。UNKNOWN 是中性占位，未编造 PCB 走线或未核实电子元件。

| 检查 | 实测或误差 mm | 目标 mm | 结果 |
| --- | ---: | ---: | --- |
| shell-back source-fidelity | 9.52911378249155e-07 | 0 | 通过 |
| shell-front source-fidelity | 9.52911378249155e-07 | 0 | 通过 |
| side-key-a source-fidelity | 2.4566650402846335e-07 | 0 | 通过 |
| side-key-b source-fidelity | 4.7607422004602995e-07 | 0 | 通过 |
| width | 30.399999618530273 | 24 | 未通过 |
| height | 48.0 | 48 | 通过 |
| depth | 23.915000915527344 | 24 | 通过 |

首次导入时的实测结果收录在 [verification.json](../evidence/m5stack-k016-t2/verification.json)。

- front: pass; IoU 0.9896932209377229
- back: pass; IoU 0.9920216247349268
- side: qualitative-only; IoU 0.6997034042888265

正反视图只有照片确属近正投影且原始型号一致时才用0.95轮廓门槛。侧视/斜视保留为定性检查，不以轮廓掩盖标签、组件或装配差异。引用照片的纹理不是独立验证依据；未通过/缺失项不会改写成通过。

厂家 CAD 保留 MIT。对照用产品照片版权归 M5Stack。屏幕取模拟器的实际画面，运行和固件范围见 [StickT2 板级验收](../stickt2-runtime.md)。

## 接口、旋钮与背面

USB-C 与白色 HY2.0-4P 插座装在同一个端面，两个口的朝外法向均为 −Y。按端面照片校准口沿与卡耳，USB-C 的金属内壁与舌片退入原 CAD 开口，两口内部保持独立空间。插接深度未做实物测量。[端面照片](https://static-cdn.m5stack.com/resource/docs/products/core/m5stickt/m5stickt_06.webp)

侧面矩形开口装有旋钮，另有 M3 和 1/4 英寸安装孔。精确 SKU 的规格、引脚表、图库和结构件均未记录 TF 卡接口。模型补入可见的旋钮，保留原有可操作按键和存储能力。[官方标注图](https://static-cdn.m5stack.com/resource/docs/products/core/m5stickt/m5stickt_01.webp) · [原厂规格和引脚](https://docs.m5stack.com/en/core/StickT2)

后盖的 M5 标识是 0.50 mm 深的原厂凹刻。将覆盖凹槽的 131 个原始三角形分配给单独的尼龙材质，改善原色下的可读性，金色和半透明模式仍随外壳变化。按背面照片补齐镜头座、镜片和两颗内六角螺丝。将 UNKNOWN 板移到前屏后方的内部范围，端面不再露出一条板边。[背面照片](https://static-cdn.m5stack.com/resource/docs/products/core/m5stickt/m5stickt_03.webp)

爆炸图沿厚度分为三层。前壳带屏幕和按键，UNKNOWN 板带 USB-C、Grove 和可见旋钮组件，后壳带凹刻、螺丝和嵌在镜头孔中的可见光学细节。光学模组支架与旋钮的内部装配尚待核实。复位恢复原装配坐标。

Chromium 检查了实际模型的端面、斜侧、背面、外壳预设、后盖展开与复位、工作台 2D/3D 切换及分享嵌入恢复。带安装耳的完整模型实测为 30.4 × 48.0 × 23.94 mm。隐藏电路、光学和旋钮内部结构仍为待测项。[照片与修订前后对照](../evidence/m5stack-k016-t2/visual-fit-comparison.webp) · [本轮验证](../evidence/m5stack-k016-t2/visual-fit-20261123.json)

## 离线重建

```sh
python tools/rebuild-product-model.py build m5stack-k016-t2 --scratch /tmp/stickt2-model-check
```
