# Seeed 48 项范围的最终集成记录

本批覆盖 **23 个独立产品/商业版本和 25 个官方板屏配置，共 48 个目录身份**，不是 48 个独立制造商 SKU。其中四项是没有屏幕的 EE 裸板。目录状态为 **47 项 `partial`、D1001 一项 `unavailable`**；接入目录、具备模型和数字板级子集，不等于全部出厂功能已兼容。设备逐项范围见 [总说明](../../seeed-onboarding.md)和 [SKU 核对](../../research/seeed-scope-2026-11-22.md)。

生产 WASM 固定为 **6,056,179 B**，SHA-256 为 `a3f0c90c47f9403225084c7138a3ec6c6b7ab5d2d9a0d1e6d3db8938a7379a75`。最终设备探针、网页回归和本地 Pages 产物使用同一份 WASM。

## 最终设备执行与模型

| 检查 | 实际结果 | 证据 |
| --- | --- | --- |
| 目录及来源 | 全站 267 产品、10 配件、234 固件、149 watchlist；其中 Seeed 48 个身份及其 267 份原始 HTTP 正文摘要核验通过 | [目录与来源收据](catalog-and-sources.json) |
| 48 个模型 | 每项独立模型定义与本地几何；几何合计 121,033,296 B；实际 Three.js loader、384 个屏幕射线和 109 个可见按钮射线通过 | [扩展模型验收](../seeed-expanded-models/README.md) |
| 最终 Seeed 执行总量 | 19 次独立运行、18 个不同产品身份、26 次输入、2 次数字 peer 注入；52 次捕获包含 49 份 RGBA 与 3 个 `NO_FRAME` | [逐次清单](final-wasm-inventory.json) |
| 原始十二机 | 重新 ROM 启动；26 份 RGBA 与先前检查点逐字节相同，3 个 `NO_FRAME` 保持相同 | [十二机收据](../seeed-publish-wasm-20261122/README.md) |
| D1L / D1Pro | 各自原厂镜像独立启动，触摸后打开密码键盘；四份完整画面与先前检查点相同；无线器件在该原厂路径仍处于复位 | [新增商品收据](../seeed-publish-extra-products-20261122/README.md) |
| D1S / Watcher W1-A | 各以自己的商品身份独立 ROM 启动。D1S 一次触摸打开密码键盘；W1-A 三次输入被接受，画面均未改变，不宣称导航或业务回调通过。六份 RGBA 与同 WASM 的家族参考逐字节一致；127 个相关文件逐次前后摘要不变 | [最后两项商品实跑](../seeed-publish-remaining-skus-20261122/README.md) |
| Watcher XiaoZhi | 官方 2.0.4 恢复镜像；旋钮触发真实音量回调，短按进入 `audio_testing`；三个输入后画面完整匹配历史捕获。启动画面的 3,741 个差异像素仅位于标题区域，保留不同捕获时刻与差异范围 | [独立执行与差异](../seeed-publish-extra-products-20261122/receipts.json) |
| Meshtastic | 两次运行共九份完整 RGBA 相同；较长运行达到 20.36 客体秒，启动 logo 不再漂移或错色。8 秒早期数字接收进入 RadioIf 并清 IRQ；16 秒晚期接收仅到达扩展器 ISR，IRQ 保持为 2 | [两个观察窗口](../seeed-meshtastic-final-20261122/README.md) |

**49 份 RGBA 不是 49 次完整 UI 成功。** 它们包含白屏、低电量页、配网页面和启动 logo。XIAO C3 电子纸仍触发 RTCWDT；Sticky 仍有 LittleFS/字体问题；E1001/E1002/E1004 默认 WASM 首屏为 `NO_FRAME`。Meshtastic 未证明退出启动画面、完成按键业务、读取全部 RX payload、发送或组网；不能把器件接收计数等同于应用消息成功。18 个执行身份覆盖本范围中除四款无屏 EE 裸板和不可运行 D1001 外的全部独立商品。D1S 使用共享官方 direct merged BIN，W1-A 使用现有带 W1-B 名称的官方家族恢复 manifest；独立的是商品配置与执行，未杜撰不同的固件字节或 manifest。

