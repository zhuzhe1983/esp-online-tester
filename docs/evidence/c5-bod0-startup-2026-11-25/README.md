# C5 BOD0：固定理想供电下的配置子集

未改动的 LCD273 factory 已实际到 `42070f9c` 读 `600b2c00`：26,913,956 instructions、26,913,920 nominal cycles、3372 UART bytes，尚未到 app_main。原日志 hash 和输入 hash 见 [checks.json](checks.json)。本提交只提供 BOD0/真实 REGI2C 阈值组件；没有运行整机，也不宣称原厂已越过该边界。

`BootBrownout` 的 enable、count-clear 和中断字遵循固定 C5 源；事件输入只支持显式**恒定理想 USB3V3**：VDDA3、VDDPST1/2/3 四轨均取虚拟3300mV，实际 guest 写入 code7 后用 nominal2510mV 比较。固定 [C5 Kconfig](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/esp_hw_support/power_supply/port/esp32c5/Kconfig.power) 将 code7 标为约2.51V，并说明是估计且有芯片差异。因此3300/2510是明确的理想输入/nominal传递函数，**不是实际引脚电压、校准结果或物理阈值保证**。公开材料没有给误差边界，单凭插入USB不能证明真实板上四轨健康。

在这个输入合同中比较结果恒为正常，不存在 brownout pulse，故内部计数保持0、RAW没有硬件自置事件。寄存器没有完成/ready标志，也不创建倒计时。RAW是独立 latch，ST按RAW与ENA推导，WT clear真正清latch；单元测试用私有latch fixture检验 mask/clear/count 相互独立，不提供产品事件注入入口。动态电压、其他threshold及触发后的 IRQ/flash/RF/reset 动作都没有实现。

## 固定数字协议

来源为 IDF5.5.3 `2c211b236707889e8400c4dc5644dd5c4ee071e0` 和5.5.4 `735507283d5b2f9fb363a1901172dbd9e847945d`；相关15对完整文件逐字一致，URL/bytes/SHA256见 [sources.json](sources.json)。包括 [brownout HAL](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/hal/brownout_hal.c)、[C5 LL](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/hal/esp32c5/include/hal/brownout_ll.h)、[LP_ANA header](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/soc/esp32c5/register/soc/lp_analog_peri_reg.h)、[threshold byte](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/soc/esp32c5/include/soc/regi2c_brownout.h) 与 [HP master driver](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/esp_rom/patches/esp_rom_hp_regi2c_esp32c5.c)。

| 对象 | 本组件合同 |
| --- | --- |
| `600b2c00` | header POR `0ffc0100`；named mask `ffffffc0`；bit6 flash suspend请求、7 RF PD请求、17:8 interrupt wait、27:18 reset wait、28 count clear、29 mode0 enable、30 reset select、31 reset enable。原厂cfg字节 `07 01 00 01 01`，实际源顺序 RMW 最终 `6ffc02c0`。 |
| count clear bit28 | R/W保持清数，guest真实写1再写0；不自清，不借IRQ clear替代。 |
| waits/actions | 当前仅POR wait1或原厂wait2、reset wait3ff。RESET_SEL0/1在硬件reset关闭时可配置；RESET_ENA1事务拒绝。flash/RF请求位只在保证无事件的输入范围内保留，不代表动作引擎已实现。 |
| `600b2c10/14/18/1c` | HP RAW/ST/ENA/WT CLR均bit31；RAW事件自置、ST=RAW&ENA、ENA读写、CLR写1清RAW/read0。header称mode0 enable0时RAW无效，所以RAW/ST观察返回None，而不是假造健康；CTRL/ENA/CLR仍可访问。RAW/ST写、reserved bits拒绝。 |
| IRQ | 固定C5 `power_supply_periph.c` 选择 `ETS_LP_RTC_TIMER_INTR_SOURCE`，enum实际为15；原DROM也为0f。原厂flags500为SHARED/IRAM，SDK默认level1，CPUline由guest allocator决定。helper的`irq()`只提供派生HP电平，父级负责source15/matrix/CLIC。 |
| `slave61 reg5[2:0]` | HP master选择ULP_ROUTE `00fffeff`，CONF2 bit10=1选CTRL0、0选CTRL1；实际MODEM/APB/I2C gates沿用现owner。reg5 virtual初始byte0不是硅片POR；成功write7才给出Some7资格。只接受完整byte7，故未知上5位或其他code写事务拒绝。真实masked writer是一次target读、一次写，控制器轮询不新增target读。 |
| 其他模拟状态 | BOD1/FIB/glitch控制、VDET slave69:10、OCODE slave61:0/3、SAR、PLL保持原owner；reg5不需要OCODE quiet、DONE或CPU-XTAL。它们不能充当BOD0电压/事件。没有扩成多byte影子RAM。 |

