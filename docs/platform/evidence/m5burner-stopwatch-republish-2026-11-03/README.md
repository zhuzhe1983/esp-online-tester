# StopWatch UIFlow2 同名版本重新发布

2026-11-03 查询官方项目 `1897200000000002172` 及其完整 PUBLIC 版本列表，StopWatch 设备 ID 为 `1897100000000000022`。当前 `v2.5.5` 的版本 ID 是 `2117443003011928065`；目录此前保存的 `2116093081161424897` 已不在返回的 11 条记录中。版本名不是唯一发布标识，同名重新发布会产生新的下载地址。

来源 URL、抓取时间、原始响应长度与 SHA-256 见 [sources.json](sources.json)。[metadata.json](metadata.json) 保留完整版本集合、项目/设备身份、文件名、发布时间与状态，去除了请求 ID、账号、统计等无关字段；来源记录同时保存清理后文件的 hash。这份元信息采集只读取两份 JSON，没有请求下载端点、下载 BIN、运行固件或访问实机。官方版本说明使用 `preview` 字样，但版本名仍为 `v2.5.5`，目录继续按既有版本名规则选择渠道，并明确标记未验证 trial。

同步器在筛选渠道、设备或压缩历史之前保留全部版本 ID。对于版本名相同的候选，只有下列条件同时成立才替换当前地址：

- 新旧记录均属于同一经过核对的官方 M5Burner 项目、设备和固定版本 URL；新旧版本 ID 不同。
- 当前文件没有固定 SHA-256；带 SHA 的当前来源及 URL 保持不变。
- 完整版本清单来自最近 24 小时内成功的官方项目和版本查询；候选 ID 存在，旧 ID 已不存在。旧 ID 即使被渠道、状态、设备或历史数量过滤掉，只要仍在原始清单中就不替换。
- 候选发布时间晚于当前目录记录，且仍满足明确的设备、文件格式、渠道和名称规则。

原来源、验证结果和下载诊断进入 `artifactHistory`；替换后的条目清除原验证及下载状态，重置为未验证 trial。watchlist 同步保存旧资产元信息。此次只更新 StopWatch UIFlow2 的目录、watchlist 和发布快照，其他产品不变。

回归命令：

```sh
node --test tools/platform-m5burner-v1-sync-test.mjs tools/platform-firmware-sync-test.mjs
python3 tools/validate-platform-catalog.py
node tools/check-evidence-privacy.mjs
```

测试覆盖同名新 ID、旧 ID 仍存在但未进入压缩历史、状态/设备过滤、已有 SHA、元信息过期/失败、项目或 provider 不匹配，以及历史与验证重置。元信息同步不证明固件能够下载、启动或正常操作；本次未实机验证。

另行检查旧版本下载端点，HTTP 404 JSON 明确返回 `VERSION_NOT_FOUND`；保留的脱敏结果与界面回归见 [download-recovery.json](download-recovery.json)。烧录窗口将其与验证码错误区分，提供同窗口重试与公开版本刷新；必须由用户选择版本再重新准备。已准备好的批次不会替换来源。配置布局与“固件已固定”的提示均等待实际校验完成。
