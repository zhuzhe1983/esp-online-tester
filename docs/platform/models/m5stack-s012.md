# Stamp C6LoRa S012 模型核验

保留厂家已装配 PCB、元件、屏蔽罩和两只 IPEX-4 天线座。原始 500 个装配组件、27 个开放壳面均保留；为天线座分配不同材质后得到 504 个渲染组，228,335 个三角面的坐标和法向多重集合完全一致，没有缩放或补猜内部电路。

来源固定于 M5_Hardware `e29c139cbc7c80b5ccc5557e5ab8025a7a19a109`，STEP SHA-256 为 `d8072cec857ea5ea45557255601a3d6b11f11c6bc5b638128c5bff857f71c335`；本次核对当前厂家结构目录，文件 Git blob 仍为 `92c04cef3f871d13639a4f81bb23cde9228eca6d`。[官方产品页](https://docs.m5stack.com/en/core/Stamp_C6LoRa)、[知识记录](../../../web/platform/knowledge/m5stack-s012.json)和[来源清单](../evidence/m5stack-s012/source-audit.json)保留逐源 URL、时间、大小及哈希。

屏蔽罩顶部标识取自官方正视照片，只投射到原 CAD 的六个平面三角面；没有额外纸贴，也不把侧壁或天线接口染成图案。背面裸露 PCB 的照片投射到原有板面，避开已有立体铜焊盘。PCB 为近黑色，天线座按照片分为黑色绝缘体、金色接触环/中心针和银色焊接端。颜色与照片光照未做实物标定，不能作为光学材料测量值。裁切、遮罩、坐标和完整面片分区均写入 [model-plan.json](../../../web/platform/assets/m5stack-s012/model-plan.json)。

离体圆柱属于 STEP 的 `Free-Models/11133974320/Cylinder`，半径 2.54 mm、高 1.27 mm，只有圆柱侧面及两个端面，无孔或螺纹，位于主板范围外约 159 mm。官方正、反、侧视照片没有对应紧固件，也没有可确认的装配位置，因此继续排除，不作为螺柱移入主板。原始坐标和实体引用保存在[排除记录](../evidence/m5stack-s012/source-component-exclusion.json)。本型号已有真实 PCB CAD，因此没有替换为空白 UNKNOWN 占位板。

最终文件通过离线重建，8 个必需资产及模型配置一致。实测包围盒约 18 × 15 × 2.26035 mm，对应官方 18 × 15 × 2.3 mm，三项均在 0.1 mm 容差内。浏览器清空缓存后检查了正反面、侧视和近距离斜视，原色、金色和半透明紫色外壳预设下，PCB、屏蔽罩标识和天线座保持逐像素一致；本裸模块没有可改色外壳网格。

| 对照 | 结果 |
| --- | --- |
| 正面轮廓 | IoU 0.9891，通过 |
| 背面轮廓 | IoU 0.9838，通过 |
| 侧面轮廓 | IoU 0.7683，仅作定性对照 |
| 材质与标识 | 标识留在平顶，焊盘/天线座独立，已检查近景 |

[浏览器回执](../evidence/m5stack-s012/browser-review.json)、[尺寸与照片结果](../evidence/m5stack-s012/verification.json)、[离线重建回执](../evidence/m5stack-s012/offline-rebuild.json)对应本次最终资产哈希。侧视姿态、反光和细分误差仍保留，整体状态为 partial；这里没有进行客体固件、射频或实机插接验收。

```sh
python tools/rebuild-product-model.py plan m5stack-s012
python tools/rebuild-product-model.py build m5stack-s012 --scratch <fresh-scratch>
```
