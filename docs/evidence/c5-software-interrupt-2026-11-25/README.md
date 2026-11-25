# C5 FROM_CPU0 软件中断

`SoftwareInterrupt` 实现精确地址 `0x600c5090` 的 R/W bit0 latch：POR0，写1持续拉起 interrupt-matrix source23，写0清除，读取不清除。重复写1不排队；clear后可以再次assert。其他位、未对齐地址、FROM_CPU1..3、DATE、CLOCK_GATE 等字仍 unsupported。非法写不改变 latch 或诊断计数。

固定来源为 IDF 5.5.3 `2c211b236707889e8400c4dc5644dd5c4ee071e0` 和 5.5.4 `735507283d5b2f9fb363a1901172dbd9e847945d`，四对完整文件逐字相同，URL/时间/bytes/SHA-256 见 [sources.json](sources.json)：

- [reg_base.h](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/soc/esp32c5/register/soc/reg_base.h#L116)：INTPRI 基址 `0x600c5000`。
- [intpri_reg.h L14–24](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/soc/esp32c5/register/soc/intpri_reg.h#L14)：FROM_CPU0 offset90、bit0 R/W、default0。
- [interrupts.h](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/soc/esp32c5/include/soc/interrupts.h#L41)：`ETS_FROM_CPU_INTR0_SOURCE` enum23，注明 level；1..3分别24..26。
- C5 实际 LL 是 [crosscore_int_ll.h](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/hal/esp32c5/include/hal/crosscore_int_ll.h#L18)，trigger写1、clear写0、get_state普通读取；单核目标不使用 core_id 选择其他字。固定553源码 tree 没有 C5 `intr_ll.h`，未借用其他芯片 alias。

[TRM 1.1](https://www.espressif.com/sites/default/files/documentation/esp32-c5_technical_reference_manual_en.pdf) §11.7.2/Register11.81 对应 bits31:1 reserved、bit0 R/W、reset0，明确0停止生成、1生成中断；interrupt-source table 给 FROM_CPU0 source23。这个字段在已核 SDK/TRM 之间没有发现冲突。它与 §2.8.3.3 的 CLINT MSIP `0x20000000` / CPU source3 不同，也不是 CLIC pending-register alias。

时钟范围：TRM 派生 HP clock 表的 INTPRI 行标 CPU_CLK；其 MMIO 属 HP APB4。helper 只在父级既有 CPU_CLK/APB 前提下更新功能 latch，不增加时间源或延迟。INTPRI `+a4` 的 CLK_EN POR1，但 header description 缺失，此提交不猜0的意义、不解码它；PCR_INTMTX `+98` 是另一字，不能作为 INTPRI functional gate。新的 gate/source/power 请求在其实际 owner 未实现前仍应故障。

reset 是声明的 functional retention：CPU-only/LP-only 保留 HP latch；HP 或 Chip/analog 恢复 POR0并清诊断。公开资料没有给出此位所有 per-flop reset/retention 细节，不宣称硅片验证。raw/SRAM deselect 时父级删除 optional owner；未选择 strict-ROM profile 的路径不分配组件或改变 legacy 行为。

静态原厂路径为 `4080c154 ->40802bf2/40802b92`，`40802be8` store1，`4080c174` 条件轮询；原 handler 在 `40802b48` store0。原 factory BIN SHA-256 `cf10a4358d03898458c16390f3f16ceb0306befe2260160de0919d755b920942`，typed segment1 SHA-256 见 checks。这些是静态指令证据，**该路径尚无本提交的 native 原厂执行结果**。纯测试中的 store/poll/clear fixture 不代表真实调度器已启动。

父级负责 source23 的实际 interrupt-matrix/CLIC 电平递送及 guest 选择的 CPU line。组件不预设 vector、不开 host yield、不改 CPU PC/SP，也不自动 acknowledge。读/poll、CLIC观察或重复assert不会消费 source level；只有真实 clear 或相应域reset撤电平。

复现纯组件检查：

```sh
python3 docs/evidence/c5-software-interrupt-2026-11-25/run-owned.py \
  --repo . --output-dir "$C5_SOFTWARE_IRQ_CHECK_DIR"
```

stable rustc `--test -D warnings` 编译同一文件，8项组件测试通过；standalone clippy-driver warnings denied 通过。覆盖POR/source23、原指令的set/poll/clear组件fixture、持续level、重复写/重发、所有reserved bit、整块无alias、CPU/LP保持和HP/Chip恢复、非法写原子性、计数饱和。没有 Cargo、workspace build、native guest、WASM、browser 或 benchmark。

基于 `9dedc474ab35401466c51318dba6ba04b882d200`，只新增此组件与证据；Bus/lib/Soc/Clock/CLIC 接线由集成修改提供。[checks.json](checks.json) 和 [owned-tests.txt](../c5-evidence-retention-final-2026-11-25/stdout-manifest.json) 保存结果及hash；调用者路径被脚本归一化为标签，没有复制私有 native 日志、用户路径或主机信息。

完整逐 test stdout 已移至 ignored 输出；原路径、bytes/hash、固定 artifact/source receipt 与重现命令见[留存清单](../c5-evidence-retention-final-2026-11-25/stdout-manifest.json)。测试结果、负结果和历史 raw/normalized hash 保持不变；此留存整理没有运行组件或原厂机。
