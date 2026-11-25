# C5 LP_FAST mux：RC_FAST 与 HP bridge 分离

真实请求是 `LP_CLKRST_LP_CLK_CONF=0`，将FAST_SEL[3:2]从1(XTAL/2)切到0(RC_FAST)，不是48MHz XTAL。固定IDF5.5.3/5.5.4 [lp_clkrst_reg.h](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/soc/esp32c5/register/soc/lp_clkrst_reg.h#L14-L52)、[LL](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/hal/esp32c5/include/hal/clk_tree_ll.h#L488-L539)、[app esp_clk_init](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/esp_system/port/soc/esp32c5/clk.c#L85-L114)与原厂typed一致：caller42000c44置a0=0，42000c4a调用4080922e；40809244读原值，9246清mask0xc，9248写0，随后真实delay_us(3)。当前写尚未接受，不能当作已切换。

`ClockTree`现只接受完整word0/4/8，即FAST0/1/2，SLOW_SEL固定0，reserved LP_PERI_DIV0；其他slow源、FAST3、divider和保留位拒绝。`BootClock.rtc_fast_hz()`委托实际mux：RC_FAST仅在实际analog FOSC power30开启且DFREQ100时为明确虚拟17.5MHz，否则None；XTAL/2为24MHz，XTAL为48MHz。RC_FAST振荡器与HP gate分开：`ICG_HP_FOSC`31只控制到HP系统的桥，关闭它会撤`hp_rc_fast_hz()`，不会关闭已选中的LP RC_FAST。没有把PLLready、HP gate或旧POR24MHz当成LP source可用的替代输入。

源选择在该功能模型的寄存器边界生效，不添加ready/settling状态；driver原有3us delay仍由guest执行，不声称已模拟物理切换延迟或glitch。

`clk_tree_defs.h`给DFREQ100的RC_FAST approximate17.5MHz；LP_CLKRST header POR却为172。本模型继续只支持既有明确virtual tuned100档，不给172猜频率，不声称物理校准。PLL与此源无依赖。[TRM1.1](https://www.espressif.com/sites/default/files/documentation/esp32-c5_technical_reference_manual_en.pdf)9.2.4.2及13章区分LP_FAST与LP_DYN_FAST：当前awake状态同频；LP_SLEEP会使DYN_FAST改用LP_SLOW，但睡眠转换不在当前支持范围。0/1/2 mux在两个固定SDK版本及TRM之间一致；源hash/版本比较见 [sources.json](sources.json)。

| 消费者 | 本次FAST切换的频率/计数合同 |
| --- | --- |
| SWD analog age | 明确虚拟request850ms/expire950ms，204M/228M nominal cycles不变；不是LP_FAST周期计数。当前requests0、pulse0，不能把切换当feed或重启age。 |
| SWD feed pulse | width300计的是LP_DYN_FAST ticks。24M→3000 nominal cycles；17.5M→ceil(300*240M/17.5M)=4115；48M→1500。另一lane实现source-clocked pulse；此提交未改LP_WDT。 |
| LP_TIMER | SLOW_SEL0仍RC_SLOW virtual136kHz，保留counter13795、phase及capture历史。FAST切换不能清零或改rate。 |
| RWDT | 仍136kHz LP_DYN_SLOW，保留stage0 count13710、phase/feed历史。其物理reset-pulse宽度仍unmodeled，不能据此声称LP_FAST实体reset波形已实现。 |
| TIMG0 MWDT | group XTAL48MHz，factory已关闭；count114777为历史，FAST mux不影响。 |
| TIMG RTC calibration | XTAL参考48MHz、slow136k或独立HP可达FOSC/32；改LP mux不改变source选择、累积edge或造CAL_READY。 |
| CPU/AHB/APB、PLL taps、MSPI | 80/40/40MHz，derived gates21；MSPI direct SPLL480/6，均独立于LP_FAST。纯fixture验证root/taps保持；未实例化MSPI引擎，不冒领其integration测试。 |
| SYSTIMER、UART、HP I²C、SPI2/LEDC | 现有XTAL/HP源合同不变；SPI2/LEDC的HP RC_FAST仍受HP bridge约束，不能被LP mux反向改变。SYSTIMER现为独立16MHz模型。 |
| SAR timer/OCODE | 各自既有XTAL timer或explicit ideal target clock；本次不改变其历史/flags。LP_PERI RNG仍deterministic sample-per-read，不代表真实LP bus/analog时钟或熵。 |
| LP CPU/LP peripherals/sleep物理时序 | 当前未实现部分不因一个mux latch获得支持；LP bus divider、动态睡眠源/电源切换继续unsupported。 |

父级接线必须在Clock/Analog改变前先pump watchdog至旧rate的当前时点并处理pending reset，再发布`rtc_fast_hz()`给新增`LpWdt::set_fast_clock(cycle,Option<hz>)`。Clock.write_at只积分自己的calibration，不能替代LP_WDT积分。运行中pulse须保存已完成ticks及fraction，再以新rate算剩余deadline；None使feed暂停，但analog expiry继续。例：300-tick pulse在24MHz先过300 nominal cycles，已30ticks；切17.5MHz后剩270ticks需3703cycles，总完成时点4003，不能继续旧3000或重启完整4115。fraction继承与nominal-cycle向上取整是功能策略，不是硅片glitch/phase测量。reset同刻优先、不在pending之后锁存新rate，仍由父级处理。

CPU-only/HP reset保留LP mux/tuning；LP或analog reset恢复mux4/DFREQ172，因此LP_FAST回24MHz而不制造已调RC source；analog撤真实输入。raw deselect使所有clock getter不可用，再启用从POR4开始。原POR24MHz与feature-off报告保持原合同。仅修改boot_clock.rs/clock_tree.rs；未改Bus、Soc、LP_WDT、Transport或固件。

本轮运行原有 [clock纯harness](../c5-pll-div-gates-2026-11-25/run-owned.py)：23项通过（原19，新增4），owned clippy-driver -D warnings通过；测试覆盖HP/LP分离、缺power/tuning返回None、三个mux值、独立root/slow/taps、reset/raw及未知位拒绝。没有Cargo/native/WASM/browser/benchmark。 [checks.json](checks.json)保留命令、输入/输出/source hash和父级native边界；完整日志只保留忽略缓存。尚未完成SWD/Bus集成，不声称原厂已越过此fault或完成app_main/LCD。

完整逐 test stdout 已移至 ignored 输出；原路径、bytes/hash、固定 artifact/source receipt 与重现命令见[留存清单](../c5-evidence-retention-final-2026-11-25/stdout-manifest.json)。测试结果、负结果和历史 raw/normalized hash 保持不变；此留存整理没有运行组件或原厂机。
