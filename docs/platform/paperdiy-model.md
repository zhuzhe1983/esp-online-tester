# PaperDIY Complete Kit 模型复核

K159-B 保留厂家 STL 中已装配的六个组件及原坐标，只做既有刚体旋转和平移。2026-11-13 复核补齐了屏幕黑色边框、背面 ESP32 屏蔽罩、M2 铜嵌件、电池插座，以及 USB 和 microSD 的凹入结构。屏幕只显示运行时 framebuffer；照片只用于屏蔽罩上的原始标记，没有用产品照片替代固件画面。

此前屏幕是 61 × 104 mm、中心 Y=2.1 mm 的估计平面，顶部留下空隙并露出内部卡扣。现在屏幕位置以官方图纸的 59.72 × 105.02 mm 开口和 Y=4 mm 中心为依据，活动平面保持 540:960 比例，四周有独立黑色边框。图纸开口不是光学有效区的测量；当前活动平面与层叠深度仍为照片约束的展示近似。

旧报告将完整装配的 121.27 mm 与产品表中的 120 mm 壳体长度比较，误记为尺寸失败。官方尺寸图明确同时标注了这两个数值。原厂 CAD 的外壳为 67 × 120 × 6.3 mm，包含侧键、底部电源键的完整模型为 **68.2000 × 121.2699 × 6.3000 mm**，均与独立图纸一致；没有拉伸或裁掉原厂按键。旧失败记录保留在 [previous-verification.json](evidence/m5stack-k159-b/model-review-20261113/previous-verification.json)。

正、背、底部、侧面照片对照的轮廓 IoU 分别为 0.989839、0.987002、0.981098、0.959763。该指标只检查外轮廓，受摄影透视、光照和像素边缘影响，不代表内部结构或制造公差通过。对照图、固定相机参数、分割方法及最终资产摘要见 [复核收据](evidence/m5stack-k159-b/model-review-20261113/receipt.json) 和 [照片比较](evidence/m5stack-k159-b/model-review-20261113/photo-comparison.json)。

原色、金色和半透明紫色均检查了正反面、端口、斜视和近侧视。屏蔽罩、接口、铜嵌件、按键和屏幕保持自己的材质。透明外壳和“分开查看”能够读到主板的 **UNKNOWN**：目前没有完整板级 CAD，主板外形、隐藏器件和走线仍为未知。已补的可见电子件由该型号照片和图纸约束，隐藏厚度、螺纹及连接器内部细节没有制造精度依据；分离偏移仅方便观察，不代表真实拆装顺序。

重建使用已提交的模型计划、原厂 STL、原图、尺寸图及许可文件，不需要私有素材目录：

```sh
python tools/rebuild-product-model.py plan m5stack-k159-b
python tools/rebuild-product-model.py build m5stack-k159-b --scratch /tmp/paperdiy-rebuild
node --test tools/platform-paperdiy-model-test.mjs
```

在 `review-lane-02.html?id=m5stack-k159-b` 上通过跟踪会话的 Playwright CLI 运行 `tools/paperdiy-model-review.js`，再用 `tools/verify-paperdiy-model-photos.py --screenshots output/playwright --output <目录>` 可复查多视图和照片轮廓。浏览器用例的灰色屏幕是明确的几何测试纹理。另保留了实际 UserDemo v1.0 framebuffer 映入模型的画面，只用于核对显示比例和边缘；本次模型修改不扩展 [固件适配结论](paperdiy-runtime.md)。
