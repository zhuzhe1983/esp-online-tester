# C5 启动外设关闭配置

`BootPeripheralClock` 增加 32 个精确地址的关闭配置：30 个 PCR 字、LP_UART CLK_CONF、LP_GPIO CLOCK_GATE。它只承接尚无控制字 owner 的范围；UART、I2C、LEDC、SPI2、GDMA、SARADC、SHA、TIMG APB/XTAL/WDT、cache、APM、LP clock-to-pad 和 LPPERI 仍由各自模块处理。

该提交基于 `7c4f084f4504b8d905084c5ff9a20fabcfc6d44b`。已有原厂执行停在 `PC=0x42000e20` read `0x6009603c`，26,601,352 instructions / 26,601,344 nominal cycles / 2,194 UART bytes。此提交没有重新运行原厂固件，也没有接入 Bus/Soc/lib；不能据纯测试声称已越过边界、进入 app_main 或有 LCD/SD/RF 输出。

固定 [IDF 5.5.3 esp_perip_clk_init](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/esp_system/port/soc/esp32c5/clk.c#L270) 与原镜像 typed 指令给出真实关钟顺序。模块测试保留属于本模块的完整 34 次 store；已存在 owner 的写入没有复制或覆盖。完整地址和值也列在源文件中的独立 typed fixture。不是按 PC 放行。

| 配置组 | PCR offsets（基址 `60096000`）或绝对地址 | 受支持的操作 |
| --- | --- | --- |
| RMT | `3c/40` | CLK_EN0、SCLK_EN22 关闭；source/divider/reset 改变拒绝 |
| 普通 TIMG timer | `58/64` | TIMER_CLK_EN22 关闭；与 WDT `5c/68` 独立 |
| TWAI0/1 | `28/2c/30/34` | APB0、functional22 关闭 |
| I2S | `74/78/80` | APB0、TX/RX CLKM22 关闭 |
| PCNT/ETM/MCPWM | `9c/a0/a4/a8` | APB0、PWM CLKM22 关闭 |
| PARLIO | `ac/b0/b4` | APB0、RX/TX18 关闭 |
| TSENS | `90` | clock22 关闭，reset23 不支持 |
| PAU/REGDMA | `f8` | 先 CLK_EN0 清除，再 RST_EN1 置位并保持；不允许 release/enable/retention engine |
| RSA/AES/ECC/UHCI/TRACE | `d4/cc/dc/38/fc` | APB0 关闭；没有 data/IRQ/crypto/trace engine |
| TCM/PSRAM monitor | `16c/170` | monitor clock0 关闭；不是 PSRAM data/cache 时钟 |
| PVT | `b8/bc` | APB0、functional22 关闭；保留 SITE bits2..4 初值 `1c` |
| CLK_OUT | `12c` | 只接受整字0；不是内部 PLL_DIV `128` 的 gate |
| LP UART | `600b1488` | TX/RX bits24/25 可分别或同时单向关闭；reset26/27、使能、数据访问拒绝 |
| LP GPIO | `600b47f8` | 只接受配置0；正常读回实际 latch，未实现 GPIO engine |

一般控制只允许命名 gate 从1清0或写当前相同值；不允许重新开启、修改 mux/divider/reserved/RO 字段。CLK_OUT 和 LP_GPIO 仅接受0，PAU 仅额外允许 assert reset。失败写完全不修改状态或诊断计数；计数饱和，不受 guest 值溢出影响。READY 若有明确 SDK POR，只保留初始只读位；不接受 guest reset，因此没有伪造 reset-completion/ready 事件。未覆盖字和外设数据/状态/IRQ 地址继续 unsupported。

`rmt_clocked()` 仅由 RMT APB enable、functional enable、reset 三个实际位推导，所有合法配置均关闭。父级必须在 validated-ROM profile 下真正屏蔽 RMT MMIO、tick、IRQ 和 deadline；该 getter 不是可用频率或硅片 ready。未选择 profile 的 legacy SRAM 行为由父级保留。LP_GPIO 对应 [rtc_io_ll 的真实双配置读回](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/hal/esp32c5/include/hal/rtc_io_ll.h#L60)：另一个 LPPERI bit27 仍由其 owner 处理，循环不是 BUSY 或 calibration completion。

reset 合同是显式 virtual functional retention：CPU 保留；HP 恢复这30个 PCR 字；LP 恢复 LP_UART/LP_GPIO；analog/Chip 恢复全部及诊断计数。它不是 per-flop 硅片保证。raw/SRAM deselect 时由父级撤掉 profile/模块；此模块没有时间源、timer、IRQ、LP CPU、模拟电压或 RF 行为。

两个固定 SDK 的相关文件逐字相同，见 [sources.json](sources.json)。但它们与 [TRM 1.1](https://www.espressif.com/sites/default/files/documentation/esp32-c5_technical_reference_manual_en.pdf) 的多处 POR 不同：

- RMT_CONF：SDK named POR0、无 READY2；TRM Register9.14/PDF p408 为5并定义 READY2。RMT_SCLK 的 SEL1/DIV1/EN0 则一致。
- TWAI/PCNT/ETM/PWM/PARL/crypto APB：SDK4，TRM5；I2S SDK `c/2000/2000`，TRM `d/00402000/00402000`。多个 functional gates 的 SDK 默认0、TRM 默认1。
- TRACE SDK0、TRM1；TCM/PSRAM monitor 的5一致。TRM 缓存未找到 PAU/REGDMA、PVT、CTRL_CLK_OUT 同名详细条目，按固定 SDK 与原指令限定。

本模型明确选择与原厂匹配的固定 SDK named-field virtual POR，不混入 TRM 的 READY/default，也不声称验证了硅片上电状态。PAU held-reset 的来源是 [pau_ll.h disable 分支](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/hal/esp32c5/include/hal/pau_ll.h#L24)，不是虚构完成状态。

复现纯测试和 owned lint：

```sh
python3 docs/evidence/c5-peripheral-clock-disable-2026-11-25/run-owned.py \
  --repo . --output-dir "$C5_CLOCK_CHECK_DIR"
```

输出目录由调用者提供。脚本只用 stable rustc 编译本模块并运行其12项测试，再用 standalone clippy-driver、warnings denied 检查同一模块；不调用 Cargo。测试覆盖完整34-store fixture、32-word 精确解码/既有 owner 无 alias、所有关闭后的单 bit 非法变更、失败写原子性、逐位 LP UART、PAU held-reset、CLK_OUT 隔离、RO POR/无伪 ready、RMT gate 推导和 CPU/HP/LP/Chip reset。

[checks.json](checks.json) 及 [owned-tests.txt](../c5-evidence-retention-final-2026-11-25/stdout-manifest.json) 保存结果、输入/hash、编译器版本和边界事实。没有工作区构建、native guest、WASM、browser 或 benchmark。只保留相对文件名、公开来源、hash 和数字；测试输出中的调用者路径替换为 `<repo>` / `<owned-output>`，未保留本机命令行或原始私有 native 日志。

完整逐 test stdout 已移至 ignored 输出；原路径、bytes/hash、固定 artifact/source receipt 与重现命令见[留存清单](../c5-evidence-retention-final-2026-11-25/stdout-manifest.json)。测试结果、负结果和历史 raw/normalized hash 保持不变；此留存整理没有运行组件或原厂机。
