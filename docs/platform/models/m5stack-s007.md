# M5Stack Stamp-S3 · S007

S007 采用原厂 STL 外盖和 KiCad STEP 机械装配。保留 29 个部件：原 STEP 中 28 个非重复实体，加单独发布的 STL 外盖；补回了铜柱和 USB 细节。只排除 STEP 中与 STL 重叠的另一版外盖，全部部件沿用原始毫米坐标及统一 Z 平移 −1.3 mm，不缩放。

[官方文档](https://docs.m5stack.com/en/core/Stamp-S3)和无排针的正面照片确认，S007 默认外观没有焊接附送排针，也没有贴附送的引脚贴纸。照片中的黑色 PCB 背面经过明确四角校正，只覆盖实际 CAD 板背面；孔洞、板边及铜柱独立处理。原图为 800×800，板面有效区域约 365×485，没有虚构高分辨率。尝试取得商店高清资源清单的 HTTP 429 失败回执已保留。

未知内部用实际 PCB 内侧的小型 `UNKNOWN` 标记表示；“分开查看”只按明确组定义将外盖、M2 螺丝和相应点击面沿 Z 轴移开 10 mm。铜柱和 PCB 留在原位置。此偏移是查看安排，不是已验证的拆卸路径。盖上 G0 点击面沿用实际盖帽位置，固件行为见独立运行证据。

| 核对项目 | 结果 |
| --- | --- |
| 原始机械部件 | 29 项尺寸误差均小于 0.000001 mm |
| 宽度 | 18.000023 mm，官方目标 18 mm |
| 含 USB 长度 | 25.947933 mm；官方 24 mm 目标仍记为失败，不能用缩放掩盖范围差异 |
| 厚度 | 4.750002 mm，官方目标 4.7 mm，0.1 mm 容差内 |
| 正面照片轮廓 | IoU 0.984708，通过 0.95 门槛 |
| 背面照片轮廓 | IoU 0.985757，通过 0.95 门槛 |
| 原色、金色、半透明紫色 | PCB、USB、天线、螺丝与铜柱材质保持独立 |
| 离线重建 | 13 个生成资产及模型配置一致，model.json 字节一致 |

照片裁图等比例放入 800×800 对照画布；没有拉伸照片，也没有改变 CAD。斜视仅做定性检查，不把相机差异算成制造精度。背面照片同时用作纹理，外观对照不能独立证明电路布局。完整报告见 [verification.json](../evidence/m5stack-s007/verification.json)，原失败记录保留在 [history](../evidence/m5stack-s007/history/2026-10-26/verification.json)。

模型状态仍为 **partial**。厂家机械 CAD 没有完整的已识别电子元件和精确镀层；浅刻字的可见性受视角、照明影响。源螺丝缺少六角凹槽，新增薄六角视觉标记仅表达官方照片中的外观，不声称已建出可加工凹孔。铜柱底部细节与照片仍有差异。照片颜色不是经校准的材料反射率。

固定来源为 M5_Hardware `e29c139cbc7c80b5ccc5557e5ab8025a7a19a109`，源 URL、UTC 和 SHA-256 在 [source-audit.json](../evidence/m5stack-s007/source-audit.json) 与 [knowledge](../../../web/platform/knowledge/m5stack-s007.json)。厂家 CAD 的 MIT 许可与照片版权分开保留。几何验证不证明 CPU、外设或固件执行。

从仓库根目录重建，依赖 `tools/requirements-model.txt`：

```sh
python tools/rebuild-product-model.py plan m5stack-s007
python tools/rebuild-product-model.py build m5stack-s007 --scratch <fresh-directory>
```

照片复核：通过受跟踪的 Playwright wrapper 在任务专用 headless 会话打开 `web/platform/review-lane-02.html?id=m5stack-s007`，然后执行：

```sh
python docs/platform/evidence/stamp-s3-model-review.py capture m5stack-s007 --session <task-session>
python docs/platform/evidence/stamp-s3-model-review.py verify m5stack-s007
```

完成后用同一 wrapper 和 session 名称关闭浏览器。相机、纹理遮罩和分组均来自 [model-plan.json](../../../web/platform/assets/m5stack-s007/model-plan.json)。
