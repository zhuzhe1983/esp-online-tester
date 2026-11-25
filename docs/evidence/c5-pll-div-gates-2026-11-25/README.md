# C5 PCR PLL divider gates

PCR `0x60096128` 按原厂所用固定IDF5.5.3/5.5.4建模：九个derived SPLL gate，精确mask/POR=`0x1ff`。gate不是PLL readiness；只有实际analog `pll_ready` 且对应gate打开时，`pll_tap_hz(mhz)`才返回功能频率。当前CPU根使用的240/160M tap拒关；closed tap不能stage或commit为CPU根。mask以外位拒绝。48M USB gate暂拒关，因为现有USB引擎尚无独立gate输入；这不是声称硅片不能关闭48M。

固定来源见 [sources.json](sources.json)：[PCR header](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/soc/esp32c5/register/soc/pcr_reg.h#L2041-L2116)、[clk_gate_ll.h](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/hal/esp32c5/include/hal/clk_gate_ll.h#L18-L124)、[esp_clk_tree_initialize](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/esp_hw_support/port/esp32c5/esp_clk_tree.c#L80-L119)、[CPU source/commit LL](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/hal/esp32c5/include/hal/clk_tree_ll.h#L298-L352)。IDF5.5.3=`2c211b236707889e8400c4dc5644dd5c4ee071e0`，5.5.4=`735507283d5b2f9fb363a1901172dbd9e847945d`；所保留相关源码对逐字节相同。原猜测的`clk_tree.c`路径不存在，真实文件名为`esp_clk_tree.c`。

| bit | fixed SDK tap | 功能边界 |
| --- | --- | --- |
| 0 | 240MHz=480/2 | CPU source3；实际active source3时拒关。 |
| 1 | 160MHz=480/3 | CPU source2及SPI2 selector1；active source2拒关。 |
| 2 | 120MHz=480/4 | SPI2 selector3。 |
| 3 | 80MHz=480/6 | LEDC selector2；不是当前CPU或MSPI的80MHz输出。 |
| 4 | 60MHz=480/8 | 固定SDK/原指令依据；TRM9.68图标reserved，非硅片验证。 |
| 5 | 48MHz=480/10 | TRM也列此字段；此模型保留为1，关闭明确unsupported。 |
| 6/7/8 | 40/20/12MHz=480/(12/24/40) | 固定SDK命名/LL/原指令依据；TRM9.68列reserved，非硅片验证。 |

[C5 TRM1.1](https://www.espressif.com/sites/default/files/documentation/esp32-c5_technical_reference_manual_en.pdf) Register9.68只命名240/160/120/80/48M，bit4与6以上覆盖与固定SDK冲突。本模型选取**与原厂版本一致的SDK及实际指令**作为限定虚拟寄存器合同，保留该差异。没有测得或验证reserved字段的硅片行为；不能把SDK POR=`0x1ff`写成实物readback。48M字段本身没有这个缺失问题；其关闭限制来自当前模型USB支持范围。

原厂typed `0x4211f8a0` 对应 `esp_clk_tree_initialize()`。当前PCR source3分支在`0x4211f8ee`读取+128，然后清160M，再清120/80/60/40/20/12，保留当前CPU240M与USB48M。按本合同的header POR，预期RMW是：`1ff→1fd→1f9→1f1→1e1→1a1→121→21`。真实原指令省略48M clear，对应源码USJ配置条件；没有替guest写寄存器、按PC放行或限定只收这七个常量。支持按named bits独立开关与复用，只有已声明的安全边界拒绝。

当前CPU80MHz来自PLL240/3，AHB/APB40MHz；MSPI/PSRAM80MHz来自**直接SPLL480/6**和MSPI自身divider/gate。因此清PLL80 gate不会改变MSPI输入。MSPI继续接实际analog readiness，未改其API或代码。TIMG校准的XTAL参考、LP slow/fast、TG0 XTAL group control同样不以这些derived gates替代。

`BootClock::pll_div_gate_mask()->Option<u16>` 在profile关闭时为None；`pll_tap_hz(mhz:u16)->Option<u32>`只接受表中九个named tap，unknown/480均None。直接SPLL480是另一个源。report保留raw gate与实际ready条件后的available bitmask，避免把mask等同ready。gate写与analog observation仍先结算旧输入；CPU-only/LP reset保留PCR gates，HP/Chip reset恢复header POR，raw deselect撤profile及availability。所有时间沿nominal240MHz调度轴，不声称CPU DFS实际cycle duration。

Transport setter改为 `set_clock_sources(cycle,rc_fast_hz,pll_ready,pll_div_gate_mask)`。默认mask1ff保留旧组件环境；SPI2分别检查bits1/2、LEDC检查bit3。setter先积分旧LEDC source，再更新mask；selected SPI失源会取消而不造DONE，unrelated tap变化保留传输；LEDC失源暂停，恢复后保持fractional phase并重新计划。既有XTAL-only默认report保持相同，非默认mask才补gate诊断。既有8个Transport组件用例的observer补第四参数；新增3个独立tap/取消/phase负例，**本轮未构建或执行Transport测试**，由父级接Bus后跑完整C5套件。

父级接线：Bus各Transport setter新增 `boot_clock.pll_div_gate_mask().unwrap_or(0x1ff)`；MSPI仍使用独立analog `pll_ready`。父级负责pending watchdog reset优先、old-input-first刷新和完整integration测试。此提交不改Bus/Soc/lib/core，无ROM、BIN或卡修改。新source接口需要接线后才能完整编译运行；本轮只跑下面的clock harness。

```sh
python3 docs/evidence/c5-pll-div-gates-2026-11-25/run-owned.py --repo "$REPO" --output-dir "$TASK_TMP"
clippy-driver --edition=2021 --test "$TASK_TMP/c5-pll-div-clock-harness.rs" -o "$TASK_TMP/clock-clippy-tests" -D warnings
node tools/check-evidence-privacy.mjs
```

[checks.json](checks.json)、[typed-sequence.json](typed-sequence.json) 和 [owned-tests.txt](../c5-evidence-retention-final-2026-11-25/stdout-manifest.json)保留静态合同、source/input/output hash及19个clock测试结果。初始独立Clippy发现生产getter在孤立harness中未消费，随后以PLL gates不影响TIMG0 group control的独立断言覆盖，未加lint豁免。父级真实起点为24764132 instructions、24764096 nominal cycles、2194 UART bytes，8MiB memory test OK后在首次+128 read fault；预期RMW尚未执行。保留原始日志hash和必要数值，未提交私有路径、完整事务流或无关主机信息。没有Cargo/native/WASM/browser/benchmark执行，也不据此声称app_main/LCD已通过。

完整逐 test stdout 已移至 ignored 输出；原路径、bytes/hash、固定 artifact/source receipt 与重现命令见[留存清单](../c5-evidence-retention-final-2026-11-25/stdout-manifest.json)。测试结果、负结果和历史 raw/normalized hash 保持不变；此留存整理没有运行组件或原厂机。
