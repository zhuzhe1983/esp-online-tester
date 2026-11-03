# Stamp-Pico · C050-B 模型

C050-B 是五件装核心模块，本视图展示其中一件；K051 Mate 与 K051-B DIY Kit 的包装和散件保持独立。

厂家 [Stamp-Pico STL](https://github.com/m5stack/M5_Hardware/blob/e29c139cbc7c80b5ccc5557e5ab8025a7a19a109/Products/C050-B_Stamp-Pico/Structures/Stamp-Pico.stl) 的原始三角形和刚体变换保留不变。模型补充了黑色 PCB 轮廓、通孔与背面官方照片、独立薄纸贴承载面，以及官网明确列出的 M2×4 沉头内六角螺丝。纸贴按真实照片跨过盖板下方槽口，螺丝孔保持通透；背面只补可见金属环，不猜测隐藏螺柱。

PCB 厚度 1 mm、固定孔 Ø2.1 mm、焊盘孔 Ø0.85 mm 与 2.54 mm 孔距来自[官方尺寸图](https://static-cdn.m5stack.com/resource/docs/products/core/stamp_pico/stamp_pico_size_01.webp)。板形细节和 Z 位置、纸厚、螺丝头细节、背面金属环厚度仍有照片或装配估算。官方 Altium 文件是外部 PCB 模板，组件及 3D 模型流为空；它不能证明完整板级 CAD。照片只说明背面外观，未知元件面保留 `UNKNOWN`。

在正式页面点击“分开查看”，再拖动旋转，可读到板面上的 `UNKNOWN`；“恢复装配”、恢复视角、切换 2D 和更换产品均会归位。金色和半透明紫色只改变外壳，纸贴、PCB 与紧固件保留各自材质。展开距离 12 mm 是观察间距，不是实际拆解行程。

[本次验证记录](../evidence/m5stack-c050-b/model-refresh-2026-11-04.json)保存最终资产哈希、照片对照、材质与紧固件近景、正式页面操作和逐像素承载面检查。两个纹理的有效像素均未超出承载面或进入通孔；仅允许一个纹理像素的取整余量（约 0.03 mm）。[旧验证记录](../evidence/m5stack-c050-b/verification-before-refresh.json)保留旧资产结果；未带 `refresh-` 前缀的旧对照图属于此前版本。

原 CAD 宽 18 mm、高约 24.028 mm；原 CAD 厚 4.4 mm 与官方整机 4.6 mm 的差异保留，未拉伸厂家模型。本次仍为部分外观还原，没有完整制造级或实机验证，也不代表固件执行兼容。

复现使用固定输入，默认不访问网络：

```sh
python tools/rebuild-product-model.py plan m5stack-c050-b
python tools/rebuild-product-model.py build m5stack-c050-b --scratch <fresh-directory>
python tools/check-stamp-pico-surfaces.py m5stack-c050-b
```

依赖与输出说明见[模型重建](../model-reproduction.md)。[计划](../../../web/platform/assets/m5stack-c050-b/model-plan.json)完整记录来源、孔洞、纸面边界、照片变换、遮罩和观察组。厂家 CAD 为 MIT；厂家照片与纹理仍归厂家所有，没有套用项目许可。

实体 GPIO39 按键的点击面按官方尺寸图和盖板标识定位；直径 3 mm 是交互范围估算，不是厂家开关尺寸。正式页面的 3D 点击、键盘 A、分开查看后的点击及恢复装配后的点击，均由独立 IDF 测试固件输出新的 GPIO39 按下/松开记录与 G27 RGB 数值验证，见[交互记录](../evidence/m5stack-c050-b/public-control-review.json)。这项验证不等同于官方 UIFlow 应用兼容；原纸面和盖板遮挡保留，未为显示灯效额外开孔。