EE 套件的 17 份完整 SenseCraft 1.1.5 镜像及 19 次历史探针在 [EE 专项报告](../../seeed-ee-runtime.md)单独保留，未冒充这份最终 WASM 上重新运行的结果。另八个面板配置没有取得完整可启动原厂镜像。D1001 的真实首个阻塞是 P4 ADC 熵源，DSI、DW_GDMA、DMA2D 与硬件 I2C0 的后续缺口按源码另列，不用伪 SPI 板替代。

模型中有官方 CAD、基于官方照片和尺寸的重建、基于 PCB 的裸板模型与明确标注的屏幕调试面板。没有结构证据的部分保持未知。144 个 CPU 渲染视图与 Three.js 几何测试不代表真实浏览器 WebGL 验收。

## 回归与有意改变的 golden

共享 RGB 修复保留分数时间额度、只消费完整 RGB565 像素，并在当前帧末停止 DMA 取数，避免与客体 ISR 的 34 字节预取补偿重复。修复前失败的 513 字节、规则/不规则服务和超大时钟步长用例均保留；来源、负例和补丁见 [帧边界报告](../seeed-lcd-vsync-20261122/boundary-review.md)。这会有意改变旧的取整时序。

首次最终原生回归通过两项 Clippy 和完整 workspace build，随后在 `panel_sid` 的控制台 golden 停止；当时其余三项断言尚未执行。该 [失败收据](pre-golden-review-gates.json)及压缩日志保留。独立基线/候选对照确认：最终 480×480 画面全部 921,600 个 RGBA 字节相同；音频整体提前一个采样点，约 45.35 µs，135,078 个重叠采样逐字节相同；控制台仅两处时间戳提前 1 ms，中断、异常和指令数的变化逐项记录。新增末端音频采样不宣称拥有已验证的未来对应值。

据此仅有意更新 `panel-sid.console.txt`、`panel-sid.report.txt`、`panel-sid.wav.sha256`、`panel-sid.insns` 四份预期文件，保留控制台 CRLF，没有放宽比较或使用全局 golden 重写。详见 [四项 golden 审查](../seeed-panel-golden-20261122/README.md)。这不是性能优化或实体硬件 IRQ 时间校准。

| 最终检查 | 结果 | 收据 |
| --- | --- | --- |
| Native 与 WASM Clippy，warnings denied | 两项通过；后续只改上述预期文件与文档，Rust 源码摘要保持相同 | [首次检查记录](pre-golden-review-gates.json) |
| Workspace release build | 通过 | [同上](pre-golden-review-gates.json) |
| 更新后的定向 `panel_sid` | 1 项通过；`UPDATE_GOLDENS` 未设置，所有 tracked golden 文件摘要前后不变 | [定向收据](targeted-panel.json) |
| 完整 native workspace，含 ignored、排除 `external_` | 2,314 项通过、0 失败、0 ignored；95 项依赖外部输入的测试按 CI 规则过滤；Rust、所有 golden 和生产 WASM 摘要前后不变 | [最终原生收据](final-native-gates.json) |
| Release CLI 与 native virtual quantum | 构建通过；显式启用 native virtual quantum 的 44 项测试通过 | [同上](final-native-gates.json) |
| WASM JIT differential 与 profile build | 111,126 个差分用例、78 个隔离组、24 个 GPIO 波形用例通过；`jit-profile,cpu-profile` release 构建通过 | [同上](final-native-gates.json) |
| 100 个平台测试文件 | 438 项通过，0 失败、0 skipped | [最终 Node 收据](../seeed-publish-node-gates/receipt.json) |
| 其余网页执行/ABI 回归 | 14 个既有 demo 与 JIT handoff、生产 sections、真实 recording、StopWatch/诊断、timing/BLE、worker pacing/channel、touch/UI 均通过；连同平台套件共 13 组 | [13 组逐项日志](../seeed-publish-node-gates/receipt.json) |
| 补充 CI 合同 | 历史 x5/m1 收据、network relay、WASM build policy 和 benchmark harness 等检查通过 | [补充检查](../seeed-extra-ci/receipt.json) |
| Network helper 集成 | 初次因 backend 尚未构建而退出；workspace build 后真实 HTTP/TLS/UDP、认证 WS 及映射清理检查通过，初次失败仍保留 | [补测收据](../seeed-extra-ci/network-helper-after-build.json) |

