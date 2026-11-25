# C5 原厂代码的离线 Wi‑Fi 驱动兼容合同

`waveshare-lcd273-idf553-offline-v1` 是**明确选择、默认关闭**的离线驱动兼容 profile。它帮助固定 Waveshare ESP32-C5-LCD-2.73 原厂应用继续执行，模式标记为 `compatible-driver-assisted`；严格硬件路径独立保留。该模式没有原生 Wi‑Fi/MAC/PHY/RF、关联或 IP，也没有静默回退、猜测私有寄存器 ready 值或修改 BIN/ROM 代码。

本合同对应内核 `7625addbe0c5164c80b60bb0e142be91134c7a23` 的固定 ABI、状态政策与历史组件收据。完整字段、签名、摘要和范围见 [abi.json](abi.json)。**最终组合源码/WASM、真实浏览器输入与存储、回归、CI 和 Pages 须分别验收。**

## 输入与可变数据范围

固定厂家来源为 [waveshareteam/ESP32-C5-LCD-2.73@418de818](https://github.com/waveshareteam/ESP32-C5-LCD-2.73/tree/418de818f4f40c5a04c4292dbe293bfaa1351068)，IDF 为 [5.5.3@2c211b23](https://github.com/espressif/esp-idf/tree/2c211b236707889e8400c4dc5644dd5c4ee071e0)，其官方 Wi‑Fi 归档子模块为 `espressif/esp32-wifi-lib@e0befaa593277b4e80726079fbd521b4681754c2`。只适用于 `waveshare-c5-lcd273` 的32MiB Flash/8MiB PSRAM。

原 FactoryOnly BIN 为3346992B，SHA256 `cf10a4358d03898458c16390f3f16ceb0306befe2260160de0919d755b920942`；rev100 ROM 为472300B，SHA256 `49693d745eb4b4e6e6c77be4f5213765d545d8c7a1d9742a8a619bf596c4e27a`。没有匹配的厂家 ELF/map；descriptor给出的原 ELF SHA为 `4cc67d1a20db1693509b2921da7d6bcb4ac7179213a5deff10520d5d60881244`，不能把反汇编容器或独立源码例的 ELF 称为该原件。

profile核对以下不可变范围，区间均左闭右开：

| 范围 | 字节 | SHA256 |
| --- | ---: | --- |
| bootloader `0x2000..0x8000` | 24576 | `3ce04f6bd5152da65dcc375b9ff8872714af9fb128b9519bb99a8184f93ba190` |
| partition table `0x8000..0x9000` | 4096 | `ec03ffe0e58a44ea48920963b3dd0e8b8a40f7b414fed5c2a44895843bea3baf` |
| 原厂 app `0x20000..0x331230` | 3215920 | `69d6362c81efc6d55048723a1af0c864b26e1990b6a8028d8fc127072bbd5f28` |

NVS `0x9000..0xd000`、otadata `0xd000..0xf000`、phy_init `0xf000..0x10000` 不在这三个哈希范围内，已有字节保留。范围之外仍受原 Flash/分区/CPU权限规则约束。每个入口须通过当前 MMU 映射读取预期物理 Flash 的固定32B context。不可变范围哈希与入口物理绑定共同限定 profile；它们不证明每个活动页面、调用者或完整 guest 控制流的身份。

入口物理校验自 `2a70059e` 生效，涵盖14个 API、其 host continuation，以及转入原生 `esp_event_post` 前的入口。每个入口逐字节 `i=0..31` 要求：

```text
translate(pc+i) == Flash[pc-0x42000000+0x200000+i]
```

原 IROM 段是虚拟 `42000020` → 文件物理 `200020`，所以物理映射基址为 **`200000`**，而 app 分区起点为 `20000`。当前 MMU缺失、无效、映射到PSRAM或同字节的其他物理页均不合格；通过后才比较32B签名。MMU页大小取当前配置，API Fetch及原生 post 的CPU权限照常适用。JSON逐项列出物理偏移；disconnect公开wrapper本体4B，核对context为含后续固定代码的32B，二者签名分别保留。

同布局应用搬到 OTA0分区 `510000` 时，相对原分区偏移 `4f0000`：init 从要求的 `2900c8` 变为 `7800c8`，原生 post 从 `31e894` 变为 `80e894`，均被拒绝。此为源码与整数推导，**未执行 OTA guest 测试**。仅将这些入口页映回原 app0 的混合 MMU 布局也不能获得整程序身份认证。卡、NVS/otadata/PHY数据不为获得资格而清除或改写。

## 核定入口与 ABI 来源

以下15个边界中，14个属于实际驱动 hook，`esp_event_post` 保持原生 guest执行。入口通过原BIN实际调用/错误表达式/参数构造、固定 IDF 源码及官方归档 `libnet80211.a:ieee80211_api.o` 的真实符号/relocations交叉核对；不是厂家链接ELF符号认证。每行的原始 entry/context 字节、SHA和来源等级均保留在JSON。

| API | 固定边界 | 处理方式 |
| --- | --- | --- |
| `esp_wifi_init` | `420900c8` | 152B初始化合同 |
| `esp_wifi_set_mode` | `420d164e` | 受限 NULL/STA 状态 |
| `esp_wifi_set_config` | `420d1c8c` | 184B STA参数与 RAM影子配置 |
| `esp_wifi_start` | `420d169e` | 通过真实guest post排STA_START |
| `esp_wifi_connect` | 公开 `42090380` → 驱动 `420d1884` | 公开4B tail仍原生执行，**hook实际在已核定的 `esp_wifi_connect_internal=420d1884`** |
| `esp_wifi_disconnect` | `42090384` | 原生公开wrapper边界，始终未关联时返NOT_CONNECT |
| `esp_wifi_get_mac` | `420d0dd4` | 现有虚拟eFuse身份，6B输出权限检查 |
| `esp_wifi_internal_reg_netstack_buf_cb` | `420d21f0` | ref/free代码指针须可Fetch |
| `esp_wifi_get_mode` | `420d0d9e` | 4B enum输出 |
| `esp_wifi_get_config` | `420d1d20` | 读回本次RAM配置 |
| `esp_wifi_stop` | `420d16e2` | 真实guest post排STA_STOP |
| `esp_wifi_deinit` | `420900b4` | 检查停止及pending状态，清驱动RAM；`4208ffa2`是内部析构，不是公开入口 |
| `esp_wifi_scan_start` | `420d1a36` | 明确在本首屏profile外 |
| `esp_wifi_scan_get_ap_records` | `420d1be8` | 同上；其count ABI本来是u16，不能作u32写入 |
| `esp_event_post` | `4211e894` → `4211f368` | 原生default-loop/FreeRTOS queue，不直接host调用应用handler |

参数采用这些整数C API的RV32 psABI。init/crypto/OSI分别为152/44/488B，版本与magic、固定命名参数受约束；STA184B、scan44B、断开payload41B。独立原样02源码例已经存在的 GCC14.2对象 DWARF用于核对结构布局；它不是厂家FactoryOnly ELF。JSON保留这些对象和原DWARF dump哈希，不复制其私有debug内容。

早期草稿错误地把两组STA位域当作独立word，并将identifier推为152；该推导已撤销，原草稿MD/JSON SHA均保留。实际rm/btm等为word128 bits16..21，HE/VHT为word144 bits8..20，identifier151..182，183为padding。`threshold.rssi_5g_adjustment`是offset124/u8，原小表遗漏；实现按该布局检查。padding不能冒充必须为0的语义字段。

## 虚拟状态、事件与存储差异

状态为 uninitialized、initialized、started-offline 或 unsupported。重复初始化、未初始化、未启动、错接口/模式、空SSID、无效密码及非法输出均有具体错误或明确ModelLimit。AP/APSTA/NAN和扫描不会统一返回成功；其余未hook代码仍是普通guest执行和严格硬件边界，不等于实现了完整SDK。

STA_START2、STA_STOP3和NO_AP_FOUND201的STA_DISCONNECTED5均经原生 `esp_event_post`：base值是 `42140704`，不是全局pointer变量地址 `422f3e5c`。真正默认loop在 `*(4083f304)`；队列复制payload后交原netif和应用handler，正常FreeRTOS抢占/重试继续执行。41B虚拟失败数据中BSSID/RSSI为未知，不是测量；明确采用0 guest-delay政策，不宣传为射频扫描耗时。禁止CONNECTED或GOT_IP。

96B guest stack frame最多支持8个pending，以hook PC/SP/RA识别。完整Fetch/Load/Store资格、实际CPU的APM/PMA/PMP context和全span事务预检成立后才提交；post返回前验证callee-saved，再完整恢复栈/寄存器。排队失败按同一lifecycle回滚，旧START失败不得撤销新STOP。已经成功提交到原guest队列的事件不能被追溯改称“尚未投递的host计划”。

驱动配置仅存于模型内RAM，deinit/reset丢弃。IDF原API默认FLASH、set_config承诺NVS保存，本profile**没有实现这一持久化承诺**；不得声称完整drop-in兼容。shim不清NVS、不写伪校准blob；原应用自己的NVS读取/写入仍由guest执行。无IP时不触发只在GOT_IP分支保存凭据的应用逻辑。

每次HLE setup/return只计1个明确披露的名义scheduler slot/cycle，不加architectural MINSTRET。原生event-post指令仍正常执行/退休。`hookCalls`、`virtualSlots`与原生退休指令不可混称，也不用于伪造硬件速度。

## 修复证据及验收范围

`8a19152d` 的权限context/Fetch资格P1、模式deselect与事件事务P2已在 `2a70059e` 修复；源码closure与原始问题报告均保留。后续valid-magic Load哨兵揭示PMA fixture把允许Load的0x10误作Execute；7625改为Fetch-only `0xc0000005`，保留原失败。真实pending软件IRQ可能抢在兼容fetch拒绝前修改trap CSR的反例也保留，7625的observer提前停止该路径。

JSON保存相关原报告、源码、负例/修后日志哈希，并分别标明静态源码核对与历史组件执行收据。2a的已记录native严格结果仍停 `408142c6`/READ`600ad858`；其辅助模式菜单帧与5次原生post、4次no-AP失败属于**2a的历史组件证据**。7625的14项dev合同和组件Clippy是另一份收据，不把旧菜单运行移作7625或最终WASM的执行。各收据只覆盖其记录的源码、输入和命令。

该组件证据未认证原厂按钮、文件读写、最终组合生产WASM/浏览器、性能、完整CI或Pages。严格FactoryOnly及原生RF接受均维持false。后续运行和修复须有各自的源码、输入和验收收据，不回填为2a或7625的历史结果。

原BIN/ROM、SDK/归档、UART、屏幕及完整日志仅保留于ignored私有存储；这里公开的是必要签名、数字、来源和哈希，没有开发者绝对路径、PID、session信息、凭据或无关inventory。原件、修正前合同及负例均独立保留。**本收据没有提供原件公开下载**；远程复核需要调用者另行提供匹配输入或复现，哈希不是下载链接。source revision/tree仅标识被核对的组件；最终公开revision/tree映射在发布记录中另列。
