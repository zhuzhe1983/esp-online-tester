# M5Stack Station-Bat · K124-B

ESP32工业控制终端，配1.14英寸LCD、三个用户按键、六路Grove扩展与双18650并联电池供电。

厂家 CAD 以毫米和刚体变换导入；本产品外观状态仍为 **partial**，本次只核验外观，固件运行支持以产品库为准。

装配：Selected components already share assembled coordinates near X=0; only common center translation applied. Distant X components are printing-layout copies. Independent drawing gives 56x88mm main housing, 65mm including side rails, 92mm including rear hook, 35mm overall depth. Product specifications use 88x65x35, so distinct measurement scopes are retained without geometry fitting.

已检查正面、背面和侧面，以及原色/金色/半透明紫色正背对照。纸贴使用源照片的显式透明遮罩，独立于可换色外壳；照片不覆盖屏幕。缺失的字样、端子细节和内部板图不因本次检查而成为已还原。

几何检查共 16 项，0 项未通过；数值和照片轮廓测试的失败均保留在 [verification.json](../evidence/m5stack-k124-b/verification.json)。三种材质对照见 [appearance-comparison.webp](../evidence/m5stack-k124-b/appearance-comparison.webp)。侧面及斜拍只作定性核查，白底白壳、标注图和附带物可能使自动轮廓分割无效，不能用该指标证明整机1:1。

来源、抓取时间、hash与装配计划见 [source-audit.json](../evidence/m5stack-k124-b/source-audit.json)。复现：`python tools/onboard-lane-02.py prepare --ids m5stack-k124-b`、`build --ids m5stack-k124-b`，然后使用 `review-lane-02.html?id=m5stack-k124-b` 截取对应正背侧视，运行 `verify --ids m5stack-k124-b`。依赖见 `tools/requirements-model.txt`。

限制：

- Manufacturer exterior CAD only; CPU, display, touch, radio and board-level firmware behavior are unavailable for this product.
- Materials are photograph-informed display approximations, not calibrated manufacturer material data.
- PCB UNKNOWN is an estimated blank placeholder, excluded from internal 1:1 claims; no inferred chips, copper or manufacturing holes.
- No physical device measurements or hardware flashing performed.
- Exposed USB/Grove contact detail and fasteners are not complete in the source CAD; no invented accurate component geometry is claimed.
- Official product description says MPU6886; one specification row spells MPU6686. The conflicting row is retained in knowledge with a note.
- A board photograph is available but has not been calibrated as a board layout; no populated PCB reconstruction is claimed.
