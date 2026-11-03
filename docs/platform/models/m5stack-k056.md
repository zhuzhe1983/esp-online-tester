# Stamp-C3 Mate（K056）模型核验

K056 是包含一块 Stamp-C3 的套件。[官方说明](https://docs.m5stack.com/en/core/Stamp-C3_Mate)把模块、12P/10P 公母排针、HY2.0 插座和扳手分列；模型保留模块本体，散件没有被推断为已焊接。厂家 K056 外盖 STL 与 C056-B 外盖逐字节一致，但仍保留 K056 自己的来源 URL、4,172 个原始三角面和毫米比例。

K056 的耐高温纸贴继续使用其独立官方照片，另有薄纸片承托；有效边界位于外盖内，中心孔遮罩避开螺丝。中央轴由 CAD 和独立尺寸图共同确定为 `[0,2] mm`，可见内六角沉头螺丝和黄铜螺柱与孔同心。隐藏螺纹、内部固定结构和照片无法确认的元件布局保持未知。

黑色 PCB 背面照片明确来自套件所含的 Stamp-C3 模块，原来源是 C056-B 图库。K056 和 C056-B 的裸板照片及外盖文件 hash 相同；这项复用不把 C056-B 包装图冒充 K056。背面按四个端部针孔和官方 2.54 mm 间距校正，中心孔独立误差约 0.15 mm。原图内板子只有约两百多像素，放大不会增加细节。正面布局保留 `UNKNOWN`，可在公开页面“分开查看”后旋转查看。

| 检查 | 结果 |
| --- | --- |
| 厂家外盖 | 20×34×4.5 mm；4,172 个三角面，源误差 0 mm |
| 独立产品图 | 20×34×4.6 mm、1.0 mm 板厚；原 CAD 不缩放 |
| 正面外形 | IoU 0.987673，通过 0.95 门槛；不代表内部 CAD 完整 |
| 纸边、孔洞与硬件 | 原色、金色、紫透、侧视和拆开展示复核 |
| 公开检查入口 | 分开查看、旋转、恢复视角、反复切换通过；局部位置十次恢复无漂移 |
| 离线重建 | 13 个资源 hash 及模型 JSON 完全一致 |

[来源审计](../evidence/m5stack-k056/source-audit.json)、[机械记录](../evidence/m5stack-k056/mechanical-refresh.json)、[验证记录](../evidence/m5stack-k056/verification.json)和[外观对照](../evidence/m5stack-k056/appearance-review.webp)保留来源身份、照片误差与当前限制。官方链接的 LCEDA 元件页返回 HTTP 418，没有核实到完整板级实体，故未宣称有完整 PCB CAD。旧按键记录缺少名称造成公开页面切换报错，已最小补充 `Button`，保留原控件 ID，后续运行时映射单独迁移。

```sh
python tools/rebuild-product-model.py plan m5stack-k056
python tools/rebuild-product-model.py build m5stack-k056 --scratch /tmp/k056-model-rebuild
node --test tools/platform-knowledge-test.mjs tools/platform-manufacturer-model-test.mjs tools/platform-plate-geometry-test.mjs tools/platform-model-inspection-test.mjs
```

模型仍为 `partial`。厂家 CAD 保留其 MIT 许可；照片和衍生贴图没有已核实的独立开放许可，不套用项目 MIT。此处只验证外观、几何和查看交互，不作为固件执行证据。
