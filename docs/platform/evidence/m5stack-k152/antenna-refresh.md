# Cardputer Mesh Kit 天线复核（2026-10-27）

SKU K152。旧天线是 30 mm 直柱，轴线 X=29、Z=-15 mm，偏离原厂安装孔；旧正面照片故意淡出天线上段，不能用淡出部分推断实际长度。本次读取官方完整 photo-06 及产品规格，恢复包含接头在内的完整 108×9.3 mm 天线。接头、银色标签带、橡胶段、圆头分别建模；橡胶不随外壳改色。

安装位置由原始 CAD 的半径 7 mm 座孔圆弧拟合，轴线为 X=33.959989、Z=-17.149995 mm，表面 Y=36.039993 mm。7 个圆弧点的最大拟合残差约 0.000003 mm；天线轴线误差 0.000002 mm。原厂 STL 和导出机械二进制保持不变。`antenna-refresh.json` 的检查验证长度、直径、方向及安装轴线。

[原色/金色/紫透与安装近景](antenna-appearance-review.webp)、[实际工作台](antenna-workbench.webp) 均显示完整天线及同心安装。[正面照片叠图](antenna-front-comparison.webp) 保留未解决差异：photo-07 的天线水平位置约为 X=29.6 mm，原厂 CAD/尺寸图约为 X=34 mm；未移动或缩放厂家壳体去掩盖冲突。源照片淡出长天线，不能据此计算完整轮廓 IoU。完整 photo-06 用于检查形状，当前完整天线斜视图未完成相机标定，不宣称 1:1 通过。

`antenna-browser-review.json` 保存本次实际重新获取来源的 URL、时间、字节数、hash，以及相机和截图回执。原始来源抓取时间不被覆盖。

```sh
python tools/verify-meshkit-antenna.py --output docs/platform/evidence/m5stack-k152/antenna-refresh.json
python tools/rebuild-product-model.py build m5stack-k152 --scratch /tmp/meshkit-rebuild
node tools/platform-manufacturer-model-test.mjs
```

重建已离线通过，7 个输入资产及模型配置一致。`lathe` 声明式轮廓支持限制点数、有限坐标、半径和单调高度，不加载模型脚本。108 mm 是含接头的总长；各段长度、圆头及防滑纹数量来自照片估算，打印频段文字暂未重建。未获天线制造 CAD、内部接触/转轴尺寸或实物测量；原模型既有的 Mesh Kit 贴纸缺失不在这次天线修正中解决。