## 接口与复位边界

父级独占Bus/lib/Soc/loader接线，本提交不改这些文件。

```text
BootBrownout::new() -> Self                    // profile OFF, header POR state
BootBrownout::contains(address:u32) -> bool    // only +00/+10/+14/+18/+1c
enable_ideal_usb3v3() / disable_ideal_profile()
observe_threshold(Option<u8>) -> bool          // actual BootAnalog getter only
read(address:u32) -> Option<u32>
write(address:u32,value:u32) -> bool
irq() -> bool
reset_domains(hp:bool,lp:bool,analog:bool)
report() -> String
C5BootAnalog::enable_virtual_bod_threshold() / disable_virtual_bod_threshold()
C5BootAnalog::bod_threshold() -> Option<u8>
```

只有显式profile和实际已写code7一起满足时，bit29使能才可成功。其他输入/threshold观察拒绝；正在检测时撤阈值也拒绝而保持状态。父级必须先处理pending reset，在actual slave61 transfer后同步getter，再接受检测使能；退出raw/SRAM时先disable/drop BOD owner，再撤target profile。默认没有新增target可达性或诊断suffix；原32项BootAnalog纯测试保持通过。

按 [TRM1.1](https://documentation.espressif.com/esp32-c5_technical_reference_manual_en.pdf) §9.1的functional域合同：CPU/Core保留LP数字配置；LP/System恢复CTRL POR、count/RAW/ENA；Chip另撤实际analog threshold资格，须新guest写7，且清累计诊断。模拟byte保留至Chip、具体内部flop保留域均是明确functional policy，未宣称硅片实测；profile选择作为板级输入独立保留，raw退出另撤。

TRM §21把检测器放always-on域，mode0计数单位是实际LP_FAST_CLK；这里无brownout pulse，无需时钟积分。若要支持动态事件，必须接实际LP_FAST source/计数/采样和action engines，不能硬填24MHz或直接将RAW锁0。LP-side +20..2c 和DATE clock不是本模块owner。LP_CLKRST ANA_PERI_RESET_EN的独立写也未新增支持。

原厂`USE_INTR=y`使硬件reset关闭；ISR会clear、可选callback/flash reset、set软件hint9，再调用ROM软件重启。若将来支持硬件 BOD cause0f，TRM要求按RESET_SEL传Chip/System域和发生时点；不能套现有HP fallback，也不能把软件hint9当cause0f。目前RESET_ENA1、未知/变化电源、未实施threshold、RAW软件置位、LP IRQ enable、物理glitch/校准一律Unsupported。

## Owned 验证

```sh
python3 docs/evidence/c5-bod0-startup-2026-11-25/run-owned.py --repo "$REPO" --output-dir "$TASK_TMP"
python3 docs/evidence/c5-bod0-startup-2026-11-25/run-owned.py --repo "$REPO" --output-dir "$TASK_TMP" --compiler clippy-driver --lint
node tools/check-evidence-privacy.mjs
```

Rust1.100.0 standalone46项通过：32既有BootAnalog、5阈值、8BOD0、1跨模块源序列。owned Clippy `-D warnings`通过，同46项通过。结果与源码/工具hash见 [checks.json](checks.json)、[stdout保留清单](../c5-owned-stdout-retention-2026-11-25/manifest.json)。覆盖真实1R/1W资格、master/route/clock、其他target不变、最终配置、RAW/ENA/CLR/count独立、事务拒绝、profile退出和reset retention。未运行Cargo/shared build、原厂后续执行、WASM、浏览器或CPU benchmark；后续由集成结果单独证明。


完整owned测试/lint stdout已按[证据保留规范](../README.md)移出当前源码树；原bytes/SHA、来源提交及逐字校验的ignored target位置见[保留清单](../c5-owned-stdout-retention-2026-11-25/manifest.json)。checks中的既有artifact hash仍标识原始输出，未改为其他文件hash；结果、命令及原负面记录保留。重新运行harness时将`--output-dir`设为调用者的ignored输出目录，全文不再复制回Git。
