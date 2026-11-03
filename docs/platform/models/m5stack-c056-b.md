# Stamp-C3（C056-B）模型核验

模型展示五件包装中的一块 Stamp-C3，保留厂家外盖的 4,172 个原始 STL 三角面；没有按包装数量复制装配，也没有把可选排针预装到板上。[官方文档](https://docs.m5stack.com/en/core/Stamp-C3)与固定源提交 `e29c139cbc7c80b5ccc5557e5ab8025a7a19a109` 的来源、时间和 SHA-256 见[知识记录](../../../web/platform/knowledge/m5stack-c056-b.json)及[来源审计](../evidence/m5stack-c056-b/source-audit.json)。

PCB 的外露背面采用同 SKU 官方照片，按四个针孔中心和 2.54 mm 间距校正透视，另用中心孔核对，误差约 0.15 mm。原图为 800×800 多产品照片，板子只占约两百多像素；输出贴图不会补出原图没有的细节。黑色板材、通孔及板边有独立几何，照片不延伸到孔壁、侧壁或 USB 插口。盖内正面布局仍然未知，保留 `UNKNOWN`，可通过分开展示查看。照片外观不等于 PCB 布局或电气验证。

中心轴按厂家 CAD 和尺寸图改为 `[0,2] mm`，螺丝头、六角凹孔、黄铜螺柱和贴纸孔共用该轴。官方注明一枚 M2×4 内六角沉头螺丝；照片确认一枚黄铜支撑。模型补齐可见部分，螺柱高度和可见头部尺寸属于照片估算，隐藏螺纹与安装结构未建模。耐高温纸贴是独立薄片，承托印刷贴图并跨越实际外盖开口；有效纸边在外盖边界内，中心孔不覆盖螺丝。

| 检查 | 结果 |
| --- | --- |
| 厂家外盖包围盒 | 20×34×4.5 mm，保持原值；源三角面误差 0 mm |
| 独立产品尺寸图 | 20×34×4.6 mm；板厚 1.0 mm，未把两种厚度强行拉成一致 |
| 正面照片轮廓 | IoU 0.989164，通过 0.95 门槛；不代表内部结构或贴图独立验收 |
| 背面孔位 | 四个端部针孔校准；中心孔独立误差约 0.15 mm，小于 0.3 mm 照片估算容差 |
| 纸边、中心硬件、支撑 | 原色、金色、紫透及斜侧近景复核，见[外观对照](../evidence/m5stack-c056-b/appearance-review.webp) |
| 原文件离线重建 | 13 个资源 hash 一致，几何与模型契约一致 |

[机械记录](../evidence/m5stack-c056-b/mechanical-refresh.json)保留估算值、旧验证失败及来源身份冲突。官方 C3 的 PcbDoc 链接实际包含一个 `M5STAMP-C3U_SMD_ONLY` footprint，且无实体或内嵌模型；KiCad footprint 也没有三维模型，因此没有将它们冒充完整 C3 板级 CAD。正面、背面、拆盖未知面及支撑近景见[验证记录](../evidence/m5stack-c056-b/verification.json)和[背面对照](../evidence/m5stack-c056-b/back-comparison.webp)。斜视仅作定性检查，未宣称完整 1:1 验收。

重建使用仓库中的固定计划，不依赖私有缓存：

```sh
python tools/rebuild-product-model.py plan m5stack-c056-b
python tools/rebuild-product-model.py build m5stack-c056-b --scratch /tmp/c056-model-rebuild
node --test tools/platform-knowledge-test.mjs tools/platform-manufacturer-model-test.mjs tools/platform-plate-geometry-test.mjs
```

厂家 CAD 保留其 MIT 许可。厂家照片及衍生纹理没有已核实的独立开放许可，不套用项目 MIT。模型状态仍为 `partial`；本记录只验外观与几何，固件执行结论由板级运行时记录独立提供。

G3 用户键采用源 CAD 弹片内侧 9 × 6 mm 命中区，位置 [0, -4.5, 2.285] mm，绑定运行时 `button`。按键面随外盖一起分开查看，EN 复位不绑定为 GPIO 用户键。实际 3D 点击与客体输出另见交互回执。
