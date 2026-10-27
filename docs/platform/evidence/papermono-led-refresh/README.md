# PaperMono RGB 漫射窗复核（2026-10-27）

原模型使用中心偏移的矩形块。现按原厂 STL 左侧胶囊开口（Y 5.9618～14.0382 mm、Z ±1.0384 mm）制作 8×2 mm 漫射片，中心 Y=10、Z=0；与官方 photo-10 的开口位置和圆角对照。15 µm 外侧偏移只用于消除共面闪烁，0.35 mm 深度是显示用估算，不代表 LED 封装或导光件结构。

`rgb-led-diffuser`、`parts['rgb-led-diffuser']`、`rgbLeds[0]` 和 `group.userData.rgbLedMeshes[0]` 指向同一独立发光材质。原色、金色、半透明紫色不改色漫射片；渲染检查通过实际 `applyRgbLedValues` 注入 `[255,0,80]`，仅证明状态到材质的显示，固件执行验证由板级测试负责。

[appearance-review.webp](appearance-review.webp) 是三种外壳与发光近景。[verification.json](verification.json) 记录最终资产、渲染器、相机及四视角对照哈希。尺寸五项通过；正面/背面/按键侧轮廓 IoU 为 0.9940/0.9904/0.9806，底部为 0.8964，未达到既定 0.95 门槛。保留底部失败；不据此声明整机 1:1 通过。原始机械几何和背贴字节未改动。

复现：打开 `platform/papermono-model-review.html`，调用 `modelReview.showView('left')`。近景使用 `modelReview.showView('left', {span:30,position:[-180,10,0],target:[0,10,0]})`。四视图保留页面原有相机。捕获画布时将实际 800×800 canvas 移到 body 顶层再截图，避免父 figure 裁切；最初扩大 CSS 而未移出父容器的捕获污染了结果（四项 IoU 0.2890/0.0242/0.4842/0.4486），属于截图方法错误，已重新捕获，未修改相机或放宽门槛。

```sh
python tools/verify-papermono-model.py --output docs/platform/evidence/papermono-led-refresh --allow-unpassed
```

照片原 URL、抓取时间及 hash 保留在 `assets/m5stack-papermono/model.json`，外观材质未经光学标定；未实物测量。
