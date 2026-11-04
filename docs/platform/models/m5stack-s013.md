# Stamp-P4（S013）模型与来源

保留 M5Stack 官方 KiCad footprint 内嵌的 `StampP4_PCB_main_V0.3_20251126.step`，固定 M5_Hardware 提交 `e29c139cbc7c80b5ccc5557e5ab8025a7a19a109`。原文件 SHA-256 为 `31606e5de694e6ca57cea1eee7c0cde57b8fb85eb2fe9ba252e03ac6068353aa`；容器与 KiCad V1 MurmurHash 提取凭据见 [知识记录](../../../web/platform/knowledge/m5stack-s013.json)。CAD 版次与用户硬件版次分别记录。

全部 213 个实体及 3 个开放 USB 曲面均保留。原始 272,368 个三角面的坐标和法向按逐面哈希多重集严格相等；仅把 PCB 背面分成独立材质组，得到 217 个渲染组。真实 PCB 没有被空白板或 UNKNOWN 替换。整个装配只平移居中，不缩放。

[模型计划](../../../web/platform/assets/m5stack-s013/model-plan.json) 记录全部变换、选面、纹理透视四角与 alpha 遮罩。官方正反面照片确认 PCB 为近黑色；原 STEP 的绿色保留在 `sourceStyle` 作为原始属性。正面照片避开所有已建模立体元件的投影包围盒，背面避开 USB 凸出片和孔套；板边与通孔继续由实际 CAD 裁切。ESP32-P4 封装文字投射在原有芯片平面上。BTB 黑色绝缘体、金属端子和 USB 壳体分别着色。照片光照与材质参数未作色彩标定。

官方照片显示一个黄铜色孔套，源 STEP 只保留了原生 `(4,-4)` 毫米位置的 R1 通孔。补充前后各一个有通孔的薄环，稳定 ID 为 `mounting-collar-front/back`，共同对应同一个孔套。外径 2.6、内径 1.2、可见厚度 0.1 毫米均为照片估算；中心取 CAD，隐藏筒身、螺纹和零件型号未知。照片未装螺丝，不按孔数补猜螺丝。没有独立纸贴平面。

最终 [验证记录](../evidence/m5stack-s013/verification.json)：

| 项目 | 结果 |
| --- | --- |
| CAD 外包尺寸 | 22.000 × 29.77994 × 4.26000 mm |
| 官方尺寸 | 22.0 × 29.8 × 4.3 mm；各轴误差 < 0.05 mm |
| 正面轮廓 | IoU 0.98384，通过 |
| 背面轮廓 | IoU 0.97805，通过 |
| 侧面 | IoU 0.60274，仅定性对照 |
| 原色、金色、半透明紫色 | 每个视角逐像素相同；裸板无可改色外壳 |

[浏览器检查](../evidence/m5stack-s013/browser-review.json) 记录最终资产与对照图哈希。原厂色的正、反、侧、斜视与孔洞已检查；未把轮廓通过扩展为内部元件尺寸或电气行为通过。固定依赖见 `tools/requirements-model.txt`，复现命令为 `python tools/rebuild-product-model.py build m5stack-s013 --scratch <fresh-scratch>`；[离线凭据](../evidence/m5stack-s013/offline-rebuild.json) 对应最终资产。

模型状态为 partial；照片与 CAD 可能有版次差异。此记录只验证机械和表面资料，固件运行范围由单独的产品运行记录说明。