### 主机 CPU 比较

完整回归后冻结独立 release CLI，使用不变的 release profile，与原 main 的可执行文件进行三对交替测量。S3/C3/C6 hello 与 Pocket Tank 各 30 客体秒共 24 次运行，再执行预先计划的 S3 hello 300 秒窗口共 6 次。计时期间暂停本任务的构建、固件探针和归档。**全部 30 次执行的控制台字节、停止条件、各核指令、周期、异常、总中断和逐来源 IRQ 表精确相同**；输入和两个可执行文件的最终摘要均未变。

| 负载 | 基线 CPU 中位数 / 秒 | 候选 CPU 中位数 / 秒 | 差异 |
| --- | ---: | ---: | ---: |
| S3 hello，30 客体秒 | 0.60 | 0.60 | 0.00% |
| C3 hello，30 客体秒 | 5.51 | 5.56 | +0.91% |
| C6 hello，30 客体秒 | 3.29 | 3.22 | −2.13% |
| Pocket Tank，30 客体秒 | 194.14 | 192.48 | −0.86% |
| S3 hello，300 客体秒 | 5.54 | 5.53 | −0.18% |

基线文件 SHA-256 为 `22954f6f7ef68a1d12e76a07ed3494c25e9cc2c231781ea912d479af8f3a87be`，最终候选为 `4dd4d549dabb2e11282e692119558bb3a4907baa02be345aac9c5ff71176c9a4`。新远端 `43de45a` 没有 Rust 或 build manifest 变更，故原 `573f5a6` 原生基线仍对应相同代码。

每组仅三对，范围重叠、存在计时粒度和轮次波动，不据此宣称零成本或性能提升；C3 的正增幅如实保留。更早整合版本的 S3 短窗口 +18.64% 和未完成的 Pocket 检查点也保留，不用这份不同二进制的新结果覆盖。完整命令、全部样本、逐来源 IRQ 比较与条件见 [最终性能报告](../seeed-performance-final-20261122/README.md)。

## 保留的历史检查点

| 检查点 | 实际结果 | 收据 |
| --- | --- | --- |
| 48 个模型、最终目录 | 121,033,296 字节几何；384 屏幕射线、109 可见按钮射线；Three.js 实际 loader 全部通过 | [模型验收](../seeed-expanded-models/README.md) |
| LoRa 板型整合后的完整 native workspace | 2,303 项通过、0 失败、0 ignored；95 个依赖外部输入的测试按 CI 规则过滤；两项 Clippy 与 release CLI 构建通过 | [源码与日志摘要](lora-checkpoint-gates.json) |
| `f3ad2ef6…` 生产 WASM | 100 个平台测试文件、438 项通过；14 个既有 demo 与 JIT handoff、真实录制、StopWatch、timing/BLE 和 worker 检查通过 | [13 组检查](../seeed-release-node-gates/receipt.json) |
| D1L / D1Pro 各自原厂默认镜像 | 各自 ROM 启动、Wi-Fi 设置画面及触摸打开密码键盘；没有继承传感器、云端或无线固件验证 | [真实执行](../seeed-meshtastic-20261122/after-lora.json) |
| 原始 Meshtastic `2.5.11.8e2a3e5` | SX1262 初始化成功；显式数字包进入器件 FIFO 并触发 IRQ 与客体处理日志；默认区域未设置，应用层拒绝接收；不证明客体读完载荷，此检查点 RGB 仍全黑 | [无线与显示边界](../seeed-meshtastic-20261122/after-lora.json) |
| 更早的 native 编译尝试 | 磁盘空间不足而退出 101；没有把未完成的测试写成通过 | [原始失败与脱敏日志](prior-disk-failure.json) |

这些历史 native/WASM 结果不替代最终产物的独立检查，也不代表实体硬件、物理射频、WebGL 或浏览器 CORS 通过。

## 发布与来源边界

