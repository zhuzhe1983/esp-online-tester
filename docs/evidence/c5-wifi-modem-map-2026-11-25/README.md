# C5 WiFi 回调的10域 ICG map 配置

本扩展只补现有 `C5BootAnalog` 的两个 map 配置字，使原厂 WiFi clock 回调能够执行已确认的逐字段 RMW。不会切换 PMU 到 MODEM/SLEEP，也不提供 RF/PHY、MAC执行、模拟校准、ACK或额外 clock-ready 状态。本 lane 没有运行原厂，以下调用链和word序列是原字节/固定源码的静态证据，不能当作客体已执行或联网、首屏验收。

固定原厂 IDF5.5.3 镜像的 OSI 表 `40818a7c+f8` 指向 `4080b184→40808a40`，后者实际加载参数47并跳 `4080108a`。`408010da..10fa` 循环10项：getter `408030bc`、读初始表、`408010ee` 执行 old|init、setter `40802fe0`。这不是host调用或函数拦截。表 `4081a5e0` 在 FactoryOnly BIN `32df1c`，40B为 `[6,4,6,4,6,4,6,6,6,6]`，SHA256 `9d175b310ef5b078b88a7ee1150b17c193550d78a347987db498d3fb259d33f3`。

固定 [modem_clock.c](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/esp_hw_support/modem_clock.c#L368)、[C5 HAL](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/hal/esp32c5/modem_clock_hal.c#L22)、[SYSCON LL](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/hal/esp32c5/include/hal/modem_syscon_ll.h#L139) 和 [LPCON LL](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/hal/esp32c5/include/hal/modem_lpcon_ll.h#L190) 与原表相符。

| 配置字 / 域 | nibble shift | 支持的bitmap |
| --- | ---: | --- |
| SYSCON `600a9c0c` MODEM_APB | 28 | 0、4、6 |
| 同字 MODEM_PERIPH | 24 | 0、4 |
| 同字 WIFI | 20 | 0、4、6 |
| 同字 BT | 16 | 0、4 |
| 同字 FE | 12 | 0、4、6 |
| 同字 IEEE802154（header称ZB） | 8 | 0、4 |
| LPCON `600af020` LP_APB / I2C_MST / COEX / WIFIPWR | 28 / 24 / 20 / 16 | 各0、4、6 |

[SYSCON header](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/soc/esp32c5/include/modem/modem_syscon_reg.h#L260) 和 [LPCON header](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/soc/esp32c5/include/modem/modem_lpcon_reg.h#L198) 给出4bit R/W字段及POR0。named mask分别 `ffffff00`、`ffff0000`，reserved始终拒绝。field2（MODEM-only）、1（SLEEP）、8（code3）、混合非法nibble和未知位在任何状态下事务拒绝；PERIPH/BT/ZB不因别域使用6就额外获准6。

以此前 guest 配置 `40000000 / 44000000` 为前提，原循环推导的10次word依次是：

```text
SYSCON: 60000000 64000000 64600000 64640000 64646000 64646400
LPCON:  64000000 66000000 66600000 66660000
```

4保留 ACTIVE(code2) 的许可；6增加 MODEM(code1) 的许可。[PMU enum](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/esp_hw_support/include/esp_private/esp_pmu.h#L31) 确定码值，这不是向 PMU 请求state1。当前 ACTIVE requested/captured code2、已应用 permissions7保持。只支持原有code0/2请求，未知PMU state/命令仍拒绝。

## 消费者、更新与复位

绝对 `read/write` API、Bus路由、诊断和profile选择均无新增接口；没有第二个map owner。配置字保留实际guest写入，改变shadow本身不修改已应用permission。已有 `+dc` code capture、`+d0` permission apply 语义与计数不变；它们仍是此前明确声明的functional latch抽象，公开 `need_des`/空描述并未证明硅片的两阶段时序。

现有真实消费者只用 MODEM_APB、master-facing LP_APB、I2C_MST 三个 permission。REGI2C仍要求APB许可、实际gate/FORCE、目标电源/路由/master selector；FORCE仅覆盖I2C functional permission，不能绕过任一APB。新MODEM bit1没有生成PLL/OCODE/SAR/BOD配置或结果、deadline、IRQ、频率或RF就绪。其他七域目前只支持明确配置；后续真实设备clock/reset或PHY请求仍由各自owner处理或Unsupported。

reset_domains保持原有functional合同：CPU-only保留全部map和applied状态；HP清SYSCON/LPCON map与master，返回bootstrap-unmanaged，保留LP域PMU requested/captured code；LP-only清PMU code并撤applied状态、保留HP域map；System同时清HP/LP；Chip恢复全部相关状态/诊断。名称中的LPCON不改变本owner已有HP域声明。没有新增silicon per-flop或睡眠/唤醒保留承诺。SRAM/raw关闭路径的既有行为、报告格式不变，无每tick工作。

## 版本与 owned 检查

来源固定553 `2c211b236707889e8400c4dc5644dd5c4ee071e0`、554 `735507283d5b2f9fb363a1901172dbd9e847945d`；8对完整文件中7对逐字相同，完整URL/bytes/hash见 [sources.json](sources.json)。**modem_clock.c整文件不同**：554引入with_refcnt及device enable/disable策略变化；本文采用的初始map表、init-all与module-enable局部相同。以注释 `the ICG code's bit 0, 1 and 2` 开始，至 `modem_clock_module_disable` 签名前：553行368–410、554行376–418，2057B，SHA256 `d6b53c111cb2ebea281bcb8b52f9a5a5641a32435b7bf13c0c012f41d76a4e52`。不能据此称WiFi驱动全版本一致。

```sh
python3 docs/evidence/c5-wifi-modem-map-2026-11-25/run-owned.py --repo "$REPO" --output-dir "$TASK_TMP"
python3 docs/evidence/c5-wifi-modem-map-2026-11-25/run-owned.py --repo "$REPO" --output-dir "$TASK_TMP" --compiler clippy-driver --lint
node tools/check-evidence-privacy.mjs
```

Rust1.100.0直接编译actual `esp32c5/src/boot_analog.rs`：37项原组件测试与6项专项测试共43通过；owned Clippy `-D warnings`及同43项通过。覆盖每字段16种值、混合组合、reserved/非法bits原子拒绝、完整10word、shadow/capture/apply、reapply门控、reset retention、无新ready/事件。两个旧负例 `SYSCON44000000`、`LPCON00400000` 因本次已定义的PERIPH/COEX ACTIVE字段变合法，分别换为同位置非法bitmap2；其余旧期望未改。测试结果、工具/源码hash及此更正见 [checks.json](checks.json)。

没有运行Cargo/shared build、native factory、WASM、浏览器或benchmark；没有修改Bus/lib/Soc/PMU/board/shared文件或输入，未push。后续客体边界及整机结果由父级在集成后独立保存。


完整owned测试/lint stdout已按[证据保留规范](../README.md)移出当前源码树；原bytes/SHA、来源提交及逐字校验的ignored target位置见[保留清单](../c5-owned-stdout-retention-2026-11-25/manifest.json)。checks中的既有artifact hash仍标识原始输出，未改为其他文件hash；结果、命令及原负面记录保留。重新运行harness时将`--output-dir`设为调用者的ignored输出目录，全文不再复制回Git。
