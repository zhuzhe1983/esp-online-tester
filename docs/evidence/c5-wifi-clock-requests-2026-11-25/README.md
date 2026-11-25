# C5 CLK_CONF1：WiFi 四组请求

父级原样运行已越过COEX请求，实际首读 `PC40800d2a → 600a9c14`，cause5；44,199,355 instructions、44,199,294 nominal cycles、4536 UART bytes，日志SHA256 `ba136ac655cdb8ed3ff7644c9c4990ff97cfb64d69e7109db5ca5f1f746a0cfe`。这确认配置字已到达；本 lane 未运行原厂，不声称其后四次store、PHY、WiFi或LCD已成功。

独立 `ModemWifiClock` 只拥有 `600a9c14`，header POR0。它保留明确请求，**不产生有效Hz、ready、IRQ、RF/PHY/MAC执行或任何APB grant**。`BootAnalog`仅新增纯 `modem_apb_accessible()`，读取已应用的 `permissions Some &1`；None/bootstrap为false，shadow map及code capture不直接grant，旧I2C/APB/160M/analog路径不变。

## 字段与原厂消费者

固定 [C5 register header](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/soc/esp32c5/include/modem/modem_syscon_reg.h#L420) 的CLK_CONF1是 `MODEM_SYSCON_BASE+14`。bits0..21均具名R/W、默认0；bits22..31 reserved。本范围只支持下列四组，其余具名FE/BT gate也明确Unsupported，不能统称reserved或顺手存储。

| 支持组 | mask / 字段 | 合同 |
| --- | --- | --- |
| WIFI_APB | `400` / bit10 | 独立请求0/1；不是配置寄存器所在MODEM_APB的访问资格 |
| BB44M | `004` / bit2 | 独立请求0/1，名称不保证当前存在44MHz源 |
| MAC | `200` / bit9 | 独立请求0/1，不运行MAC或生成IRQ |
| BB_MISC | `1fb` / bits8,7,6,5,4,3,1,0 | 只支持全组0或1fb；未观察的单bit/partial组合拒绝 |

BB_MISC逐位名是160X1、80X1、40X1、80X、40X、80M、40M、22M。aggregate为7ff，共16种四组组合；bits11..15为FE20M/40M/80M/160M/APB，16..18为BT_APB/BB/MAC，19..21为FE_PWDET_ADC/ADC/DAC，均未实现。`+08 FORCE`、`+10 reset`、`+18 WIFI_BB_CFG`及其他PHY/BB/MAC字不被本owner接管。

固定 [C5 LL](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/hal/esp32c5/include/hal/modem_syscon_ll.h#L319) 用RMW OR/AND~mask；在 `SOC_PHY_CALIBRATION_CLOCK_IS_INDEPENDENT=1` 分支，BB configure使用1fb，44M由独立setter处理。两版C5 soc_caps都明确该宏为1。

原镜像 `40800ede:0f400513` 实际返回 WIFI47 deps=`f4`。固定 [modem_clock.c](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/esp_hw_support/modem_clock.c#L310) 的设备enum/dep匹配：COEX index2、WiFi_APB4、BB44M5、MAC6、BB7。未包含I2C_MASTER index3。原 `4081a614` 的8个device records/64B SHA256为 `6c020f095fe915c41d39d80e067778c8c012742ac9edecad9712955fe2516f9f`，对应callback为 `40800da2 / 40800d18 / 40800d36 / 40800c56 / 40800c74`。初始refs0是原BIN数据，不是运行时refcount观察。

原enable loop按低bit依次调用设备。COEX之后，静态 `40800d2a` 读本字，`d32`写WIFI_APB10，`d4e`写BB44M2，`c70`写MAC9，`c8a` OR BB_MISC1fb；若进入word0，则推导值 `400→404→604→7ff`。只有首读已由本轮父级native确认，后四word仍为静态/组件测试结果。公开源码的WiFi初始化状态和refcount分支由guest执行，owner不替它们作host决定。

## 接口与真实边界

```text
new() -> Self                         // header request word0
contains(address:u32) -> bool          // exact600a9c14 only
read(address:u32,modem_apb_accessible:bool) -> Option<u32>
write(address:u32,value:u32,modem_apb_accessible:bool) -> bool
requested_bits() -> u16                // diagnostic requests only
reset_domains(hp:bool,analog:bool)
report() -> String
```

caller提供已有MODEM_APB applied permission；false时read=None、write=false且word/诊断全部不变。即使WIFI_APB请求bit10已置1，也不能反向授予caller资格。partial BB、FE/BT、reserved、别名或misaligned地址拒绝且不部分改状态。写次数是固定大小、饱和的诊断，非硬件计数器。

没有新clock源/时间输入、deadline或per-tick工作。门控配置请求与来源/有效时钟、PMU state、实际功能引擎分开；原map6仅ACTIVE|MODEM许可，不是PHY就绪。已声明的functional HP域：CPU/LP retain，HP/Chip清word与本组件诊断；不是硅片每flop或sleep retention保证。父级独占Bus/lib/Soc/profile/reset/partial-watchdog/ModelLimit接线，必须保持pending reset屏障、严格ROM选择及实际provider，不加host bootstrap grant。

## 固定来源与 owned 检查

553=`2c211b236707889e8400c4dc5644dd5c4ee071e0`，554=`735507283d5b2f9fb363a1901172dbd9e847945d`。字段header27337B SHA `4afef574adf1402402e0c91b92196fc93eba88ccd616b0bf16a2694e00d4770c`、LL20670B SHA `8b3cceaae9a38d56de285ac5a0e42344c261ca292aa1f952b1344cafa8aa81cb` 两版逐字相同。modem_clock.c/soc_caps整文件有差异，所用callback和independent宏已单独核对，不能称完整驱动一致。全URL/hash、原callback表和typed输入pin见 [sources.json](sources.json)；复用既有固定public缓存，无新fetch时间声明。

```sh
python3 docs/evidence/c5-wifi-clock-requests-2026-11-25/run-owned.py --repo "$REPO" --output-dir "$TASK_TMP"
python3 docs/evidence/c5-wifi-clock-requests-2026-11-25/run-owned.py --repo "$REPO" --output-dir "$TASK_TMP" --compiler clippy-driver --lint
node tools/check-evidence-privacy.mjs docs/evidence/c5-wifi-clock-requests-2026-11-25
```

actual模块standalone58项通过：49旧BootAnalog、1新provider、7请求组件、1跨模块；严格Clippy `-D warnings`及同58项通过。覆盖原RMW/clear、16组合、unknown/false-input事务性、provider撤回和恢复、requested bit不grant、reset、诊断饱和及不制造旧analog readiness。完整stdout只放调用者ignored target，当前树仅 [checks.json](checks.json) 的bytes/hash/结果；没有Cargo/native/WASM/浏览器/benchmark、输入修改、发布或push。源码未接入整机前不提升WiFi/PHY功能声明。
