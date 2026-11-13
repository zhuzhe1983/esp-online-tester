# PaperDIY Complete Kit 模型复核

K159-B 保留厂家 STL 中已装配的六个组件及原坐标，只做既有刚体旋转和平移。2026-11-13 复核补齐了屏幕黑色边框、背面 ESP32 屏蔽罩、M2 铜嵌件、电池插座，以及 USB 和 microSD 的凹入结构。屏幕只显示运行时 framebuffer；照片用于可核验的板面标识与反面外观，没有用产品照片替代固件画面。

此前屏幕是 61 × 104 mm、中心 Y=2.1 mm 的估计平面，顶部留下空隙并露出内部卡扣。现在屏幕位置以官方图纸的 59.72 × 105.02 mm 开口和 Y=4 mm 中心为依据，活动平面保持 540:960 比例，四周有独立黑色边框。图纸开口不是光学有效区的测量；当前活动平面与层叠深度仍为照片约束的展示近似。

旧报告将完整装配的 121.27 mm 与产品表中的 120 mm 壳体长度比较，误记为尺寸失败。官方尺寸图明确同时标注了这两个数值。原厂 CAD 的外壳为 67 × 120 × 6.3 mm，包含侧键、底部电源键的完整模型为 **68.2000 × 121.2699 × 6.3000 mm**，均与独立图纸一致；没有拉伸或裁掉原厂按键。旧失败记录保留在 [previous-verification.json](evidence/m5stack-k159-b/model-review-20261113/previous-verification.json)。

正、背、底部、侧面照片对照的轮廓 IoU 分别为 0.989839、0.987002、0.981098、0.959763。该指标只检查外轮廓，受摄影透视、光照和像素边缘影响，不代表内部结构或制造公差通过。对照图、固定相机参数、分割方法及最终资产摘要见 [复核收据](evidence/m5stack-k159-b/model-review-20261113/receipt.json) 和 [照片比较](evidence/m5stack-k159-b/model-review-20261113/photo-comparison.json)。

后续 PCB 复核补充了官方 Assembly Kit 的裸板照片 05–09 和尺寸图。此前主板采用 60 × 108 mm 的整壳占位，这与裸板照片不符；现已改为位于机身下部、按 61.15 mm 直边宽度标定的黑色异形板，板厚使用图纸的 1 mm。板边缺口、两处安装孔、ESP32 模块及悬伸天线、完整 TF 金属座、两只 FPC、显示连接器、三个板上开关、97 个可辨被动器件和 13 个 IC 外形均按照片地标建立独立几何。元件坐标不是由原理图猜测，封装名称和精确高度也未冒充工程数据。

原色、金色和半透明紫色共检查 34 个视图。PCB 和元件保持自己的材质；板面少量文字及反面可见走线用未修改原图的 UV 区域投射，孔洞及已有立体金属区域予以裁开。屏蔽罩上的厂家/型号使用透明文字近似，金属罩和 TF 座由 PBR 材质受光，避免将照片高光永久贴在模型上。板上的 **UNKNOWN** 保留在内侧，表示隐藏铜层、精确封装高度和制造公差仍未经核准；板形与两面可见外观已有官方依据。分开查看时 PCB、所有器件、标识和原厂 USB CAD 一起移动，复位恢复原坐标。

可直接打开 [官方照片与模型对照](evidence/m5stack-k159-b/pcb-review-20261113/comparison.html)，来源、像素标定、尺寸检查、最终资产哈希和保留截图见 [PCB 复核收据](evidence/m5stack-k159-b/pcb-review-20261113/receipt.json)。A/B 两按键的照片间距为约 12.009 mm，与独立图纸的 12 mm 接近；它是视觉约束交叉检查，不是制造精度声明。原厂外壳六个组件及 geometry.bin 未改。

重建使用已提交的模型计划、原厂 STL、原图、尺寸图及许可文件，不需要私有素材目录：

```sh
python tools/update-paperdiy-pcb-plan.py  # 固定像素地标生成 PCB 计划，可重复执行
python tools/rebuild-product-model.py plan m5stack-k159-b
python tools/rebuild-product-model.py build m5stack-k159-b --scratch /tmp/paperdiy-rebuild
node --test tools/platform-paperdiy-model-test.mjs tools/platform-photo-surface-test.mjs
```

在 `review-lane-02.html?id=m5stack-k159-b` 上通过跟踪会话的 Playwright CLI 运行 `tools/paperdiy-model-review.js`，再用 `tools/verify-paperdiy-model-photos.py --screenshots output/playwright --output <目录>` 可复查多视图和照片轮廓。浏览器用例的灰色屏幕是明确的几何测试纹理。另保留了实际 UserDemo v1.0 framebuffer 映入模型的画面，只用于核对显示比例和边缘；本次模型修改不扩展 [固件适配结论](paperdiy-runtime.md)。
