# M5Stack Capsule · K129

内置 Stamp-S3、250 mAh 电池、BMI270、RTC、麦克风与红外发射的便携采集设备。

厂家 CAD 以毫米和刚体变换导入；本产品外观状态仍为 **partial**，本次只核验外观，固件运行支持以产品库为准。

装配：Native assembled STL cluster rotated180degrees about Z and centered by rigid translation; no scale. This places USB/Grove on -Y and wake-button end on +Y, matching the exact product photographs.

已检查正面、背面和侧面，以及原色/金色/半透明紫色正背对照。纸贴使用源照片的显式透明遮罩，独立于可换色外壳；照片不覆盖屏幕。缺失的字样、端子细节和内部板图不因本次检查而成为已还原。

几何检查共 6 项，0 项未通过；数值和照片轮廓测试的失败均保留在 [verification.json](../evidence/m5stack-k129/verification.json)。三种材质对照见 [appearance-comparison.webp](../evidence/m5stack-k129/appearance-comparison.webp)。侧面及斜拍只作定性核查，白底白壳、标注图和附带物可能使自动轮廓分割无效，不能用该指标证明整机1:1。

来源、抓取时间、hash与装配计划见 [source-audit.json](../evidence/m5stack-k129/source-audit.json)。复现：`python tools/onboard-lane-02-extra.py prepare --ids m5stack-k129`、`build --ids m5stack-k129`，然后使用 `review-lane-02.html?id=m5stack-k129` 截取对应正背侧视，运行 `verify --ids m5stack-k129`。依赖见 `tools/requirements-model.txt`。

限制：

- Stamp-S3 is retained at its source assembly position; user-fitted headers and example external peripherals in gallery photographs are not part of the base product.
- No screen is present; gallery example OLED/LED peripherals are excluded.
- Runtime board/peripherals are not implemented for this SKU; geometry preview does not establish firmware compatibility.
- Photo and geometry comparisons remain partial; no complete 1:1 manufacturing or populated-PCB certification.
- Source CAD MIT and manufacturer photographs have distinct rights; no separate open photo license verified.
- Official photographs include unlabelled Stamp cover, separately fitted labelled module and accessories. No sibling label is substituted. USB/Grove external position is photo/CAD estimated; cable remains disabled without verified pin-order/mating evidence.
- Manufacturer cover interior ribs are visible in native CAD; no speculative flip or invented populated StampPCB is applied. Source and camera pose differences remain visible in the comparison.
