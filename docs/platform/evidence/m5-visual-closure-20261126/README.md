# M5 模型候选闭合矩阵

本矩阵只覆盖原始 `visual-review.json` 的 **35 个候选，加后续 C008，共 36 个 SKU、35 个独立模型文件**。K148 显式复用 C148。U215 不是原始候选，不计入分母。

**最终实际树已通过**：`integrated-check.json` 绑定稳定 Root `28bd36532d3f732ddad100146b4bc6d6d3c58a7e`，完整 36/36，错误和待处理项均为零；矩阵 SHA-256 为 `83e491fbefb8dd1ca050b9059a35b91d6fca678dc220318e375b622051a00590`。这是对模型、plan、artifact、来源证据及保留图像实际字节的独立检查，贡献快照结果另存。

贡献快照已逐项核对最终模型、重建 plan、全部 artifact、CPU 证据及保留 review 图的实际 SHA-256：32 项有证可见面修复，K148 共享宿主复核 1 项，Faces 排除误判 1 项，COMM / SwitchC6 源不足保留 UNKNOWN 2 项。修复和保留均有具体边界，不代表隐藏 PCB、机械配合或全部外观已达到 1:1。

`matrix.json` 固定原始筛查输入及各项贡献 commit。`contribution-check.json` 只证明这些贡献快照；**它不声称最终整合树已经包含这些资产**。最终树须另跑默认模式，模型、plan、catalog URL、artifact、proof 和 retained review 图有一项改变即失败；缺少未合入证据也会失败。

```bash
# 有完整贡献对象的 Git 仓库：只读快照检查
python tools/verify-m5-visual-closure.py --snapshot --source-root /path/to/git-checkout --output /tmp/m5-contributions.json

# 所有最终批次合入并稳定后：严格读取实际整合树
python tools/verify-m5-visual-closure.py --source-root /path/to/final-checkout --output /tmp/m5-integrated-visual-closure.json

# 正反边界；不更改任何模型，不执行 Cargo 或渲染
python tools/test-m5-visual-closure.py --source-root /path/to/git-checkout --output /tmp/m5-closure-controls.json
```

`--ids` 只做选定子集，并在回执中明确标为子集；仍校验完整 36 项分母，不能冒充全表通过。默认实际树模式不依赖原始贡献 commit 在未来主分支克隆中可达，验证的是文件本身与已固定证据；快照模式则实际读取并校验贡献 commit 对象。

K151 最终模型为 `0b97c19c…`，只新增 `presentation.inspectionGroups`。验证器独立删除该项并用指定 JSON 编码重建，要求精确回到旧 `7f0dd1c0…`；另外核对实际 loader 的原色、金色、紫色 mesh / UV / index / world transform / material digest 均旧新相同。旧三色 CPU 回执保留旧 hash，新展开 CPU 单独绑定新 hash；不改写旧回执。五块板各有中心和四内角的可达视线，完整展开图中的 power / touch UNKNOWN 可读，不宣称所有标记朝向同一相机。

Faces 的官方 kit-photo-5 证明 Gray/QWERTY 已装配，而 Gamepad、Calculator 和 dock 是旁置套件内容。COMM 修复的是 pin 纸，透明壳内未有可靠布局的元件仍为 UNKNOWN；SwitchC6 的官方裸机图存在多高度器件，不能压成平面 PCB 贴图。Lite 保留自身 CAD，字面保留当前官网图库中与旧 SE 图相同的纸贴，并公开图库 / SKU 冲突。

Tab5、两款 Station、NanoC6 的最终 CPU view hash 与配方有保留，原始 CPU PNG 不在贡献中保留。K131、两款 Cardputer 和 K152 的回执包含最终 CPU comparison 与三材质 loader 检查，不把它们写成另有三组独立三色 CPU 图。所有这些边界都保留在机器矩阵中。

K117 的合入曾出现重复同值 `controlId`，最终恢复原 `f1c03f03…`，不需要视觉等价豁免。验证器现拒绝重复 JSON key。`k117-root030-offline-control.json` 证明 Root 原有通用 builder 已足够完成刻槽及 USB 开孔并逐字节重建；`k117-root030-control-rays.json` 是实际最近按钮五点射线。独立工具补丁只含旧 979 提交中的公共 verifier，不含任何其他 SKU 模型。

