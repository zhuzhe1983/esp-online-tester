# 第 030 批导入检查与证据保留

第 030 批精确交付提交为 `14ff3233d03478bf4b883e7cbbcfacec85021485`，
tree 为 `3652fc8251fdd129a285ef4a508800f04cb44f7c`。它从保留的第 029 批
精确交付提交续接，再与包含 Cloudzao 和 P4 布局修复的主线合并。来源标识、
所有分片与补丁的字节数和 SHA-256 见[导入进度](../../waveshare-patch-import-20261124.json)。

本轮只在完整导入全部新增批次后统一检查。`checks.json` 记录本地结果；源站
`waveshare-final-20261124/` 中的检查与性能数字保留原执行日期和结果。
源站的浏览器阻塞与 C3/C6 性能回退不会被本地模型显示结果改写。
本地浏览器检查没有加载固件，也没有新增实机或固件兼容声明。

## 原始记录与当前文件

[保留说明](curation.json)限定本次整理范围。厂家 CAD、照片、源归档、模型几何、
纹理和测试输入保持原字节；商品页及来源元信息的清理见后文。历史结果中的输入、数字、负面结果与限制均保留。

- [日志清单](retained-logs.json)和[目录校验日志清单](retained-research-log.json)记录移出当前树的原始日志路径、字节数与 SHA-256。
- [模型审图清单](retained-model-captures.json)记录原始 PNG 和临时网格展开数据。
- [JSON 传输清单](retained-json-transport.json)记录大 JSON 原始与 gzip 文件的
  hash。解压字节与原始 JSON 完全一致，所有采样值与源文件清单仍在。
- [retained_evidence.py](../waveshare-final-20261124/retained_evidence.py) 使历史检查脚本按原逻辑路径读取 `.json.gz`；其 hash
  核对仍以解压后的原始字节为准。脚本修改属于保留格式调整，并非重新执行历史验收。

所有上述原始记录都已在完整交付补丁中公开。本地精确交付提交保持未发布；
它及原始集成提交不属于新增发布提交的祖先。新发布提交仅以原主线为父提交。
本地副本置于忽略的 `target/batch030-source-evidence/`，新浏览器截图置于
`output/playwright/`；当前证据文件均小于 50,000 字节。

已有本地精确交付提交时，可从仓库根目录恢复原始模型审图和日志：

```sh
mkdir -p output/batch030-original
git archive 14ff3233d03478bf4b883e7cbbcfacec85021485 \
  docs/platform/evidence/waveshare-final-20261124 \
  docs/platform/evidence/waveshare-batch030-models-20261124 \
  docs/research/waveshare-s3-industrial-2026-11-24/batch-04-catalog-recovery \
  | tar -x -C output/batch030-original
```

也可从[公开交付清单](https://waveshare-adaptation-batches-20261124.zhuzhe1983.chatgpt.site/downloads/manifest.json)
恢复第 030 批。按清单顺序拼接 5 个分片，并先核对每片、完整 gzip 与解压 diff
的 hash。站点未提供独立完整 gzip 下载地址；不要用未经核对的新文件替代历史记录。

新克隆没有本地原始第 030 批提交时，下载并核对原始 diff 后运行：

```sh
python3 docs/platform/evidence/waveshare-batch030-import-20261124/recover-original-delivery.py \
  --patch output/delivery/waveshare-030.diff \
  --output output/batch030-original
```

脚本从已公开的第 029 批提交建立临时私有索引，核对精确目标 tree，并恢复证据；
默认不写仓库对象或 refs。若要继续第 031 批，可显式增加
`--local-ref refs/heads/codex/waveshare-delivery-030-recovered` 创建仅限本地的
原始交付分支。提交元信息可以不同，tree 必须一致；该分支不得推送。

14 个新商品页快照中的匿名表单令牌已清理，[前后 hash 与变换](source-normalization.json)
保留原 HTTP 取得时间和原始身份，同时固定清理后的正文与压缩文件身份。几何、
纹理、厂家照片与 CAD 字节保持原样。原始补丁的这些无关会话字段不进入新发布历史。
