# C5 COEX gate 请求

原厂已实际进入 `app_main=4200aebe`、完成PCF/QMI初始化并进入wifi_manager，随后在应用IRAM `40800db0` store `600af018=3` 遇到模型边界。父级原样运行记录为44,200,045 instructions、44,199,998 nominal cycles、4657 UART bytes；原日志14746B，SHA256 `8f98e07c231537eab58961783c0cef03ad29a1d6727b4bce384eda0eb273608d`。本 lane 没有运行整机，也不宣称已越过此边界、完成PHY或产生LCD像素。

**`600af018` 是 MODEM_LPCON_CLK_CONF，不是I2C source选择器。** 固定 [C5 header](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/soc/esp32c5/include/modem/modem_lpcon_reg.h#L138) 定义bit0 WIFIPWR_EN、bit1 COEX_EN、bit2 I2C_MST_EN，各为R/W、POR0；bit3是尚未实现的LP_TIMER_EN。本扩展只新增具名COEX bit1，与已实现bit0/2组合，读写范围为0..7。LP_TIMER和其他reserved/unknown位仍拒绝。

原typed `40800da2..db2` 从ctx取HAL与LPCON指针，读+18，`a1&1` 左移1，清旧bit1后OR，再于 `40800db0:cf1c` store；不是ROM400xxxxx执行或host函数拦截。固定 [modem_clock.c](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/esp_hw_support/modem_clock.c#L136) 的coex configure直接调用 [C5 LL](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/hal/esp32c5/include/hal/modem_lpcon_ll.h#L161) 设置 `clk_conf.clk_coex_en`，与指令匹配。它只替换bit1；用原strict owner已接受的word1作前值，RMW结果为3。先前WiFi map回调与后续PHY入口的静态存在不等于PHY已执行成功。

## 消费者与边界

- 新 `coex_clock_requested()->bool` 只读bit1请求，report仅在该位为1时增加 `coex_lp_gate_requested=1 COEX_execution=unsupported`。没有返回运行Hz、ready、ACK、IRQ或计数进度。bit1为0时原报告格式保持。
- 3表示COEX/WIFIPWR请求，**I2C_MST_EN2为0**。REGI2C仍由bit2/FORCE2、已应用MODEM_APB/LP_APB/I2C权限、目标电源、route和master selector共同资格；COEX bit不能授予master访问。FORCE仍只实现I2C bit2，COEX/LP_TIMER/FE_MEM force请求继续拒绝。
- 旧4是I2C gate请求；真正的160M选择是 [SYSCON+04 bit12](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/soc/esp32c5/include/modem/modem_syscon_reg.h#L137)，完全独立且未修改。这里没有新增PLL160M或REGI2C频率建模，也不据COEX gate制造SPLL lock。
- `ModemLpClock` 的COEX selector/divider仍只是pre-functional-gate配置观察，未被改动。有效COEX/RF/MAC engine没有实现，不能把其pre-gate ratio与此bit拼成“已运行时钟”。后续实际请求若需要这些消费者继续Unsupported，不能因本配置写成功跳过它们。
- BOD0阈值slave61/reg5、OCODE reg0/3、SAR、DIG_REG与BBPLL旧target保持原边界；没有新byte影子、校准code、eFuse镜像或常量DONE。未知PHY/MAC/RF、slave61 reg4等访问不因此获准。

`write_at` 仍先以旧输入pump至给定cycle，再提交新control。纯COEX toggle不改变I2C clocked或已结算target/deadline；若同一完整word同时清I2C_EN，且FORCE未覆盖，按原引擎撤掉尚未完成的PLL配置时钟资格。测试分别在deadline前和deadline同刻提交3：前者不制造完成，后者先由旧有效输入真正到期，随后仍须guest真实STOP序列才能ready。已经真实锁定并STOP的SPLL不依赖I2C配置总线继续开钟，这项旧行为也保留。

reset_domains沿现有functional声明：CPU-only/LP-only保留HP域CLK_CONF，HP/Core及Chip清COEX请求；并非独立硅片flop或睡眠retention验证。此bit没有新增事件或deadline，父级继续负责pending-reset屏障、profile边界和拒绝分类。本提交不改Bus/lib/Soc/PMU/board或共享模块。

## 来源与检查

来源固定IDF553 `2c211b236707889e8400c4dc5644dd5c4ee071e0` 与554 `735507283d5b2f9fb363a1901172dbd9e847945d`。完整URL/bytes/hash见 [sources.json](sources.json)，复用既有public缓存，没有伪造新fetch时间。LP register/LL、SYSCON register、REGI2C LL两版相同；modem_clock.c整文件有refcount策略差异，不能称完整驱动版本一致。相关coex configure局部：553行136–140、554行137–141，162B，SHA256 `69dad74b8191711b4fe5e8d12e5f20ab5d9c4eaed5a42b630ec7f70731e12436`。

```sh
python3 docs/evidence/c5-coex-gate-2026-11-25/run-owned.py --repo "$REPO" --output-dir "$TASK_TMP"
python3 docs/evidence/c5-coex-gate-2026-11-25/run-owned.py --repo "$REPO" --output-dir "$TASK_TMP" --compiler clippy-driver --lint
node tools/check-evidence-privacy.mjs docs/evidence/c5-coex-gate-2026-11-25
```

actual BootAnalog standalone49项（43旧+6新）及owned Clippy `-D warnings`通过；没有Cargo、native、WASM、浏览器或benchmark。覆盖8种具名组合、真实RMW、无master授权、160M独立、旧输入先结算、无新ready/deadline、retention和严格未知模式。初轮48过/1失败是旧fixture仍把word3当非法；现改为接受3并验证不授master、保留真实既有锁定PLL，然后拒绝LP_TIMER bit3值8。另一旧非法6也换为未实现8，避免继续把已确认COEX+I2C组合当坏值。原失败hash/原因及最终输出保留于 [checks.json](checks.json)；未提交含thread标识的原失败stdout。

后续由父级整机运行验证实际下一边界。本组件的配置成功不代表COEX、PHY、WiFi关联或LCD首屏成功；未push，旧map/BOD分支保留。


完整owned测试/lint stdout已按[证据保留规范](../README.md)移出当前源码树；原bytes/SHA、来源提交及逐字校验的ignored target位置见[保留清单](../c5-owned-stdout-retention-2026-11-25/manifest.json)。checks中的既有artifact hash仍标识原始输出，未改为其他文件hash；结果、命令及原负面记录保留。重新运行harness时将`--output-dir`设为调用者的ignored输出目录，全文不再复制回Git。