本地按照发布工作流准备的实际产物为 **2,209 个文件 / 661,894,046 B**，包含生产 WASM、ROM、固定依赖、两套完整 skill 和全部运行模型，不是按源码估算容量。独立验收核对了 152 个模型 manifest 的 295 个依赖引用、47 个固定工作流资源、58 个 vendor 文件和 805 个 JSON；556 个实际文件合计 485,556,243 B 经过摘要核验。见 [实际产物验收](../seeed-publish-dependencies-20261122/artifact-check.json)和 [预发布收据](pages-preflight.json)。

Pages 不发布 181 个源 CAD/PCB 档案、共 448,016,304 B；七个内部 StopWatch fixture 也不进入公开产物。当前预发布目录的五个版本 URL 使用八字符检查点 token，最终干净提交可替换这五处 token 而不改变目录字节总量。该替换不修改仓库源码，也不等于远程部署。

第三方设备固件仅记录远程来源和摘要，不新增到仓库或本站。厂商 CAD/PCB 输入保留在 Git，Pages 只发布运行几何、必要纹理、模型定义与来源收据；两个既有其他品牌模型审核页的明确 STL 下载入口保留。

原始基线为 `573f5a6c1bcc6dbf0388383d178a3935ec8ddcce`。发布前只读复查发现远端新增 `43de45a1244fd1a0a023beba778adaee169f86bc`，只有三个固件目录文件的 266 处时间戳变化。已按稳定记录 ID 合并全部变化，保留新增 Seeed 条目、全部非时间语义与原序列化；没有改动 Rust、产品、模型、固件来源、摘要或布局。最终交付提交将直接继承这个新远端基线。见 [三方合并收据](remote-metadata-merge.json)和 [合并后的 schema 与 23 项合同测试](remote-metadata-checks.json)；同样三份文件已更新至本地 Pages 产物，文件数和总字节数不变，见 [产物补充核验](pages-metadata-update.json)。新增 Seeed 的原抓取时间和错误状态不变，顶层新时间不表示全部来源均已重新成功抓取。

当前 GitHub 连接的写入被 HTTP 403 `Resource not accessible by integration` 拒绝，命令行也没有可用 HTTPS 写入凭据。尚未完成远端提交或 Pages 部署。用户已经授权推送和部署；缺少的是该仓库可用的写入权限，不是发布意愿确认。本次有工作流文件变更，认证权限也须允许相应工作流更新。

[独立合并审查](remote-metadata-independent-review.json)另行从远端原始 delta 与合并前副本构造完整预期结果，确认全部 266 处更新、27 个 Seeed report 条目、27 个 Seeed snapshot 条目和 56 个 Seeed firmware 条目完整保留。七个旧失败快照只更新 `error.attemptedAt`，没有把缓存成功时间改写成新抓取时间。

本地 Chromium 无法创建其所需 socket，云浏览器无法访问工作区回环地址。软件模型渲染、Three.js 射线检查及 Node 中的真实 WASM 执行分别保留，不能写成真实浏览器验收。AArch64 CI 与部署后公开 HTTPS 的验证也不能由当前环境代替。

[发布前一致性检查](publication-checks.json)确认 631 个 Rust 源文件、92 个 golden、原生二进制与生产 WASM 等摘要仍与最终门禁相同，全部 19 份原始执行收据摘要都有公开脱敏证据对应，697 个已检查的本地文档链接没有缺失。隐私扫描通过；完整文本空白扫描仅报告两个原厂 STEP 注释行末的既有空格，这两份文件与官方归档提取摘要完全相同，作为源数据保留。代码、测试、文档和其他资产的空白检查通过，未改变仓库检查配置。

测量中的本地检查点提交属于未发布整合历史。交付树以已审阅、已去除非必要来源元数据的最终文件形成单个新提交，直接继承已合并的远端 `43de45a`；不附带被替换的未脱敏历史。保留每次实际测量的产物、输入、原始日志与脱敏日志摘要，复现以这些摘要和最终保留来源为准。Git bundle 只导出交付 `main` 及其公开父提交作为 prerequisite；完整 Pages 静态包另含逐文件校验清单。包内文件、缓存 token 和 Git 对象范围在打包阶段核验，记录于随包的交付清单。