| SKU | 批次范围 | 实际处置 | 贡献/处置 commit | 模型 SHA-256 | 最终证据 |
|---|---:|---|---|---|---|
| m5stack-c145 | 028 | 有证可见面已修 | `b69734f96727` | `182d235e87c0` | [proof](../../../../web/platform/assets/m5stack-c145/rear-paper-cpu-review.json) |
| m5stack-k124-b | 028 | 有证可见面已修 | `d00d9dd05e95` | `81e2380d6481` | [proof](../../../../web/platform/assets/m5stack-k124-b/rear-paper-cpu-review.json) |
| m5stack-k123 | 028 | 有证可见面已修 | `0eb8c0976b11` | `02136a47be0b` | [proof](../../../../web/platform/assets/m5stack-k123/rear-paper-cpu-review.json) |
| m5stack-c125 | 029 | 有证可见面已修 | `82dd30645de6` | `f01799051128` | [proof](../../../../web/platform/assets/m5stack-c125/rear-paper-cpu-review.json) |
| m5stack-k131 | 029 | 有证可见面已修 | `63bba31b5dfa` | `4e725f7e47a8` | [proof](../m5-paper-repairs-20261126/m5stack-k131.json) |
| m5stack-k129 | 030 | 有证可见面已修 | `3d5e84bdeaec` | `1379f49e9cbb` | [proof](../m5-open-face-20261126/m5stack-k129/appearance-receipt.json) |
| m5stack-k129-v11 | 030 | 有证可见面已修 | `3d5e84bdeaec` | `7d05777da392` | [proof](../m5-open-face-20261126/m5stack-k129-v11/appearance-receipt.json) |
| m5stack-k152 | 029 | 有证可见面已修 | `23d9fd971b94` | `e8f49c1f90fd` | [proof](../m5-paper-repairs-20261126/m5stack-k152.json) |
| m5stack-k132 | 029 | 有证可见面已修 | `23d9fd971b94` | `c7971934965d` | [proof](../m5-paper-repairs-20261126/m5stack-k132.json) |
| m5stack-k132-v11 | 029 | 有证可见面已修 | `23d9fd971b94` | `4d62c1730a14` | [proof](../m5-paper-repairs-20261126/m5stack-k132-v11.json) |
| m5stack-c126-ext | 030 | 有证可见面已修 | `3d5e84bdeaec` | `498093ce9873` | [proof](../m5-open-face-20261126/m5stack-c126-ext/appearance-receipt.json) |
| m5stack-k125 | 030 | 有证可见面已修 | `f31cd9de21a9` | `992399eeb273` | [proof](../m5-open-face-20261126/m5stack-k125/appearance-receipt.json) |
| m5stack-k010-v11 | 027 | 有证可见面已修 | `1fe3934ff0f3` | `3d33cc3279e6` | [proof](../m5-rear-paper-20261126/m5stack-k010-v11/receipt.json) |
| m5stack-k128-lite | 032 | 有证可见面已修 | `8d4de303f317` | `8b0c2c168d99` | [proof](../m5-rear-paper-20261126/m5stack-k128-lite/receipt.json) |
| m5stack-c148 | 033 | 有证可见面已修 | `16a62ab5bbbf` | `7a054a376445` | [proof](../m5-classic-appearance-20261126/c148-final/review.json) |
| m5stack-k148 | 033 | 共享宿主独立复核 | `77d1e3cc0e7a` | `7a054a376445` | [proof](../m5stack-k148/review.json) |
| m5stack-k151 | 033 | 有证可见面已修 | `7265e02e8209` | `0b97c19cdf23` | [proof](../m5stack-k151/inspection-20261126/loader.json) |
| m5stack-k005 | 032 | 源证据排除误判 | `995138b42e3d` | `e6b77d361938` | [proof](../m5-faces-layout-review-20261126/receipt.json) |
| m5stack-k001-v27 | 032 | 有证可见面已修 | `5e79730eac87` | `958994943b07` | [proof](../m5-rear-paper-20261126/m5stack-k001-v27/receipt.json) |
| m5stack-k001 | 032 | 有证可见面已修 | `44ed366809b1` | `0e84d0acbe42` | [proof](../m5-rear-paper-20261126/m5stack-k001/receipt.json) |
| m5stack-k162 | 031 | 有证可见面已修 | `871d91623a7e` | `44e950d9d578` | [proof](../m5-visible-face-review-20261126/k162/receipt.json) |
| m5stack-a172 | 031 | 有证可见面已修 | `66af480a1612` | `ee2b3640401c` | [proof](../m5-visible-face-review-20261126/a172/receipt.json) |
| m5stack-k002 | 027 | 有证可见面已修 | `1454dd38eaa7` | `37196035dc34` | [proof](../m5-rear-paper-20261126/m5stack-k002/receipt.json) |
| m5stack-k001-v26 | 027 | 有证可见面已修 | `3348bd90cad8` | `b1b13017d1e6` | [proof](../m5-rear-paper-20261126/m5stack-k001-v26/receipt.json) |
| m5stack-k001-b | 032 | 源不足，保留 UNKNOWN | `3259ec470584` | `e7b7146ed496` | [proof](../m5-rear-paper-20261126/m5stack-k001-b/receipt.json) |
| m5stack-k016-b | 033 | 有证可见面已修 | `7d5e20564f0c` | `cfdc0fc052e5` | [proof](../m5-classic-appearance-20261126/k016-b/review.json) |
| m5stack-k140 | 031 | 源不足，保留 UNKNOWN | `dffbf773617e` | `4f974b93eb3b` | [proof](../m5-control-picking-20261126/k140-button.json) |
| m5stack-k016-p2 | 033 | 有证可见面已修 | `00e2d0dc2caf` | `aadbdb683296` | [proof](../m5-classic-appearance-20261126/k016-p2/review.json) |
| m5stack-k117 | 030 | 有证可见面已修 | `887744859854` | `f1c03f03c649` | [proof](../m5-classic-appearance-20261126/k117/review.json) |
| m5stack-m141 | 031 | 有证可见面已修 | `2a0fab78fc46` | `74ca67bd6e1a` | [proof](../m5-visible-face-review-20261126/m141/receipt.json) |
| m5stack-u017 | 020 | 有证可见面已修 | `10239e9a80f6` | `62b4b0356b79` | [proof](../m5-unit-surface-repairs-20261126/m5stack-u017/checks.json) |
| m5stack-u017-pcba | 027 | 有证可见面已修 | `caa80ebe0697` | `7a30a4541220` | [proof](../m5-unit-surface-repairs-20261126/m5stack-u017-pcba/checks.json) |
| m5stack-u007 | 027 | 有证可见面已修 | `b6151b124129` | `7612a352d9f1` | [proof](../m5-unit-surface-repairs-20261126/m5stack-u007/checks.json) |
| m5stack-u082 | 028 | 有证可见面已修 | `5b2f39c34718` | `63c4b47ad1a1` | [proof](../m5-unit-surface-repairs-20261126/m5stack-u082/checks.json) |
| m5stack-k115 | 028 | 有证可见面已修 | `596e9a28212b` | `0d666299b41f` | [proof](../m5-unit-surface-repairs-20261126/m5stack-k115/checks.json) |
| m5stack-c008 | 031 | 有证可见面已修 | `7c1b302ff69e` | `e5a2cf210d53` | [proof](../m5-visible-face-review-20261126/c008/receipt.json) |

