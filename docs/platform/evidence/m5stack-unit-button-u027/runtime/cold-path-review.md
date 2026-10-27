# U027 输入路径的布局审查

`ee3d47d087276db896839a751e47f59f536b8d8f` 的功能验证通过，但 C6 hello 曾测得 +1.30% / +2.48% 的 CPU 中位数偏差。本次做了一项有明确编译结果的小改：附件消息只在当前实际支持它的 S3 上编译分派，JSON 字段解析移至 `#[cold] #[inline(never)]` 独立函数。**这消除了 C6 中重复的附件解析代码，仍未关闭 CPU 偏差：最终一次候选测量为 +1.875%。**

旧的 [cpu.json](cpu.json)、[cpu-c6-repeat.json](cpu-c6-repeat.json) 和构建条件不一致的 [cpu-initial.json](cpu-initial.json) 原样保留。不能将本次记录描述成旧回归已消失或性能无回退。

## 与既有实验的区别

先审查了 [EX220](../../../../experiments.md#ex220) 和 [EX212](../../../../experiments.md#ex212)。EX220 曾通过冷设备合并分配恢复共享数据尺寸和热字段偏移；EX212 记录了不同内联方式、代码布局与机器之间不稳定的性能结果。本次不是重试其 DMA、缓存或通用 tick 优化。U027 的新增状态本来就在 StopWatch 独立的 `Arc<Mutex<PanelState>>` 中，没有进入 C6 的共享状态；因此没有凭猜测再改 PMIC/板级状态分配。

本次唯一实现改动在 `esp-soc/src/machine/web.rs`：`S::NAME == "esp32s3"` 编译期条件，加一个非泛型冷解析函数。接线、GPIO 驱动/释放、M5PM1 供电、重启和录制行为不变。其他 SoC 原本也没有支持该附件的 board callback，门控没有关闭已支持功能。

## 静态证据

双方均用 Rust 1.99.1、`cargo build --release --bins`、仓库 fat-LTO 配置；基线为 `7d03d2409912516030412d715454da8dd8e4bc93`。最终编译产物的反汇编与静态审查产物逐字比较一致，输入与源码 hash 见 [codegen.json](codegen.json)。

| C6 函数的归一化指令数 | 基线 | ee3 | 冷解析候选 |
| --- | ---: | ---: | ---: |
| `web_poll_input` | 1730 | 1998 | 1730 |
| `web_push_due` | 2832 | 3092 | 2832 |

54 个选定 C6 专属函数（包括上述输入/推送函数）在候选中全部匹配基线。仓库 `hot.py` 的较宽选择器另匹配 130 个编译进 C6 可执行文件的共享/S3/C3/C6 函数；这 **不是 130 个 C6 专属函数**。ee3 的这 130 项本来就没有归一化差异。

比较复用了仓库已有 `asmcmp.py`：保留操作码、寄存器、栈/字段偏移和其他立即数，归一化指令地址、分支/调用地址及其偏移、ADRP 常量页偏移、nop/udf 填充；合并函数别名按被调用体形状比较。它说明选择范围内的代码形状一致，不是对所有跳转目标或常量内容的形式化等价证明。绝对函数入口地址另行记录：它们仍有变化，不能由归一化一致推出缓存行为或 CPU 时间一致。

[layout.rs](layout.rs) 编译到各自 release rlib，实测 75 个公开字段偏移和三个类型的大小/对齐共 78 行输出完全相同：`Machine<C6>` 11272 bytes，`SocBus` 10264 bytes，`Peripherals` 8712 bytes。详见 [layout.json](layout.json)。私有字段没有逐个直接探测，热函数中的字段位移提供补充证据。新增 Board trait 方法追加在末尾；既有热调用位移保持不变，但 vtable 多出一个入口，并不等于整个镜像布局不变。

复现命令：

```sh
xcrun llvm-objdump -d --demangle --no-show-raw-insn "$BINARY" > "$DISASSEMBLY"
python docs/platform/evidence/m5stack-unit-button-u027/runtime/compare-codegen.py \
  . "$BASE_DIS" "$EE3_DIS" "$COLD_DIS"
# layout.json 记录了针对各构建 rlib 的 rustc 命令。
```

## 一次候选测量与检查

[C6 样本](cpu-cold.json)：同一固件、30 客体秒，一组预热不计入中位数，随后七组交替 base/candidate 与 candidate/base。每次都是 **4,800,000,000 指令**，客体标准输出 SHA-256 相同。没有本任务的编译、浏览器或其他客体测试重叠；共享机器的其他负载未控制，保留了每次采样前的负载数值。

| CPU user 秒 | 基线 | 候选 |
| --- | ---: | ---: |
| 中位数 | 1.60 | 1.63 |
| 范围 | 1.52–1.67 | 1.60–1.66 |

候选 5/7 组较慢、1 组相同、1 组较快，中位数 **+1.875%**。范围重叠不构成将偏差归为噪声的依据。尚无证据把偏差具体归因到缓存、代码地址、调度或其他机制。

两项 Clippy、1361 项 Rust、60 项 Node、11 个真实 WASM 场景和 U027 的九项真实客体检查重新通过，详见 [checks-cold.json](checks-cold.json) 与 [fixture-cold.json](fixture-cold.json)。没有改 golden，没有减少客体指令或省略输入行为。UI/CAD 没有改动，原有浏览器和模型证据保留；新的运行证据带最终 WASM/source hash。

本轮只给这个有明确代码变化的候选做了一次 C6 计时，没有继续猜测性调整或无变化重跑。ee3 的四工作量记录仍是历史结果，不能冒充本候选的四工作量新测量。

## 整合取舍

冷解析改动可以保留，避免将单个 S3 产品功能复制到每个芯片的 host 输入函数。但 CPU 标记仍未关闭。主线程可以在明确接受约 2% 的本机 C6 实测代价后整合本功能；若必须先满足无 CPU 回退，则应继续保留分支，而非将现有证据改写为通过。

若继续调查，下一项应收集能区分具体原因的新证据，例如相同 workload 的硬件采样/缓存或分支计数；或明确评估按芯片拆分可执行代码的架构成本。现有证据不支持任意改对齐、反复切换内联、移除硬件行为，或把功能只在计时构建中关闭来获取更好数字。
