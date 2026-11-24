# CloudZao 证据阶段说明

本目录为 2026-11-23（Asia/Shanghai）批量适配保留可核查的公开资料和验证结果。
JSON 收据使用 UTC；没有保留账号列表、上游 uploader 信息、设备凭据或固件正文。

| 文件 | 阶段与含义 |
| --- | --- |
| `inventory-and-firmware-audit.json` | 初始仓库/渠道调查；其中 `binaryFetched:false` 描述 17:18 UTC 的下载前状态；`subsequentEvidence` 指向后续审计 |
| `official-firmware-sources.json` | 原始两项发布元数据快照，也是 RAM 审计的可重跑输入；不把历史字段改成事后状态 |
| `chronology.json`、`catalog-chronology.json` | 仓库创建日期、最早可达 commit 和固定源码版本；用于明确标注的排序代用值，不是上市证据 |
| `source-receipts.json` | 原始 HTTP 响应的 URL、实际字节数、SHA-256 与抓取时间；涉及账户字段的原文不复制，哈希仍对应原始响应 |
| `firmware-memory-audit.json` | 随后的完整下载静态核验；仅在 RAM 中读取；记录实际镜像布局和内部版本 |
| `claw154-source-review.json` | 独立源码/边界复核，包括 PWM 门控、光学颜色与导航坐标限制 |
| `catalog-source-review.json` | 集成审阅发现及 `integrationResolution`；保留已经解决的负面结果，不抹去检查历史 |
| `validation.json` | 最终本地构建、测试、模型和环境限制的汇总 |

Claw 1.54 的后续真实官方镜像运行单列于
[`../cloudzao-claw154-official-probe-20261122.json`](../cloudzao-claw154-official-probe-20261122.json)。
该文件的日期后缀沿用 UTC。`runtimeSources` 是实际测试字节的摘要，本地中间提交不是用户
应用补丁所需的基准；补丁基准以交付包的 `BASE_COMMIT.txt` 为准。

两款 Claw 的模型来源与确定性重建收据在各自资产目录和专项报告；两款 INK 已有的厂家
STL/STEP、作者署名及 CC BY-NC 4.0 许可继续保留。初始 GitHub 树中没有 CAD 不代表全网
没有厂家模型，调查中的 INK 条目已明确链接既有 MakerWorld 模型。
