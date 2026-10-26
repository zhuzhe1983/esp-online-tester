# Lane01 追加12项模型审核

审核时间：2026-10-26T20:45:06Z。12项均为 `partial`，没有整机1:1完全通过项；原厂CAD未缩放。

| 产品 | 尺寸检查 | 近正视照片未通过 |
| --- | --- | --- |
| [m5stack-k146](m5stack-k146.md) | 通过 | 无定量未通过；定性/未测量仍单列 |
| [m5stack-s007](m5stack-s007.md) | 未通过：height | 无定量未通过；定性/未测量仍单列 |
| [m5stack-k051-b](m5stack-k051-b.md) | 未通过：depth | 无定量未通过；定性/未测量仍单列 |
| [m5stack-k131-v11](m5stack-k131-v11.md) | 通过 | front |
| [m5stack-k147](m5stack-k147.md) | 未通过：base-depth | 无定量未通过；定性/未测量仍单列 |
| [m5stack-c126-ext](m5stack-c126-ext.md) | 通过 | back |
| [m5stack-c123](m5stack-c123.md) | 通过 | 无定量未通过；定性/未测量仍单列 |
| [m5stack-k150](m5stack-k150.md) | 通过 | 无定量未通过；定性/未测量仍单列 |
| [m5stack-k010-aws-v13](m5stack-k010-aws-v13.md) | 未通过：depth | 无定量未通过；定性/未测量仍单列 |
| [m5stack-k128-se](m5stack-k128-se.md) | 未通过：depth | 无定量未通过；定性/未测量仍单列 |
| [m5stack-c050-b](m5stack-c050-b.md) | 未通过：depth | 无定量未通过；定性/未测量仍单列 |
| [m5stack-k016-c](m5stack-k016-c.md) | 未通过：width, height, depth | 无定量未通过；定性/未测量仍单列 |

每项 `docs/platform/evidence/<id>/verification.json` 保存数值、来源和资产哈希，`browser-review.json` 保存真实浏览器回执，`appearance-review.webp` 保存原色/金色/半透明正反斜视。

主要边界：

- StampS3/StampPico 的完整板级布局未取得，保持 UNKNOWN；Pico 包装套件只建其中模块。两张印刷膜按各自官方斜照四角校正并剔除金属紧固件，不能当作独立尺寸依据。
- Xiaozhi 模型含厂家 CAD 的支架，官方54×84×8mm只适用于手持部分，不能当作整个组合外形。
- AirQuality 的三片附加安装耳复用厂家原始组件，以官方照片作刚体装配估计；前视差异仍保留。
- AtomS3R Ext 的前部金色洞洞板/FPC尚无验证PCB数据，保留UNKNOWN，未用照片冒充电气模型。
- AtomS3 白色外壳在白底照片上的阈值分割无效，原0.9335/0.6819负诊断和对照另存，照片验证改为定性，并未改为通过。
- Core2 AWS 为23.5mm而规格23.7mm；CoreS3 SE来源CAD为16.5mm而规格15.5mm；StickC尺寸及爆炸归位为估计，均保留失败。
- 可见排母、Grove、螺钉、胶垫的照片估算与厂家几何分开。未核对插座局部线序的接口禁止显示已连接线缆。
- 无板级适配、无固件或实机验证；官方固件入口只作参考，不下载镜像入仓库。

复现：`tools/onboard-lane-01-extra.py prepare/build/verify/publish`；相机参数在每项 `model-plan.json`，`tools/calibrate-lane01-extra.py` 只注册相机、不改几何；实际headless截图通过 `tools/capture-lane-01.py`，材料图通过 `tools/build-lane01-review-grid.py`。

原27+追加12合计：39份知识记录通过校验，840条实际下载记录的字节数/SHA-256通过复核（678个不同内容哈希），268个模型声明资产哈希通过。
