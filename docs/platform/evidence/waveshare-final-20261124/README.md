# Waveshare 第 030 批：最终覆盖与验证

本目录记录第 030 批的候选源码、目录、模型与测试证据。第 030 批增加最后
4 个系列、16 个 SKU / profile。与此前 029 批合并后，覆盖保留的官方分类快照
中全部 **149 个主控系列、383 个主控 SKU、407 个精确硬件配置**。

这里的完成指上述范围的产品入库、来源、机械表示及已声明的仿真能力覆盖。
所有 407 个产品的 runtime/model 仍为 `partial`，不代表所有原厂应用、外设、
浏览器交互或物理硬件都已验收。补丁全链与实际 Sites 下载回读的最终结果，
另外写入下载包中的 `README.txt`，以免在补丁内部循环引用自身的最终 hash。

## 范围与本批增量

| 系列 | 本批 SKU / profile |
| --- | ---: |
| ESP32-S3-Nano / Nano-M | 2 |
| ESP32-S3-Zero / -M / -N8R8 / -N8R8-M | 4 |
| ESP32-S3-DEV-KIT：N8R8、N16R8、N32R16V、N16R8-U 及排针款 | 8 |
| ESP32-S3-Pico / Pico-M | 2 |

官方分类清单快照完成时间为 **2026-11-23 16:35:55 UTC**。180 个分类 listing
中，149 个纳入主控或含主控套件范围。纳入系列有 384 个原始 SKU；其中
SKU33355 是不含主控的 `ESP32-XX-Basic-Kit-Acce` 附件包，排除后为 383。
24 个 SKU 各有两套明确硬件修订或控制器配置，因此 profile 数为 407。

149 个 fragment 与官方逐系列 SKU 集合全部相等，407 个产品对象与中央目录
逐对象一致。原 029 的 660 个产品、584 个固件、416 个 watch 和 668 个知识索引
对象全部保留。本次增加 16 / 25 / 25 / 16，最终中央目录分别为
**676 / 609 / 441 / 684**。实际导出器的目录过滤与序列化也已在内存中检查，
四个最终目录字节与测试输入完全一致，没有静默丢弃条目。

当前 407 个 profile 引用 259 个不同的模型 URL；共享机械外形的套件、容量或
修订变体不计作独立机械模型。本轮未核实各型号的硬件发布日期，全部
`chronology.date` 保留为 null。商品 Position 是排期参考，实际还按公共实现
分组交付；不能声称严格按发布日期或全程先完成所有屏幕型号排序。

逐项清单、canonical guards、来源链接及历史元数据说明见
[发布前目录审计](publication-catalog-audit-README.md)和
[完整审计 JSON（gzip）](publication-catalog-audit.json.gz)。早期 inventory 的未完成标志、
404 调查记录和旧缓存路径保留其历史含义；最终覆盖以本次审计和发布 manifest 为准。

## 模型与固件边界

本批交付 15 个新生成模型，并保持已发布的基础 Zero 模型全部 10 个文件不变。
其中 3 个新模型使用厂家原生 CAD 或在原生 CAD 上补充明确估计的排针；
另 12 个按官方尺寸和精确型号照片重建。模型目录保留输入字节、取得时间、
SHA-256、原始与保留编码、生成参数、几何、纹理、权益说明和复现步骤。

15 个新模型的生产 Three.js loader、有限几何、尺寸与 CPU 射线检查通过。
对应正反侧视和官方图叠图已经逐图检查，验收绑定最终 model/plan/source hash。
黄色排针塑料、Zero N8R8 照片的 420×360 尺寸和被遮挡板角的外推说明均已修正。
旧 batch-04 图片仅证明其历史模型修订，不能用于当前新资产的验收。

- [Nano / Pico 的来源、交互和模型证据](../waveshare-batch030-models-20261124/nano-pico/README.md)
- [Zero / DEV-KIT 的 CAD、照片和模型证据](../waveshare-batch030-models-20261124/zero-devkit/README.md)
- [本批目录与固件恢复记录](../../../research/waveshare-s3-industrial-2026-11-24/batch-04-catalog-recovery/README.md)

