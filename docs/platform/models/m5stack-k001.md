# M5Stack BASIC · K001

经典 ESP32 主控套件，配 2 英寸彩屏、三个实体按键、扬声器与电池底座；以当前 K001 官方资料为准。

厂家 CAD 以毫米和刚体变换导入；本产品外观状态仍为 **partial**，本次只核验外观，固件运行支持以产品库为准。

装配：Source glass and LCD surfaces are separated3.294mm above the frame lip. A local rigid -3.2940483mm source-Y translation seats their lower face at frame top8.3mm; keys move -1mm source-Y to leave approximately0.48mm cap protrusion. These seating offsets are photo/CAD-estimated assembly, not manufacturer assembly constraints; raw source and transforms retained. Main frame geometry is unscaled.

已检查正面、背面和侧面，以及原色/金色/半透明紫色正背对照。纸贴使用源照片的显式透明遮罩，独立于可换色外壳；照片不覆盖屏幕。缺失的字样、端子细节和内部板图不因本次检查而成为已还原。

几何检查共 9 项，1 项未通过；数值和照片轮廓测试的失败均保留在 [verification.json](../evidence/m5stack-k001/verification.json)。三种材质对照见 [appearance-comparison.webp](../evidence/m5stack-k001/appearance-comparison.webp)。侧面及斜拍只作定性核查，白底白壳、标注图和附带物可能使自动轮廓分割无效，不能用该指标证明整机1:1。

来源、抓取时间、hash与装配计划见 [source-audit.json](../evidence/m5stack-k001/source-audit.json)。复现：`python tools/onboard-lane-02-extra.py prepare --ids m5stack-k001`、`build --ids m5stack-k001`，然后使用 `review-lane-02.html?id=m5stack-k001` 截取对应正背侧视，运行 `verify --ids m5stack-k001`。依赖见 `tools/requirements-model.txt`。

限制：

- Disconnected zero-thickness LCD edge triangles duplicated in the source export are omitted; two complete source LCD faces and outer panel are retained.
- Runtime board/peripherals are not implemented for this SKU; geometry preview does not establish firmware compatibility.
- Photo and geometry comparisons remain partial; no complete 1:1 manufacturing or populated-PCB certification.
- Source CAD MIT and manufacturer photographs have distinct rights; no separate open photo license verified.
- Blank display rectangle uses documented active dimensions when available; placement/window boundary is estimated from photos/CAD, not a calibrated measurement.
- Source file contains upper core frame/glass/keys; matching bottom battery base geometry is absent. Reassembled upper core is shallower than the17mm complete-product specification, a retained dimension failure.
- Glass/key seating is an explicitly estimated rigid assembly. Port mouths are photo-derived and no physical fit is certified.
- Back photograph includes missing battery base, so it is not a like-for-like silhouette comparison. Side comparison shows separated base and cannot certify assembled thickness.

## 原代 Basic 背部外观补全边界

本型号官方 CAD 仍缺电池底座。本次按 K001 自身正反/侧照、54×54 mm 外形与官方 17 mm 整机厚度，增加独立的底部外形薄层估算，并从 K001 自身背照恢复四条针脚纸色。原生厂家几何和装配变换不变；该近似外形没有补猜电池、PCB、螺钉、磁铁、LEGO 孔或端口结构，也不把原 CAD 覆盖/尺寸失败变成通过。UNKNOWN 保留，透明紫色下可读。

[最终回执与 CPU 原色/金色/紫透背面和近侧视](../evidence/m5-rear-paper-20261126/m5stack-k001/receipt.json) 记录来源、估算和最终哈希。纸贴逐字节离线重建通过，144,545 个有效 alpha 纹素均由估算底面承载，间隙 0.025 mm。完整通用重建仍存在二进制排序、PNG 编码和纹理记录差异；原生三角形位置与法向经明确量化、排序后一致，完整字节比较仍记失败。未声称校准照片或实物验证。
