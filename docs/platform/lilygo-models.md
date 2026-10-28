# LILYGO 首批四款 CAD 预览

本次接入原版 TTGO T-Display、非触摸 T-Display-S3、原版 T-Deck 和原版 T-Embed CC1101 的厂家 CAD。四款均标记为 `partial`。源文件、固定提交、原始 URL、实际 UTC 抓取时间、字节数及 SHA-256 保存在各自 `assets/<id>/model-plan.json` 与 `evidence/<id>/source-audit.json`。模型与固件适配分别验收。

| 产品 | 导入对象 | 导出包络（毫米） | 尚未完成的部分 |
| --- | --- | --- | --- |
| TTGO T-Display | 完整 PCB/LCD/USB CAD，移除照片中未焊接的 24 组可选排针；保留 GPIO0/GPIO35 实际按键几何 | 52.842 × 25.192 × 7.675，横屏摆放 | PCB 丝印/材料未恢复；有效显示区为照片估算 |
| T-Display-S3 | 原版非触摸 PCB、LCD 和支架 | 61.982 × 26.313 × 9.880，横屏摆放 | PCB 为厂家组合实体，尚未逐器件配色；显示有效区估算 |
| T-Deck | 原版裸板 PCB、屏幕、键盘与轨迹球机械装配 | 69.592 × 101.527 × 14.600 | 官网的 68 × 100 × 11 与 CAD 不同；逐键扫描/命中未实现 |
| T-Embed CC1101 | 壳体、旋钮、侧键，按源坐标装配 | 98.000 × 41.800 × 30.850，含侧键 | 官网 97.5 × 39 × 31 与 CAD 包络定义不同；独立 PCB 坐标未核对，暂用 UNKNOWN；透明壳原色光学材料未标定 |

原版 T-Display 和 S3 的原生竖屏纹理通过 `-π/2` 旋转显示平面映射到横置模型，不把 135×240 / 170×320 framebuffer 拉成横屏比例。画面来自运行时纹理，照片中的固件屏幕没有烘焙到模型。T-Deck 仅键盘与轨迹球饰面使用官方照片裁切，其裁切、投影和局限记录在计划中。

T-Embed 的 PCB 标为 `UNKNOWN`，没有虚构走线或内部元件；小号重复标识位于主板边缘以便在透明材质下辨识。外壳金色/半透明紫色检查不改变按键、屏幕、旋钮和背垫材质。裸板三个型号没有把 PCB 错当成可改色外壳。

## 来源和权利

- [TTGO-T-Display](https://github.com/Xinyuan-LilyGO/TTGO-T-Display/tree/6f89cddcde7152b84f7afc1831b6a1dee1fc3206/3d_file) 与 [T-Display-S3](https://github.com/Xinyuan-LilyGO/T-Display-S3/tree/1cc41eb1c76b2f3215130a3dd64e7138ce00b482/dimensions) 保留仓库 MIT 许可原文。
- [T-Deck](https://github.com/Xinyuan-LilyGO/T-Deck/tree/2027dee79198af949a9e14e1a09e301f993fc7cf/shell) 与 [T-Embed CC1101](https://github.com/Xinyuan-LilyGO/T-Embed-CC1101/tree/c36826e0f91311cb591c4545fccf91e254fc55dc/3D_files) 的 CAD 由厂家公开，但本次未找到明确根许可；不能称为已取得开放再授权，也不套用本项目 MIT。各目录的 `SOURCE-RIGHTS.txt` 保留这一状态。
- 官方照片版权归 LILYGO。未核实独立开放图片许可；仅作为产品参考及明确标注的局部饰面来源，不重标为项目原创或 MIT。
- T-Beam V1.2 和 T-Watch S3 暂无本次可核对版次的模型；没有把 T-Beam Supreme / 旧款 T-Watch 壳体当作替代。

## 验证和复现

`evidence/<id>/verification.json` 记录最终模型/几何哈希、逐零件导出误差、照片来源和相机。导出使用原始单位及刚体变换，浮点导出误差小于 0.05 mm；这个检查证明源几何保真，不证明制造尺寸准确。

四款均在实际通用加载器中完成 headless Chromium 正面、背面、侧面检查。每款保留原图裁切、统一缩放后的并排图与 50% 叠图；对齐未经镜头/透视标定，**不是 1:1 验收通过**。屏幕、键帽、端口、材料和官网/CAD尺寸差异保留在报告中。T-Deck 的照片饰面不作为自己的独立验证依据。T-Embed 另有原色、金色、紫色及 UNKNOWN 内层截图。

工具不联网，从已保留并校验哈希的源文件重建几何、纹理和元信息；不要覆盖源目录：

```sh
python3 tools/build-lilygo-models.py --output /tmp/lilygo-model-rebuild
```

依赖见 `tools/requirements-model.txt`。四款均已实际重建并逐字节比较模型和生成资产；所有来源/纹理哈希、二进制零件范围有效。`node --test tools/platform-manufacturer-model-test.mjs` 通过。固件输出、分享/mini 以及实际按键行为由产品集成另行验证；本报告不声称实机验证、逐键键盘适配或线缆插合验证。