未知板厚、元件高度、隐藏结构、插合深度和实物公差继续明示为估计或 UNKNOWN。
几何一致性容差不代表制造精度。Nano 没有虚构 BOOT 控件；GPIO0 保留为 RGB
通道，物理 RESET 外形与平台独立重启能力明确区分。其他模型也不以外观连接
代替电气路由或固件执行。

全部 Waveshare 产品关联的 246 条固件中，180 条为 `trial`、66 条为 `reference`；
`simulationVerified` 与 `hardwareValidated` 均为 false，285 条关联 watch 同样
没有继承整机或实机验收。完整镜像允许按现有试运行流程尝试；缺少完整启动
材料的保留为资料参考。25 条本批新增固件/25 条 watch 没有冒充出厂身份，
也没有把示例、开发环境或其他容量型号的原 BIN 验收转移给新 SKU。

## 最终检查

| 检查 | 实际结果 |
| --- | --- |
| 全目录 validator | exit 0；四目录 freeze 与最终平台输入相同 |
| Host Clippy / WASM Clippy | 两者均以 `-D warnings` 通过 |
| Release workspace build | 通过 |
| 原生完整 workspace suite | 2,884 passed / 0 failed / 0 ignored；按 CI 的 `--skip external_` 排除 136 项 |
| 原生 VQ 专项 | 44 passed / 0 failed / 0 ignored |
| 同配置性能可执行文件构建 | baseline 与 candidate 均为 `cargo build --release --bins` |
| WASM / JS 检查 | 24 个步骤通过；包括页面固件、ABI、Worker、录制回放和平台合同 |
| WASM JIT / profiling | 111,126 个差分案例、78 组、99,679 个模块释放；另 24 个 GPIO 波形案例通过；profiling 构建通过 |
| 最终平台合同 | 507 / 507 passed；0 failed / skipped / cancelled；127 个测试文件 |
| 当前资产与平台测试绑定 | 237 项数据/资产及 128 项测试/采集器文件，字节与 SHA 均仍一致 |
| 真实浏览器 / WebGL | 0 / 16 完成，环境阻塞；未记为通过 |
| 物理硬件 | 未实机验证 |

原生 suite 在最终源码 freeze 上执行。`host.json`、`wasm-build.json` 保留实际
命令、工具版本、退出码、时长和完整日志。`wasm-tests.json` 与 `wasm-jit.json`
保留其原始执行源码身份，不被改写为后来重新执行。

最终平台 507 项在模型复核和 C5 异步测试修正后执行。之后仅修改了原生 Rust
integration-test 的 LCD147 容量 fixture。重新构建的生产 WASM 与原 24+2 步
测试所用 artifact 逐字节相同；平台 237 项输入和 128 项 harness 也保持不变。
[WASM 与平台证据连接](wasm-equivalence.json)明确记录三个源码修订和检查边界，
没有重复执行无关门禁，也没有把旧执行日期改成最终构建日期。

当前 855 个 Rust/Cargo 文件的 manifest SHA-256：

```text
7348dc64949ddb04177a42ac70b798358a220e71857239252abbf2d8b08a77bc
```

四目录 manifest SHA-256：

```text
959710f0e2ae5666d534d20db52d4ceed89b283d854ed196d3f1b815abefb206
```

生产 WASM 为 8,494,328 字节，SHA-256：

```text
5b91383f70d44d6a960dc475850a06340dc59e3c0fd830d4a87c5b8606d711a0
```

WASM 为构建产物；补丁保留源码、固定输入说明和生成流程，用户按项目文档重新
构建。49 项固定输入的来源与 hash 见 `pinned-inputs.json`。没有把新的第三方
固件镜像复制进交付仓库，也没有更改 golden 输出。

## 原生性能对照：完整适配栈对固定基线

基线为 `9e0f09c3f5507e0c50be1bb69d5653a40b42a3d4`，候选为上述最终源码。
双方使用 Rust 1.100.0、未修改的 release profile、jobs=2、incremental=0。
每个工作负载先执行一对不计入统计的预热，再交替执行三对计时样本，共 32 次运行。
每次都执行 30 guest seconds；只有正常 `Halted`、完整 guest cycles、逐核指令数、
异常/中断数和 stdout 字节完全一致才接受。所有流均拒绝 fatal 标记。

