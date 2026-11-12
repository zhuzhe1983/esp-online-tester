# M5Stack PaperMono-Lite · C153-Lite

Lite 使用独立官方 STL 和照片；保留厂家原生毫米坐标，仅作共同平移。原厂装配的壳体与三个侧键使用分量 0/2/3/4，打印排版副本 1/5/6/7 不混入装配。官方尺寸为 62×101×8 mm，壳体本体宽 61 mm。

2026-11-13 修正前盖被 CAD 安装沿穿出的现象，补齐侧面 RGB 光窗、A/B 键帽输入映射，并按 Lite 图纸修正后背黑盖尺寸和纸贴底边。原色、金色、紫透，以及正背侧、上下与斜视已检查；详见 [本次外观验收](../evidence/m5stack-c153-lite/model-review-20261113/README.md)。光窗接现有客体 RGB 输出索引，验证的是渲染和映射，本轮未重新运行固件。

复现使用公开入口：

```sh
python tools/rebuild-product-model.py plan m5stack-c153-lite
python tools/rebuild-product-model.py build m5stack-c153-lite --scratch output/lite-model-rebuild
node tools/platform-papermono-lite-model-test.mjs
```

依赖和离线重建说明见 [模型复现](../model-reproduction.md)。旧版 [尺寸与照片记录](../evidence/m5stack-c153-lite/verification.json)及 [来源审计](../evidence/m5stack-c153-lite/source-audit.json)作为历史证据保留，不代表本次资产。

外观状态仍为 **partial**：内部 PCB 是 UNKNOWN 占位；连接器细节、材料色彩和粗糙度为显示近似，未作实机测量。纸贴采用 Lite 官方照片透明遮罩，屏幕由运行时纹理显示，不覆盖宣传图。电源键未新增行为。已有显示、触摸、TF、RGB、IMU 和联网的实际范围见 [运行说明](../papermono-runtime.md)，本次模型修正不扩大兼容结论。