每项完整处置、来源限制、贡献快照、所有 proof/image hash、非视觉等价链与 K140 真实几何证据均见 [matrix.json](matrix.json)。独立集成检查通过前，批次列只表示交付范围，不表示 Root 已落。

P2 与 Core2 v1.1 的控制补映射也保留原三色 CPU 回执：验证器只允许删除明确的 controlId / 隐形 input region / inspection companion 指针，独立重建旧 JSON 的精确 SHA，再核对原色、金、紫的实际 loader 可见语义 digest。任意 geometry / material / texture 豁免均拒绝；输入最近射线证据绑定新模型，不将旧渲染改写为新哈希。

K132、K132 v1.1、K152、Capsule 两版和 C126-Ext 后续只新增隐形 boot 区域。矩阵独立删除该字段精确恢复旧模型，并分别读取绑定旧 hash 的 before 与绑定新 hash 的 after loader 回执，核对三材质可见摘要。跨回执的旧/新绑定不可互换；原 CPU 回执和图像保持原值。

额外五款 Stick/TLite 控制修复单独记在 [control-only-followups.json](control-only-followups.json)，对视觉候选分母贡献为零；用其固定的专用 loader 校验命令独立检查整合树。

K140 后续是真实几何增量：单独恢复有本 SKU 照片及官方尺寸图支持的橙色侧钮、实际壁孔和前 U 缺口，位置/轮廓约 ±0.6mm、未实测深度约 ±0.5mm；仍是程序估算壳体，未称厂家 CAD。最终模型 `4f974b93…` 和 geometry `a593e424…` 使用新 CPU 原/金/紫斜视及原色正视直接绑定。旧 `3ec4ac35…` 阶段所有 proof / 图片继续校验，但只作为历史，不给新几何做等价豁免。内部多高度 PCB 布局继续 UNKNOWN，故本项仍列为“源不足保留 UNKNOWN”的混合处置。