| 工作负载 | 基线 CPU 中位数（秒） | 候选 CPU 中位数（秒） | CPU 时间变化 |
| --- | ---: | ---: | ---: |
| S3 Hello | 0.583741 | 0.578017 | -0.98% |
| C3 Hello | 4.999073 | 5.746147 | +14.94% |
| C6 Hello | 3.197150 | 3.465099 | +8.38% |
| Pocket Tank | 179.030498 | 177.541310 | -0.83% |

正值表示 CPU 时间增加。**本次 C3 / C6 结果存在回退，不能宣称零性能开销。**
这是单一共享 Linux x86_64 主机上的三对测量；没有浏览器或其他 CPU 的性能结论，
也不能把整套 001–030 相对基线的差异归因于第 030 批或某一条具体修改。
所有原始样本、输入/可执行文件 hash、完整输出、失败判定和不确定性保存在
[native-perf.json（gzip）](native-perf/native-perf.json.gz)。它记录正确性一致和测量结果，
不是性能无回退的验收声明。相关既有机制为 EX114 / EX094，结果补充在
`docs/experiments.md` 的 EX114 条目中。

## 修正与保留的负面结果

- 普通 classic ESP32 CLI 恢复原来的 0 / 4 MiB PSRAM 契约；精确 Waveshare 型号
  继续按资料允许 2 / 8 MiB 并拒绝容量错配。定向 5 项检查通过，随后完整 suite
  也通过。见 `classic-cli-regression/` 与 `classic-cli-correction.json`。
- LCD147 的旧 canonical-chip 测试统一使用 1 MiB Flash，与精确硬件 4 MiB
  不符。更新为合法容量，保留 CPU/SPI2 断言，增加三个名称的 36 个错误容量
  拒绝案例。定向 8 项检查通过；没有放宽 runtime 校验。见 `c6-boundary-regression/`。
- 补齐 `USB-C · 电源/程序下载` 的英文词典条目，修正第一次平台检查的缺项。
- C5 测试将原有统计断言的条件加入等待谓词，等待独立的 Worker stat 消息；
  UART/LED 检查、原 30 秒期限及后续断言保留。最终 507 项验证通过。

前两轮 host 失败记录、平台早期失败及一次未完整采集的退出状态均保留，未
补造缺失的退出码。浏览器 headless 启动被 Unix socket 限制阻塞；另一次受支持
Browser 工具访问 loopback 预览返回连接拒绝及 URL policy 拒绝。两次都没有
实际完成页面/WebGL 检查，详见 `browser-final16/` 与 `browser-cua-assessment/`。

固件输出的 CRLF、尾随空格和末尾空行在原始交付记录中保持原字节；日志仅
规范化本机路径及 `ls -l` owner/group 字段，原 hash、保留 hash 和测量值分别记录。
当前树保留结构化结果与日志 hash，原始日志和审图可从精确交付提交或公开补丁
恢复。大 JSON 以 gzip 无损保留，解压字节未改动。详见[保留清单与恢复步骤](../waveshare-batch030-import-20261124/README.md)。
`log-ownership-normalization.json` 与输入保留记录仍说明来源阶段的具体变化。
公开厂家来源保留原有权益和来源含义，不能把公开可访问误写成 MIT 授权。

## 复核入口

准备项目文档要求的 ROM / 固件输入及 Rust 1.100.0 后，可用本目录
`run-final-ci.py` 分别运行 `wasm-build`、`host`、`wasm-tests`、`wasm-jit` lane。
将新的 `--output` 指向独立目录，避免覆盖这些历史收据。`run-native-perf.py`
要求两个分别以同配置 `cargo build --release --bins` 构建的工作区；运行时停止
其他构建和测量。模型复现步骤见上面的两份模型 README。

补丁的压缩/展开 SHA、父提交和每批目标树以 Sites 的 `manifest.json` 为准。
下载包中 `apply-waveshare.py --check` 先验证所有待应用批次，`--apply` 再写入索引和
工作区。从基线开始时检查完整30批；已经精确应用到029时，只需030补丁与最新
manifest 即可接续，保留同目录的应用脚本。`verify-chain.py` 提供另一个
私有 Git index/object store 的独立重放检查，并绑定最终源码、四目录和官方 SKU
集合。最终第 030 批的 tree 及网络回读事实只在完成导出和下载后填写到 Sites。
