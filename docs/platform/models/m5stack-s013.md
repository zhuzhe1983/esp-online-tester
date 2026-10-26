# M5Stack Stamp-P4 · S013

ESP32-P4 微型模块，16MB Flash、32MB PSRAM，提供 USB、MIPI 与扩展引脚；此模型使用官方 footprint 内嵌的板级 STEP，P4 运行内核尚未接入。

厂家 CAD 以毫米和刚体变换导入；本产品外观状态仍为 **partial**，本次只核验外观，固件运行支持以产品库为准。

装配：Official Stamp-P4 KiCad footprint contains a base64+zstd STEP payload. Container receipt, KiCad V1 MurmurHash checksum and decompressed SHA-256 are retained. All213 solids and3 otherwise-orphaned USB shells are included in source-native millimetres. Translation only centers the assembly; no geometry scaling. STEP component reference labels/board layout belong to CAD V0.3 dated2025-11-26, not an inferred user hardware revision.

已检查正面、背面和侧面，以及原色/金色/半透明紫色正背对照。纸贴使用源照片的显式透明遮罩，独立于可换色外壳；照片不覆盖屏幕。缺失的字样、端子细节和内部板图不因本次检查而成为已还原。

几何检查共 219 项，0 项未通过；数值和照片轮廓测试的失败均保留在 [verification.json](../evidence/m5stack-s013/verification.json)。三种材质对照见 [appearance-comparison.webp](../evidence/m5stack-s013/appearance-comparison.webp)。侧面及斜拍只作定性核查，白底白壳、标注图和附带物可能使自动轮廓分割无效，不能用该指标证明整机1:1。

来源、抓取时间、hash与装配计划见 [source-audit.json](../evidence/m5stack-s013/source-audit.json)。复现：`python tools/onboard-lane-02-extra.py prepare --ids m5stack-s013`、`build --ids m5stack-s013`，然后使用 `review-lane-02.html?id=m5stack-s013` 截取对应正背侧视，运行 `verify --ids m5stack-s013`。依赖见 `tools/requirements-model.txt`。

限制：

- Runtime ESP32-P4/board model is not implemented; geometry and firmware compatibility are independent.
- Manufacturer board geometry is available, so no invented UNKNOWN board replaces it. Exact circuitry behavior, copper stackup, silkscreen completeness are not certified.
- No shell exists on this bare module. Shell recoloring is inapplicable; board/component materials remain separate.
- OpenCascade tessellation .05mm linear/.2rad angular is approximate; raw sourceSTEP remains authoritative.
- Official photos may represent later revisions; user hardware revision remains unknown.
- Manufacturer CAD retains its repository license; official product photographs are not relicensed under projectMIT.
- No physical measurement or flashing performed.
- XCAF source colors are applied to the33 components with a native color. The remaining183 components use neutral display colors; these are not measured or claimed as manufacturer material colors.75 source names are retained separately from stable renderer IDs.
