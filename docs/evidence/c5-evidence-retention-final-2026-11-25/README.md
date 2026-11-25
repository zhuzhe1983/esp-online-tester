# C5 证据留存补充

完整逐 test stdout 从当前 tree 移至 ignored 输出，compact receipts 补充按历史 SHA256 核实的日志字节数。原命令、输入/来源、结果、负结果、计数和历史 artifact hash 均保持；这是证据位置与元数据整理，没有重新运行组件、原厂固件、构建或性能检查。

快照为 `df8a8bf5aae88afd0c3a71ddfc0ead07b2480e40`。只处理相对 `461695ca` 至 `d55f23d0` 新增的旧 C5 证据，不改产品代码、平台文档、新 modem-wifi-clock 证据、golden 或 experiment catalog。

## 留存内容

- [stdout-manifest.json](stdout-manifest.json)：12份完整输出，合计22791 bytes。记录原路径、Git blob/artifact revision、被测 source/input 所在的固定 receipt、原重现命令及结果。artifact revision 是取回该文件的版本，不冒充被测 source revision；原 receipt 的 source hash/baseline 保持。
- [log-artifacts.json](log-artifacts.json)：143个不同的历史日志 SHA256，全部已匹配实际 ignored 原件的 bytes。原 native UART、register/trace 和测试 stdout 均不复制回 Git。
- [24日引用](log-references-2026-11-24.json) / [25日引用](log-references-2026-11-25.json)：34份 JSON 的164处引用，按 receipt 日期分组，记录原 hash 字段及新增 bytes 字段的精确 JSON pointer。重复引用不是新的 artifact 或新的执行。
- [component-output-hashes.json](component-output-hashes.json)：47处 raw/normalized/output hash 引用。34处匹配字节，13处未在限定 capture 根找到原件，bytes 明确为 null。保留原字段名和 hash；不按 curated 大小推算 raw 大小，不把新运行挂到旧 hash。
- [receipt-hash-mapping.json](receipt-hash-mapping.json)：42份 JSON 和11份 README 的旧/新 bytes/SHA256、旧 Git blob和 ignored 原件位置。JSON只增加 bytes/留存字段；每个原有字段值及负结果可逐项还原核对。README只调整 stdout 链接并补留存说明。

APM negative history 的失败 `log/sha256` 与修正 `correctedLog/correctedSha256` 分开匹配，分别记录 `logBytes` 和 `correctedLogBytes`；3 passed/1 failed 的原记录保留。native `factory_machine` 使用 CPU insn_count 的历史标签更正也保持，不将其改称 retired。

此前 BOD0/WiFi-map/COEX gate 六份 stdout 的[独立清单](../c5-owned-stdout-retention-2026-11-25/manifest.json)保持原样，本目录不重复移除它们。此次12份已逐字复制、验 SHA、确认可读及 ignored 后才撤当前跟踪文件。原文件仍可按 manifest 的固定 revision/path 用 `git show` 取回；没有改写旧提交或上传 release。

## 输出位置与核对

`${INTEGRATION_ROOT}` 是调用者选择的集成仓库根，新增私有输出在其 `target/c5-evidence-retention-final-20261125/from-df8a8bf5/`；`${C5_CACHE_ROOT}` 是保留原始 capture 的 ignored C5 cache 根。摘要使用这些标签，不写本机 home、用户名或 Trash 路径。所有旧 receipt 原件也保留在该 ignored 输出，旧→新映射不覆盖历史 hash。

```sh
python3 docs/evidence/c5-evidence-retention-final-2026-11-25/verify.py \
  --repo "$REPO" --integration-root "$INTEGRATION_ROOT" \
  --cache-root "$C5_CACHE_ROOT"
git diff --check
node tools/check-evidence-privacy.mjs docs/evidence/c5-evidence-retention-final-2026-11-25
```

`verify.py` 只读 Git历史、当前metadata和可选调用者capture路径；不执行记录里的重现命令。它核对旧 JSON 字段值、README 可重放修改、所有旧/新 hash/bytes、12个已撤 stdout 和实际原件。省略两个本地根仍可核对 Git/metadata，不能据此声称私有副本重新验证。实际静态检查见[curation-checks.json](curation-checks.json)。

13处未定位仅指 ignored cache、原 curated stdout 和本任务清理清单指定的旧 task target 范围，没有扫描整个 Trash，也不宣称原件全局丢失。受影响的 raw/normalized 关系、初始失败和命令仍可在原 receipt 查到；未知 bytes 不影响既有测量值，但限制对该原始stdout的逐字核验。

此整理没有新功能或性能结果。私有raw仍需由拥有者另行提供；公开树保留重现命令、版本、输入、hash、数字与限制，没有新增完整 console/event流，也不提高 LCD/SD、WiFi/RF、硬件时序或安全功能的验收范围。
