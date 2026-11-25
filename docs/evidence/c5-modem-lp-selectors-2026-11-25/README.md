# C5 MODEM LP selector 配置子集

仅新增三个精确word：`600af004`是MODEM BLE RTC timer LP selector，`600af008`是COEX，`600af00c`是WIFI；不是HP_SYSTEM CPU timeout，也不是独立LP_TIMER `600b0c00`。 [modem/reg_base.h](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/soc/esp32c5/include/modem/reg_base.h)固定MODEM_LPCON base=`600af000`。[header](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/soc/esp32c5/include/modem/modem_lpcon_reg.h)、[LL](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/hal/esp32c5/include/hal/modem_lpcon_ll.h)和[C5 HAL](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/hal/esp32c5/modem_clock_hal.c#L132-L249)给相同选择位及POR0。

三个word只接受完整值0/1/2/4：bit0选raw RC_SLOW virtual136kHz，bit1选raw RC_FAST virtual17.5MHz，bit2选XTAL48MHz；0表示不选任何源，输出观测None。只支持divider[15:4]=0。bit3的共享32K路径涉及RC32K/XTAL32K/external32K和另一个selector，全部unsupported；multi-source、非零divider及reserved bits事务拒绝。没有整片RegRam、fallback source或ready/status寄存器。

`selected_source_hz(address,ClockInputs)`只报告**functional gate之前的所选裸源**，不是模块运行频率或RF就绪。ClockInputs的rc_slow_hz/rc_fast_hz/xtal_hz默认全None；只承认对应136000/17500000/48000000，None、0及其他值不可补频。没有数值ADC校准、RF状态、WiFi成功、MAC/IRQ、BLE timer执行、sleep或真实熵。

为了避免混用selected clock，本提交给BootClock增加`rc_fast_osc_hz()`和`rc_slow_osc_hz()`：前者只由实际FOSC power+DFREQ100产生，与HP bridge gate和LP_FAST mux独立；后者保持实际guest DIG DCAP128条件。`rtc_slow_hz()`仍在裸慢源上检查LP mux；原频率/phase合同不变。纯fixture覆盖LP_FAST=XTAL48、HP gate关闭时raw RC_FAST仍17.5MHz，以及未配置/掉电/profile off返回None。不得用lp_fast_hz或hp_rc_fast_hz冒充modem裸RC_FAST。

APB可达性是父级显式输入：read/write均要求`lp_apb_accessible=true`。父级新BootAnalog getter只承认已实际apply的Some(permissions) bit1(LP_APB)，不继承I²C master的bootstrap None宽例，也不拿I²C FORCE代替APB许可。旧BootAnalog继续拥有`600af018/1c`的I²C clock/force和`600af020`的domain map；本模块不复制它们，不把source selector当成domain/functional gate。APB gate改变不擦除选择配置，也不证明下游clock正在运行。

实际typed caller`420031f2`对应 [modem_clock_deselect_all_module_lp_clock_source](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/esp_hw_support/modem_clock.c#L453-L462)：顺序WIFI(`42070cc8`)、BLE RTC(`42070b10`)、COEX(`42070bec`)。每个HAL按bit0/1/3/2分别读改写清0，共12次真实store；POR全0时每次仍写0，累计writes预期4/4/4。当前原厂仅在第一个WIFI read `PC42070cca/address600af00c` fault，尚未读到值或执行这些store。见 [checks.json](checks.json)与[source回执](sources.json)。

IDF5.5.3=`2c211b236707889e8400c4dc5644dd5c4ee071e0`；5.5.4=`735507283d5b2f9fb363a1901172dbd9e847945d`。header、LL、C5 HAL、modem reg_base两版本相同；generic modem_clock.c完整文件不同，5.5.4修改with_refcnt管理，尤其I²C_MASTER不重复内部refcount。当前三组selector/deselect路径保持相同，不能把版本比较写成所有文件相同。TRM1.1缓存文本没有MODEM_LPCON/WIFIPWR_LP/COEX_LP字段描述，保留公开覆盖不足，不借HP_SYSTEM地址别名或猜divider/RF行为。

API：`ModemLpClock::{new,contains,read,write,selected_source_hz,reset_domains(hp,analog),report(ClockInputs)}`。HP/Chip清3word及诊断，CPU/LP保留，是与已有LPCON控制一致的明确functional reset合同，非per-flop硅片保留证据。父级负责strict原ROM profile、LP_APB getter、Bus/lib/module字段与reset/退出接线；raw退出应撤对象/路由。此提交只新selector模块、两个clock裸源getter和纯tests/docs，未改Bus/lib/Soc/BootAnalog、LP_TIMER或固件。

```sh
python3 docs/evidence/c5-modem-lp-selectors-2026-11-25/run-owned.py --repo "$REPO" --output-dir "$TASK_TMP"
clippy-driver --edition=2021 --test "$TASK_TMP/c5-modem-lp-harness.rs" -o "$TASK_TMP/clippy-tests" -D warnings
node tools/check-evidence-privacy.mjs
```

31项纯组件测试通过：24项clock（新增1个bare-source反例）+7项selector。严格owned Clippy通过；无Cargo/native/WASM/browser/benchmark。父级当前边界24768743 instructions/24768704 nominal cycles/UART2194，8MiB PSRAM memory test OK且LP mux已为0，但request/feeds/pulse均0；此pre-pulse prefix没有验证动态SWD。完整日志留忽略缓存，只提交必要hash/数值和固定来源，不宣称本模块已越过原厂fault或完成app_main/LCD。

完整逐 test stdout 已移至 ignored 输出；原路径、bytes/hash、固定 artifact/source receipt 与重现命令见[留存清单](../c5-evidence-retention-final-2026-11-25/stdout-manifest.json)。测试结果、负结果和历史 raw/normalized hash 保持不变；此留存整理没有运行组件或原厂机。
