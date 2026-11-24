# Waveshare 001–029 合并验证

29 批交付按编号下载、校验并应用后，统一运行检查。逐批来源哈希、精确 tree 和本地提交见[续接记录](../../waveshare-patch-import-20261124.md)。原始交付终点为 `78ac0278f7418556f4afa3327b5b6ce28ae453c9`；集成包含已发布的 Cloudzao 主分支 `d148ef4e85aced5b18a69e03a539d77d74bc7df4`，验证修复和发布代码提交为 `738dac99e1915aa9674fbc4799188ec13406d332`。

[检查收据](checks.json)保存命令、结果与哈希，[模型收据](source-model-checks.json)记录 244 个不同交付模型的检查结果。完整日志、截图和模型几何检查明细保留在忽略的本地输出中。

## 正确性与发布

- 使用 stable Rust 1.100.0；原生和 WASM Clippy 均以 `-D warnings` 通过。
- 全工作区 release 测试包含 ignored、排除 `external_`，共 2,895 项通过；未重建 golden 输出。
- 初次 520 项平台检查通过，目录聚合快照修复后复验 521 项通过，另验证真实录制、15 个 WASM demo、Worker、输入、JIT 差分及 CI 的其余契约。
- 244 个交付模型通过实际 Three.js 加载器、来源图片哈希、尺寸和几何射线检查；真实 Chromium 覆盖九类芯片代表、2D/3D 切换及 390 px 嵌入页。
- 完整本地发布目录为 775,521,924 字节；522 个模型的解码后几何字节、元数据和来源记录保持一致，并核对 233 份贴图与 1,260 项参考照片引用。传输格式与复现命令见[发布说明](../../pages-model-assets.md)。
- 打包后的 C5 LCD147 运行原始第一方 SRAM 诊断：172 × 320 帧缓冲、BOOT 按下改变 35,840 个像素、释放后逐字节恢复，UART 报告 SD 原始块已恢复。此结果验证该诊断的执行和输入路径。

最初三次 Rust 测试失败来自旧测试的容量假设：classic ESP32 新增 2/8 MiB 配置，而 LCD147 别名现在要求物理 4 MiB。修正测试输入并增加拒绝错误容量的断言，保留实现中的严格校验。初次发布目录超过体积限制，改为无损传输；元数据序列化曾把负零改成正零，修正后重新检查了全部模型。

## 部署前目录快照修复

首次 Pages 运行在固件目录同步时触发 2 MiB 快照上限，构建与部署尚未执行。完整聚合快照包含新增厂商/SKU，原来的紧凑 JSON 编码仍不足以适用旧上限。现由浏览器和同步工具共享有限的 8 MiB 聚合快照上限；普通 watchlist 读取仍限制为 2 MiB，各上游请求保持原有界限，本次修复保留已有版本、哈希和历史。真实发布源 dry-run 的完整快照为 3,736,886 字节，未更新固件或下载固件正文。增加大于 2 MiB 的多设备完整快照往返，以及 header/流式超限取消读取检查。[修复检查收据](publication-followup.json)保留失败运行和真实发布源 dry-run 的结果。

## 原生 CPU 对比

延续 [EX201](../../../experiments.md#ex201) 的物理总线/板级回调正确性检查，并参考 [EX206](../../../experiments.md#ex206)、[EX212](../../../experiments.md#ex212)、[EX235](../../../experiments.md#ex235) 的布局和测量限制。本次不同之处是 Waveshare 全批次与 Cloudzao 的集成状态、板型和输入边界；没有提出新的执行优化机制。

[完整 CPU 收据](cpu-main-comparison.json)保存一个预热及三个交替测量对的全部样本、二进制与输入哈希、源文件清单哈希和匿名负载。两个版本均使用 `CARGO_BUILD_JOBS=8 cargo build --release --bins`，每个工作负载运行 30 秒 guest 时间。计时前关闭任务浏览器并停止任务构建与打包；共享主机未做专用隔离。

```sh
python3 tools/bench-lilygo-cpu.py \
  --baseline <published-main-checkout> --candidate <integration-checkout> \
  --assets <integration-checkout>/web/wasm/fw \
  --output target/cpu-main-comparison.json --raw-dir target/cpu-raw \
  --rounds 3 --workloads hello c3-hello c6-hello pocket-tank \
  --timeout-seconds 600 \
  --build-command 'CARGO_BUILD_JOBS=8 cargo build --release --bins'
```


| 工作负载 | 主分支 user CPU 中位数 | 集成版中位数 | 变化 | 集成版较慢的测量对 |
| --- | ---: | ---: | ---: | ---: |
| hello | 0.234875 s | 0.235132 s | +0.109% | 1/3 |
| c3-hello | 2.525060 s | 2.508318 s | -0.663% | 1/3 |
| c6-hello | 1.542647 s | 1.562524 s | +1.288% | 3/3 |
| pocket-tank | 41.022527 s | 40.159980 s | -2.103% | 2/3 |

全部样本的 guest 指令数、周期数、异常、IRQ 和 stdout 哈希一致。小幅 CPU 差异保持为未归因的观测结果，不据此声明加速或零开销。模型及目录检查不代表全部原厂固件、实物装配或模拟外设的物理精度均已验证；保留各产品已有的试验状态和限制。

## 复现边界与隐私

模型检查使用 `node tools/check-waveshare-model-loading.mjs --output target/models.json <model-ids...>`；244 个 ID 可由模型收据提取。浏览器夹具为 `tools/fixtures/waveshare-models-browser-check.js` 与 `tools/fixtures/waveshare-c5-packed-browser-check.js`，后者要求调用方通过 `?firmware=` 提供仓库内原始诊断地址，并校验固定 SHA-256。

提交内容仅保留命令、数值、哈希和结果。原始日志中的本地路径、浏览器连接信息和截图未导入证据；这些字段的省略不改变保留的 guest 计数、输出哈希及 CPU 样本。发布前运行证据隐私检查并人工审阅新增文本。
