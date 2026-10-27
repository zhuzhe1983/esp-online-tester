# M5Stack 官方 UIFlow 与小智固件补录

本轮在 86 个产品（84 个 M5Stack 型号）的目录中，为 46 个具体型号核对了官方项目。补齐 65 个远程固件选项：34 个新增、31 个原资料入口升级；另 2 个已有同版本 SHA-256 的 UIFlow 镜像保留原 URL 和 hash。固件目录共 148 项，watchlist 共 102 项。

| 系列 | 新版官方 watch 规则 | 对应产品选项 |
| --- | ---: | ---: |
| UIFlow2 | 40 | 45 |
| UIFlow1 | 9 | 12 |
| 小智（含要求 Atomic Echo Base 的版本） | 9 | 10 |
| 合计 | 58 | 67 |

产品选项按“设备 × 项目”计数；67 项中包括前述 2 个已收录镜像，并非 67 个不同设备。没有宣称全部 M5Stack 产品都有 UIFlow 或小智。尚无精确硬件映射的项目及型号保存在 [unmapped.json](../evidence/m5-official-firmware-catalog/unmapped.json)，没有拿相似型号补齐。

StopWatch 新增官方 UIFlow2 v2.5.3（项目 `1897200000000002172`）与小智 v2.2.6（项目 `1897200000000000855`），设备 ID 均为 `1897100000000000022`。原 User Demo 保持第一项和默认项。小智 Card Kit 的官方项目明确说明出厂用途，因此补上既有 factory 条目的固定版本地址；未将一般 UIFlow/小智条目改成出厂固件。所有原产品默认 ID、用户固件、PaperMono 与 Cardputer-Adv 已校验的同版本源均保留。

## 匹配与更新

新适配器 `tools/firmware-source-m5burner-v1.mjs` 读取官方项目详情和公开版本列表。项目、设备、版本 ID 全程为字符串。每次同步验证项目 `OFFICIAL`、精确项目/设备 ID、版本 `PUBLIC/PUBLISHED` 与设备归属，再构造固定版本下载地址。GitHub token 不传给 M5Burner。

设备分类不能代替 SKU 适配依据。本次实际发现：

- `sourceType=OFFICIAL` 列表查询仍返回社区项目；同步检查每条项目详情的真实身份。
- AtomS3 分类还包含 AtomS3U、VoiceS3R、AtomS3R-CAM 等不同硬件，只按人工核对的项目 ID 映射。
- 小智 AtomS3R 项目的当前版本是 `v2.2.6-AtomS3`；为 AtomS3R 选择 `NiHaoXiaoZhi` 分支，为 AtomS3 选择 `AtomS3` 分支，两者都明确要求 Atomic Echo Base。
- Core2 历史版本含 TOUGH 镜像，规则排除该分支。Basic/M5GO 选目录声明的 16 MB 分支，不使用 4 MB 项。
- 小智 Card Kit 被放在 Paper 分类，不将其固件配给 M5Paper。

项目说明、初始版本、精确型号合同见 [mapping.json](../evidence/m5-official-firmware-catalog/mapping.json)。未确认 UIFlow1 的公开源码仓库，`upstreamRepo` 留空；官方分发来源由项目 endpoint 单独标识，不虚构 GitHub 仓库。

按用户要求，**仅新版官方 provider** 可在发布方没有 SHA-256 时自动推进固定版本：新版为 `trial`，清除旧兼容性结论，归档旧源和验证记录，写明 `publisher-hash-unavailable`。浏览器下载时计算实际摘要并检查芯片、容量和启动布局。同版本不覆盖已有 pinned URL/hash；其他 provider 的无 hash 门禁不变。固定版本 ID 不能提供密码学内容保证，不能承诺发现上游在同一地址替换二进制。

`Check firmware update` Action 和发布前同步仍调用同一入口 `node tools/sync-firmware-catalog.mjs`。新增 provider 自动进入此流程；本轮实时同步的 114 次元数据请求覆盖 57 个项目、58 条规则，全部成功。快照按每条规则保留最近 5 个适用版本，仍小于浏览器 2 MiB 限制。本站不留存第三方 BIN，目录补录和版本检查不执行固件。

## 验证与边界

- Catalog schema 通过：86 产品、7 配件、148 固件、102 watch entries。
- 全量 Node：99 项通过，含新 provider 的官方身份、字符串 ID、设备/版本分支、失败保留、无 hash 例外、同版本保护、旧版 provider 门禁与隐私过滤。
- 真实 API：58 条规则成功，零固件正文下载。最初 20 项因旧版本文件名以 `file` 结尾而失败；已保留负面记录并改为安全保存原名，具体资产规则仍只接受 `.bin`，不把旧文件猜成完整镜像。
- Headless 浏览器：StopWatch 出厂默认、两项新增固件的二次确认和取消、AtomS3R 版本隔离、小智 Card Kit 未适配时隐藏运行按钮均通过；没有启动固件。任务浏览器和 HTTP 服务已关闭。
- 没有下载 BIN、核验实际镜像布局、运行兼容性或连接硬件。现有板级模拟器才提供尝试运行；其他设备仍显示“尚未完成适配”。没有修改 CPU、总线或 Rust/WASM，不进行性能基准。

证据与复现命令见 [README](../evidence/m5-official-firmware-catalog/README.md)。此前出厂固件审计保留原时间与结论，本报告补充本轮新增来源，不把旧资料核对改写成新实测。
