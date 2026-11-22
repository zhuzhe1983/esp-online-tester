# LilyGO 原生 CPU 回归检查 — 2026-11-22

三个 hello 场景的指令数、周期、异常、中断及控制台输出均保持一致。计时中保留了 **C3 用户 CPU 时间中位数增加 5.272%** 的结果；并行构建和样本波动使这次测量无法确定增量来自哪项改动，不据此宣称没有性能回归，也不把 S3 的下降解释为已证实的加速。

完整样本、输入与可执行文件 SHA-256、源码摘要、运行顺序及负载记录见 [JSON 收据](lilygo-performance-2026-11-22.json)。复现工具为 [bench-lilygo-cpu.py](../../../tools/bench-lilygo-cpu.py)。

## 源码与构建

- 原版：`573f5a6c1bcc6dbf0388383d178a3935ec8ddcce`，干净的独立 worktree。
- 候选：`0cfd601b035f4fb84bc034b1c4f16bc7066f0e77`，加上当时已完成但未提交的 C5 板型、C5 attach、S3 板型注册及 WASM 注册文件。六个文件的完整内容哈希在 JSON 的 `source.candidate.uncommittedRustFiles` 中；不是仅凭 HEAD 推断候选代码。
- 候选的全部 621 个 Rust / Cargo 文件摘要为 `c6b05af6e6ffef445814d609bee5d7456a59cc292ae267324a4bd06c015a23e8`。冻结前后及采样前均核对该摘要与集成仓库相同。
- 两版各自运行 `CARGO_BUILD_JOBS=1 cargo build --release --bins`，均成功。工具链为 `rustc 1.100.0 (0ef1bc386 2026-11-09)`，Linux x86_64；相同 release 配置为 opt-level 3、fat LTO、codegen-units 1、debug 1，没有 RUSTFLAGS 或 release profile 环境覆盖。

这项检查按 [AGENTS.md](../../../AGENTS.md) 和 [CONTRIBUTING.md 的 Speed 要求](../../../CONTRIBUTING.md#speed) 执行。C5 bus 只在 attach 时发布已声明输入；C6 新增的 `refresh_board_outputs` 只在板型选择 GPIO PWM 时进行冷查询。S3 CPU、S3 bus 和共享调度代码未修改，因此本轮没有新增 Pocket Tank 测量。已查阅 [EX201](../../experiments.md#ex201) 的物理接线与冷回调、[EX220](../../experiments.md#ex220) 的布局与计时限制；本轮是集成回归检查，没有重新尝试 S3 执行优化。

## 测量方法与条件

每个场景执行 30 秒模拟时间。每个版本先执行一次预热，随后固定进行五轮 AB / BA 交替配对；6 个预热和 30 个正式样本全部保留，没有选择性删除或为取得更好数字而重复测试。

`resource.getrusage(RUSAGE_CHILDREN)` 在单个子进程前后取差，统计用户 CPU 和系统 CPU 时间；墙钟单独记录。脚本检查正常退出、唯一 stop 行、`Halted`、30.000 秒，并逐次比较精确工作量与 stdout SHA-256，任何差异都会失败。

采样期间另一个 `CARGO_BUILD_JOBS=1` 的全工作区 release 测试构建仍在运行，开始时正在链接 ESP32-P4。没有安排额外的重型检查，但这不是独占或空闲主机。JSON 保留每次运行前的聚合负载；未保留主机名称、进程清单、登录信息或无关命令。原始 stdout / stderr 留在忽略的 `target/lilygo-perf/raw/`，收据保存其哈希和必要统计。

## 用户 CPU 时间

单位为秒；变化按“候选中位数 / 原版中位数 − 1”计算。

| 场景 | 原版中位数 | 候选中位数 | 变化 | 候选较慢配对 |
|---|---:|---:|---:|---:|
| S3 hello | 0.617607 | 0.593717 | −3.868% | 1 / 5 |
| C3 hello | 5.661374 | 5.959861 | +5.272% | 4 / 5 |
| C6 hello | 3.760686 | 3.778039 | +0.461% | 5 / 5 |

| 场景 | 原版样本范围 | 候选样本范围 |
|---|---:|---:|
| S3 hello | 0.525205–0.814886 | 0.549249–0.616713 |
| C3 hello | 5.565924–5.999927 | 5.520776–6.737926 |
| C6 hello | 3.557885–4.541086 | 3.696434–4.659967 |

C3 最后一次候选运行墙钟为 9.951204 秒、用户 CPU 为 6.737926 秒；该样本和其余波动均保留。C3 增量尚未解释，C6 的五次配对也均较慢。这里报告实际测量值与限制，不将并行负载作为消除负面结果的理由。

## 精确工作与输出

下列值在每个场景的两版全部 12 次运行中一致；三个场景的 stdout SHA-256 也分别逐字节一致，完整哈希见 JSON。

| 场景 | 执行指令 | 模拟周期 | 异常 | 中断 |
|---|---:|---:|---:|---:|
| S3 hello | core0 11,934,783 + core1 6,775,776 | 7,200,000,000 | 27,229 | 6,032 |
| C3 hello | 4,800,000,000 | 4,800,000,000 | 0 | 3,028 |
| C6 hello | 4,800,000,000 | 4,800,000,000 | 0 | 3,029 |

## 复现

准备两个源码目录及 `web/wasm/fw/` 中现有的三个 hello manifest、ROM、bootloader、分区表与 app；使用收据中的哈希核对输入。候选目录必须包含上述六个文件的记录内容，或使用 Rust / Cargo 源码摘要一致的后续集成提交。分别在两版目录执行相同的 release 构建，再运行：

```sh
python3 tools/bench-lilygo-cpu.py \
  --baseline "$LILYGO_BENCH_BASE" \
  --candidate "$LILYGO_BENCH_CANDIDATE" \
  --assets "$LILYGO_BENCH_ASSETS" \
  --output target/lilygo-perf/reproduction.json \
  --raw-dir target/lilygo-perf/reproduction-raw \
  --rounds 5 \
  --conditions 'Describe the actual concurrent work and host isolation here'
```

测量的是现有 hello 的兼容性与原生执行开销，不是新增设备的厂商固件启动证明，也不替代浏览器、WASM、显示和按钮的独立验证。
