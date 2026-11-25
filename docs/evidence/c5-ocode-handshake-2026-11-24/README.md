# C5 OCODE：显式理想参考握手

本模块让未改动的 C5 软件实际执行内部 I²C target `0x61` 的 RESETB 与 DONE 轮询协议。仅在显式 `enable_virtual_ocode_ideal(cycle)` 后，真实 write0→write1、实际 CPU XTAL observation、实际 PLL power/ICG 关闭共同启动有限事件。24,000 nominal cycles 后才产生 reg3 bit0/bit3；这100us是声明的稳定理想参考/target clock策略，不是测得的模拟收敛时间。没有提供数值 OCODE、电压、校准精度、真实熵或校准 eFuse。

固定来源见 [sources.json](sources.json)：IDF5.5.3 `2c211b236707889e8400c4dc5644dd5c4ee071e0` 与5.5.4 `735507283d5b2f9fb363a1901172dbd9e847945d`。[调用流程](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/esp_hw_support/port/esp32c5/ocode_init.c#L30-L93)、[target字段](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/soc/esp32c5/include/soc/regi2c_lp_bias.h#L18-L55)、[HP route及master选择](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/esp_rom/patches/esp_rom_hp_regi2c_esp32c5.c#L11-L173)确定数字协议。相关源码对一致；`soc_caps.h`完整文件在MSPI/BLE宏上有版本差异，本路径root-gate定义一致。C5 TRM1.1没有公开此target的POR、内部收敛时钟/算法或保留域。TRM映射与实际HP `0x600af800` SDK地址覆盖存在缺口；不添加LP地址别名。

| 接口/字段 | 严格合同 |
| --- | --- |
| `enable_virtual_ocode_ideal(cycle)` | 默认关闭，重复启用保留协议历史；显式理想参考、target clock和100us策略。target reg0初值0是virtual，不能代替guest写0。 |
| `observe_ocode_cpu_xtal(cycle, bool)` | 父级注入实际**committed CPU root=XTAL**，不能只比较48MHz频率，也不能使用`!pll_ready`。先用旧observation结算到cycle，再观察新值。 |
| 启动quiet guard | 实际WT latch bitmap：bit0 GLOBAL_BBPLL_ICG、bit2 XPD_BBPLL_I2C、bit3 XPD_BBPLL均关闭，即`power & 0x0d == 0`；bit1 BB_I2C可保留。PLL profile未启用时不认为quiet。固定LL的disable确实清这三项。 |
| HP master事务 | ANA_CONF1=`0x00fffeff`；ANA_CONF2 bit10=1选CTRL0，=0选CTRL1，HOSTID不决定master。沿用实际MODEM/APB/I2C gate。每次成功命令真正读/写target后才完成；RO BUSY输入位被忽略，结果BUSY0，不占用整个校准等待期。 |
| target reg0 | 仅bit0；每次合法write0清flags/撤deadline，实际write1只在已reset且guard满足时开始。无前序0、重复1、溢出、其他控制位拒绝且无半启动。 |
| target reg3 | 仅读派生flags，deadline前0、有效到期后9；两个字段在此虚拟策略中同时完成。读本身不生成成功，也不重启计时。写拒绝。 |
| target reg4/5/6及其他byte | 读写均unsupported：不提供result、FORCE_CODE或EXT_CODE，不选择eFuse旁路。 |
| `deadline/pump/read_at/write_at` | 与现有PLL共用最早deadline；O(1)工作，无每cycle循环。写入先结算旧quiet输入，再观察实际新power。CPU/quiet在运行中丢失则取消，须新write0后才能重试；Done后恢复CPU/PLL保留flags。route/master gate只限制后续事务，不冒充target clock。 |
| `disable_virtual_ocode()` | 撤profile、target可达性、state/flags/deadline及OCODE诊断；不改变其他target/PLL状态，不重置已有controller配置。`virtual_ocode_enabled()`为纯getter。 |

RESETB0/1时间、启动所用CPU observation、状态/deadline/attempt/completion/cancellation、first/last reg3 `(nominal_cycle, byte)`及poll数量均为固定大小诊断。report还暴露实际power bitmap、route/selector和两个controller最后word。`cancel_observed_at`是模块最后结算/观察时点；`reset_domains`本身不接收reset timestamp，不能把该字段当作硬件reset到期时点。

复位策略是明确的**virtual functional retention**，没有宣称硅片保留保证：CPU-only保留；HP/Core清现有master、取消未完成序列并撤CPU observation，要求新RESETB，已完成flags保留；LP清target/flags/CPU observation，保留累计诊断；analog再清累计诊断。以上均保留profile选择；raw/SRAM deselect撤profile。父级必须先处理更早或同刻watchdog reset，并禁止pending reset之后继续pump；可在reset分支仅结算严格早于reset的OCODE事件，再调用reset_domains。模块不会替父级回滚已经错误pump到reset之后的历史。

源码中的10ms是**guest LP_TIMER超时窗口**，从保存/切换CPU及RESETB之前开始。`rtc_clk_cal(...,100)`真实非零period是前提，period0会assert；REGI2C BUSY轮询没有软件超时。该模块不替guest设RTC deadline、推进LP_TIMER、跳过warning或强造DONE。若CPU/quiet中途丢失且未重新RESETB，flags保持0，guest仍可按实际LP_TIMER走超时cleanup。目标校准时钟与CPU frequency缩放之间的物理关系未建模；全部计时沿当前nominal240MHz调度轴。

父级接线范围：strict原始ROM profile选中后启用；取得committed CPU root，先结算旧observation再注入；联合analog deadline加入scheduler；模拟电源写入前后遵循旧输入优先；reset优先；raw deselect撤profile。此提交不改ClockTree、Bus、Soc、lib、ROM/BIN、卡或eFuse。原厂动态起点与typed对应见 [checks.json](checks.json)：当时只到route store，RESETB和reg3尚未执行，不能把本组件测试当作工厂路径已越过校准函数。

复现只运行owned harness：

```sh
python3 docs/evidence/c5-ocode-handshake-2026-11-24/run-owned.py --repo "$REPO" --output-dir "$TASK_TMP"
clippy-driver --edition=2021 --test esp32c5/src/boot_analog.rs -o "$TASK_TMP/boot-analog-clippy-tests" -D warnings
node tools/check-evidence-privacy.mjs
```

[owned-tests.txt](../c5-evidence-retention-final-2026-11-25/stdout-manifest.json)为组件结果；[checks.json](checks.json)保留输入/源码/工具/输出hash、初始失败的错误BUSY断言及其更正说明。没有运行Cargo/shared build、原厂机执行、浏览器或性能benchmark。未提交完整原厂日志、私有路径或事务流；仅保留必要hash和数值，不影响本组件结论。

## Parent integration checkpoint

Parent Bus selects/deselects this profile with validated cold ROM, publishes only the committed CPU root, and processes an OCODE deadline strictly before a captured watchdog reset. Reset wins at the same nominal cycle; pending reset blocks later analog advancement. Five Bus fixtures cover reset protocol, staged/committed source changes, CPU/Core/LP retention, raw deselection, pending reset and late watchdog ordering. A separate clock fixture distinguishes PLL240/5 from XTAL even though both can yield48MHz.

Independent review found and closed one strict-off issue: the new observation initially pumped PLL even when OCODE was disabled. The wrapper now returns before pumping and Bus filters by enabled. A new running-PLL control checks exact state and report preservation at its deadline and at U64MAX. Actual power-latch diagnostics are also restricted to the enabled OCODE suffix so PLL-only console output stays unchanged.

The final focused C5 suite passes316 tests with all5 local-input cases supplied; both warnings-denied native/workspace and release-WASM Clippy commands pass. Focused release explicitly uses LTO=false. A retained earlier failure exposed a SAR fixture taking its rollback snapshot before elapsed Analog time was settled; the fixture now settles both old inputs before taking snapshots and preserves strict equality. The initial Clippy missing-else formatting failure and fixes remain in the receipt. These are component/integration checks, not final full-workspace CI.

Unchanged original ROM and factory code now execute RESETB0 at20622656 and RESETB1 at20622976. The first reg3 read is0 at20623232; the last is9 at20647488, with82 reads, one attempt/completion and no cancellation. The virtual deadline is20646976. Direct CPU-only observations at caller-supplied PCs capture real period3855046(Q19), timeout1360slowticks, cycle0=11446 and deadline12806. The guest reaches common cleanup at40801504/20647616 and returns from CPU restoration at4080152c/20672640; final PLL is ready with2 attempts/2 locks and source3 CPU/AHB/APB80/40/40MHz. No firmware or target result code was substituted.

The actual next boundary is the original PSRAM SPI write/read connectivity check, before ID reading: UART reports not connected or wrong line mode and Failed to init external RAM, then an exception. The run has20748961 instructions,20748928 nominal cycles,2046 UART bytes and no access firstFault. It has not entered app_main or passed the full PSRAM test; LCD and TF output remain absent. [integration-checks.json](integration-checks.json) retains exact source/input/log hashes, commands and limitations. Final native/product/WASM/browser/full-CI/CPU/fresh acceptance work remains.

完整逐 test stdout 已移至 ignored 输出；原路径、bytes/hash、固定 artifact/source receipt 与重现命令见[留存清单](../c5-evidence-retention-final-2026-11-25/stdout-manifest.json)。测试结果、负结果和历史 raw/normalized hash 保持不变；此留存整理没有运行组件或原厂机。
