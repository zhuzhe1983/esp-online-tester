# CoreMatrix 模型验证回执

`source-audit.json` 保留实际获取的公开来源、固定 CAD 提交，以及[完整模型计划](../../../../web/platform/assets/m5stack-c158/model-plan.json)的路径和哈希；不重复存储三角形分组。`verification.json` 绑定最终模型、原始几何、浏览器检查、照片相机与输出哈希。

六张 `*-comparison.webp` 按顺序展示独立官方照片、生产模型渲染、50% 叠图和轮廓差异。只根据独立照片中的机身宽度与官方 54 mm 尺寸设定相机；没有缩放网格或变形照片。红色表示只在照片轮廓中出现，蓝色表示只在模型轮廓中出现。轮廓范围不包含灯光、文字准确度或内部器件验证。

`browser/geometry.json` 来自生产模型加载器，检查完整装配尺寸、四个可见按键、256 个独立窗口、改色隔离和展开复位。原始 PNG 留在忽略的 `output/corematrix-model-evidence/`，其哈希见 `browser/captures.json`，可按模型检查脚本重新生成。LED 的全亮与四角不同强度是显式视觉探针，不是固件执行证据。材质汇总为 `material-review.webp`，其中 inspection 显示完整装配旁的 UNKNOWN 板面，没有隐藏零件来制造“可读”结果。压缩预览不用于重新计算轮廓；[整理记录](curation.json)保留原始及整理后哈希，几何与 IoU 数值未改动。

`rebuild.json` 为 `tools/rebuild-product-model.py build m5stack-c158` 的离线输出：17 个固定来源/几何/纹理资产及模型字段一致，`model.json` 逐字节一致。

完整边界与复现方法见 [模型说明](../../models/m5stack-c158.md)。没有实物测试、板级制造 CAD或光度测量；因此总状态保留 `partial`。公开回执只包含来源、命令、数值、哈希与产品图像，没有保留浏览器会话、机器用户名或进程列表。
