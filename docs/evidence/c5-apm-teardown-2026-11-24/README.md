# C5 app 启动的 APM 关闭与 TEE0 配置

`BootApm` 只支持原厂当前实际进入的 `bootloader_init_mem()` 配置路径：guest 清四组 SYS_APM filter，再真实写15个 HP_TEE mode 和1个 LP_TEE mode 为0。没有权限区域执行引擎、REE支持、lock、IRQ/status或整片RegRam。当前原厂停在第一词写入请求；此提交的组件测试不代表原厂已执行后续20词、PMA、app_main或LCD。

固定 IDF5.5.3 `2c211b236707889e8400c4dc5644dd5c4ee071e0` 与5.5.4 `735507283d5b2f9fb363a1901172dbd9e847945d` 的 [bootloader_mem.c](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/bootloader_support/src/bootloader_mem.c#L18-L48) 明确在非bootloader app阶段关闭四组filter，再 `apm_hal_set_master_sec_mode_all(APM_SEC_MODE_TEE)`。[HAL](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/hal/apm_hal.c)、[C5 LL](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/hal/esp32c5/include/hal/apm_ll.h) 及 [apm_defs.h](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/soc/esp32c5/include/soc/apm_defs.h#L14-L45) 确定真实寄存器和master集合。40份抓取的URL、final URL、UTC、类型、长度与SHA见 [sources.json](sources.json)。20对中19对相同；`cpu_start.c`有Picolibc TLS初始化、flash-tuning guard差异，本APM调用段相同。两个猜测路径404亦保留为失败发现，不能当作抓取成功。

| 精确地址 | 初始配置/支持操作 |
| --- | --- |
| `0x600990c4` HP_APM FUNC_CTRL | header POR `0x1f`，5个路径；仅literal0写关闭。 |
| `0x600998c4` LP_APM0 FUNC_CTRL | POR1，1个路径；仅literal0写关闭。 |
| `0x600b38c4` LP_APM FUNC_CTRL | POR3，2个路径；仅literal0写关闭。 |
| `0x6009a0c4` CPU_APM FUNC_CTRL | POR3，2个路径；仅literal0写关闭。 |
| `0x60098000 + 4*master` HP_TEE MODE_CTRL | 仅master `0,1,2,3,4,5,6,8,17,18,19,22,23,24,25`，即mask `0x03ce017f`。四FUNC全真实关闭后只接受0；实际写前read None，写后read0。 |
| `0x600b3400` LP_TEE M0_MODE_CTRL | LPCORE还有这个独立target；四FUNC全关闭后只接受0，实际写前read None、写后read0。 |

这是20个具体word，不覆盖地址空洞、区域边界/权限、TEE peripheral权限、状态、IRQ、clock gate或Cache别名。`reg_base.h`明确HP_APM base=`0x60099000`，Cache base=`0x600c8000`。任何非零写，包括idempotent active filter、partial clear、REE0/1/2、mode lock、保留位，都事务拒绝。不会为了接受SDK的全开 `UINT32_MAX` 而忽略真实enable请求。

POR确实开启SYS_APM，region0默认启用、全地址、REE权限0，不能称为inactive。[C5 TRM1.1](https://www.espressif.com/sites/default/files/documentation/esp32-c5_technical_reference_manual_en.pdf) 18.5.1/18.5.2.2说明HP CPU机器态为TEE，TEE可读/写/执行地址范围；其他master冷启为REE2、访问默认被阻。机器态CPU因此可以执行当前关闭序列。HP user mode配合TEE_M0_MODE0实际仍是REE0；不能因mode字段0就把用户态当作机器态。

公开来源存在明确缺口：HP `tee_reg.h`列所有mode字段默认0，TRM和`bootloader_mem.c`注释却写其他master冷启REE2；LP_TEE header明确默认3。本模块不解决这个effective-world POR冲突。world读和effective getter在实际guest write0前保持unsupported/false，绝不host seed“全TEE”；FUNC读取仍保留一致的header POR。LP master1必须同时有HP和LP两个实际0写入。

API为 `new()`、`contains(address)`、`read(address,cpu_machine)`、`write(address,value,cpu_machine)`、`all_filters_disabled()`、`all_masters_tee_selected()`、`master_tee_selected(id)`、`access_supported(master,cpu_machine)`、`reset_domains(hp,lp,analog)` 和 `report()`。read/write的`cpu_machine`必须由父级实际执行约束提供，false无条件unsupported，不能使用MSTATUS.MPP替代current privilege；本模块没有隐藏Machine=true默认值。`access_supported`是已支持上下文边界，非硬件denied-read/exception实现：HP CPU master0只支持实际机器态；其他已知master须四FUNC全关闭且其target实际选择TEE0，未知master永不支持。父级可在DMA边界更严格地同时检查all-filters-off和all-masters-TEE-selected；当前prefix在该序列前没有DMA，不能把这一事实推广为所有固件。

父级另行提供C5 machine-only核心约束、CpuObserver上下文、DMA与Bus接线，以及strict原始ROM profile选择/退出。APM对象本身没有隐式ROM、PC或eFuse识别，也不观察固件PC来跳过指令。关闭profile时父级撤路由/对象；未选择时此新私有模块不参与旧执行。此提交未改Bus、Soc、lib或核心。未具备可靠CPU约束的集成状态不能把bool默认设true后宣称安全支持。

复位为显式functional retention：CPU-only保留；HP/Core恢复HP_APM、LP_APM0、CPU_APM的FUNC POR并撤HP_TEE实际写入标记；LP恢复LP_APM FUNC POR并撤LP_TEE标记；Chip全恢复并清诊断。HP/LP reset保留累计真实写次数。公开资料没有完整per-flop保留表，因此不声称已验证硅片域细节。PCR_RESET_EVENT_BYPASS_APM是bit0，区别于system-bus bypass bit1；当前BootAnalog仍拒绝APM bypass非零，此模块不能把bit1当作APM复位选择。

typed入口、每个word、来源hash和实际父级运行回执见 [typed-sequence.json](typed-sequence.json) 与 [checks.json](checks.json)。`0x4206e910`先以a0=0调用`0x420710b4`（四FUNC清0），再a0=0调用`0x420710a6/0x42071044`（全部mode0），然后跳到`0x420713ac`的PMA CSR设置。PMA/PMP与CPU monitor是独立scope，本模块没有声称实现。

复现只运行owned harness：

```sh
python3 docs/evidence/c5-apm-teardown-2026-11-24/run-owned.py --repo "$REPO" --output-dir "$TASK_TMP"
clippy-driver --edition=2021 --test esp32c5/src/boot_apm.rs -o "$TASK_TMP/boot-apm-clippy-tests" -D warnings
node tools/check-evidence-privacy.mjs
```

[owned-tests.txt](../c5-evidence-retention-final-2026-11-25/stdout-manifest.json)覆盖真实20词顺序、POR、显式current context、pre-init DMA/world拒绝、LP双target、未知地址、所有非零/enable/region/lock、复位、重复0计数和纯读诊断。没有Cargo/shared build、原厂机运行、浏览器或benchmark；完整native日志和源header留在忽略缓存，仅提交必要数值/hash/固定URL，不提交私有路径或进程信息。

## Parent integration checkpoint

The parent wires these20 exact words only under strict validated-ROM selection. A constrained Machine execution profile supplies actual CPU context before every instruction; saved MPP is not used as current privilege. The shared Cpu flag defaults off, and C5 selects it explicitly. Lower-privilege MRET and MPRV-setting instructions reject before committing their target state; legal Machine return, reads and reset remain supported. These are limits of this model, not a statement that C5 silicon lacks lower privilege.

CPU contextNone denotes host/component inspection and grants no APM-register access. A real observed but unconstrained CPU is rejected. DMA has a separate descriptor/payload boundary requiring all four filter disables and all16 actual TEE selections. Modeled DMA never borrows CPU context. Three real instruction fixtures execute all20stores, reject missing/unsafe context, and check owner reset/deselection; the core adds4 instruction-level tests with default-off controls.

Full focused checks pass331 C5 tests with all5 local-input cases supplied,90 RV32 tests with2 external-input cases filtered, and both native/workspace and release-WASM Clippy with warnings denied. Release tests explicitly use LTO=false, so final default-LTO workspace CI and CPU controls remain separate. The corrected early test assumption and original output hash are retained.

The unchanged original factory actually performs4/15/1 writes: all filters0, HP mask03ce017f and LP TEE selected, then reaches a real breakpoint at420713ac before the first PMA function instruction. The run records20820883 scheduler steps,20820882 CPU instruction-count slots,20820864 nominalcycles,2150 UART bytes and no firstFault. Source ROM/factory/prepared-card hashes are unchanged. This proves the APM startup sequence, not PMA/PMP support, full PSRAM selftest, app_main, LCD/buttons/TF or Wi-Fi. [integration-checks.json](integration-checks.json) records sources, phases, checks and limitations.

完整逐 test stdout 已移至 ignored 输出；原路径、bytes/hash、固定 artifact/source receipt 与重现命令见[留存清单](../c5-evidence-retention-final-2026-11-25/stdout-manifest.json)。测试结果、负结果和历史 raw/normalized hash 保持不变；此留存整理没有运行组件或原厂机。
