# CoreMatrix 模型验证回执

`source-audit.json` 保留实际获取的公开来源、固定 CAD 提交、完整三角形分组及确定性的模型计划。`verification.json` 绑定最终模型、原始几何、浏览器检查、照片相机与全部输出哈希。

六张 `*-comparison.webp` 按顺序展示独立官方照片、生产模型渲染、50% 叠图和轮廓差异。只根据独立照片中的机身宽度与官方 54 mm 尺寸设定相机；没有缩放网格或变形照片。红色表示只在照片轮廓中出现，蓝色表示只在模型轮廓中出现。轮廓范围不包含灯光、文字准确度或内部器件验证。

`browser/geometry.json` 与对应 PNG 来自生产模型加载器，检查完整装配尺寸、四个可见按键、256 个独立窗口、改色隔离和展开复位。LED 的全亮与四角不同强度是显式视觉探针，不是固件执行证据。材质汇总为 `material-review.webp`；`browser/inspection.png` 显示完整装配旁的 UNKNOWN 板面，没有隐藏零件来制造“可读”结果。

`rebuild.json` 为 `tools/rebuild-product-model.py build m5stack-c158` 的离线输出：17 个固定来源/几何/纹理资产及模型字段一致，`model.json` 逐字节一致。

完整边界与复现方法见 [模型说明](../../models/m5stack-c158.md)。没有实物测试、板级制造 CAD或光度测量；因此总状态保留 `partial`。公开回执只包含来源、命令、数值、哈希与产品图像，没有保留浏览器会话、机器用户名或进程列表。
