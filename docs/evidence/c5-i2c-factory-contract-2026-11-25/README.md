# C5 原厂 I²C 主机中断合同

对象为 Waveshare ESP32-C5-LCD-2.73 固定 FactoryOnly 镜像，厂家提交
`418de818f4f40c5a04c4292dbe293bfaa1351068`，镜像 SHA-256
`cf10a4358d03898458c16390f3f16ceb0306befe2260160de0919d755b920942`。
本改动基于仓库提交 `5ae6002bc21b190bd23f01d274094d258931f184`。

`esp32c5/src/i2c.rs` 原先只接受主机 `INT_ENA` 的 `0x488`；固定 IDF 驱动在
发送前启用 `0x5a8`。现允许保存新增的仲裁丢失、超时使能位，沿用实际字节执行
产生的 END、完成和 NACK。单主机、无 SCL 拉伸模型不产生不存在的仲裁或超时。
未修改固件、ROM、共享 I²C 引擎、器件响应、SoC 总线或中断路由。

## 固定来源与版本比较

原固件锁定 IDF 5.5.3 提交 `2c211b236707889e8400c4dc5644dd5c4ee071e0`。
原有 C5 寄存器表采用 IDF 5.5.4 提交 `735507283d5b2f9fb363a1901172dbd9e847945d`。
本次重新读取两版本的 13 对官方源文件，逐字节相同：C5 `i2c_ll.h`、
`i2c_reg.h`、`i2c_struct.h`、`pcr_reg.h`、`reg_base.h`、GPIO 信号表、中断表、
时钟枚举、`i2c_periph.c`、公共与主机驱动、HAL 和 IRAM HAL。
各来源的完整固定 URL、最终 URL、抓取时间、媒体类型、字节数与 SHA-256 见
[source-receipt.json](source-receipt.json)；没有用其他芯片的测量替代 C5 证据。

| 项目 | 固定 C5 来源与语义 | 当前范围 |
| --- | --- | --- |
| 中断位 | [LL 53–74](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/hal/esp32c5/include/hal/i2c_ll.h#L53)：NACK10、TIMEOUT8、COMPLETE7、ARBITRATION5、END3 | 使能掩码 `0x5a8`；只由真实已建模事件置位 |
| RAW / CLR / STATUS | [C5 寄存器定义](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/soc/esp32c5/register/soc/i2c_reg.h#L439)：RAW 为 WTC，CLR 写 1 清除，STATUS 为已使能状态 | RAW 与 ENA 独立；清除不相关原因不会吞掉 NACK 或完成 |
| ACK | 同一寄存器文件 171–175：RESP_REC 为 0 表示 ACK，1 表示 NACK | 保留 C5 明确极性，不继承其他芯片的相反约定 |
| 控制器 / PCR | `reg_base.h`；`pcr_reg.h` 261–320；LL 148–163、816–829、895–904 | HP I²C0 `0x60004000`；PCR `0x60096020/24`；使能、复位与 XTAL 源门控仍有效 |
| 矩阵 / IRQ | `gpio_sig_map.h`、`i2c_periph.c`、`interrupts.h` | SDA47、SCL46、源56；必须同时匹配器件的实际 SDA/SCL 引脚 |
| 时钟 / 分频 | LL 103–125、172–208 | 显式 48 MHz XTAL；NUM+1+B/A；计数生成字节期限。RTC 请求 400 kHz，SHTC3 请求 100 kHz，不能混用 |
| 超时配置 | LL 321–333、1008–1013；主机驱动 703–714 | 保存指数与使能，不模拟拉伸超时；未因普通空闲、成功或 NACK 而置 TIMEOUT |
| 仲裁 | HAL `i2c_hal_master_init()` 关闭仲裁；ISR 仍预先使能仲裁错误 | 无第二主机，不造仲裁丢失；允许保存错误使能不表示实现多主机 |

[主机驱动 679–737](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/esp_driver_i2c/i2c_master.c#L679)
配置时序和超时、清 FIFO，再 OR 入完整主机中断掩码。同步调用最终检查状态是否
DONE；其他状态聚合为 `ESP_ERR_INVALID_STATE=0x103`。因此单独的 `0x103`
不能证明发生地址 NACK，更不能成为强制 ACK 的理由。

同一驱动 791–808 的 ISR 先读 STATUS 并写 CLR，优先处理 NACK，其次是
TIMEOUT/ARBITRATION，随后是 COMPLETE。END 单独释放命令阶段，不能当作
整笔事务成功。LL 1103–1120 是另一套旧驱动事件分类函数，其优先级不同；
本镜像的新主机驱动采用前述 ISR。

## 原厂指令的静态交叉核对

输入为只读缓存的 typed 反汇编，未重新解释或改写原镜像。app 段反汇编为
28,946,111 字节，SHA-256 `3e23c10184628e7c5dcea63972581d1683ba76345ad250af08dbb22c43646a45`；
IRAM 段为 1,525,607 字节，SHA-256 `4696bac57e7e7a848e73c86de7d0513c81e6ce5ec3e9483e41f6866b3cc7ba41`。
指令字也保存在 receipt；不提交私有反汇编原件或其中的本地路径标签。

| 地址 | 观察 |
| --- | --- |
| `0x420ee890/94` | HAL 选择 HP 控制器基址 `0x60004000`；LP 分支为独立的 `0x600b1800` |
| `0x4211d51e/520/524` | 读 offset `0x28`，ORI `1448=0x5a8`，写回 offset `0x28` |
| `0x4211d5b2/5b6` | AND `~0x5a8` 后禁用对应使能 |
| `0x4211ddf6/dfa` | 初始化时将 `0x5a8` 写到 offset `0x24` 清除旧状态 |
| `0x4211e026/02a` | 分配中断时传入 `0x5a8` 与 STATUS 地址；处理函数地址为 `0x408074bc` |
| `0x408074d4/d8` | 处理函数读 offset `0x2c` 并写 offset `0x24` |
| `0x408074de`、`0x4080750c`、`0x40807526` | 分别检查 `0x400`、`0x120`、`0x80`，与固定源文件的新驱动一致 |

这些是静态指令证据，不证明原厂客体已执行至 I²C 或应用已启动。

## 紧邻驱动的未支持边界

正常初始化和短 FIFO 事务的寄存器路径中，未发现需要把未知偏移开放为通用 RAM
才能继续的访问。此次仅补已经定位的 ENA 掩码。FIFO 水位、从机、SCL 状态超时
等中断使能仍返回 Unsupported；非 FIFO 模式、FIFO RAM 别名、SCL_SP_CONF
`0x80`、SCL_STRETCH_CONF `0x84` 和 LP I²C 仍未支持。

超时或 BUS_BUSY 触发的恢复路径会调用主机驱动 `s_i2c_master_clear_bus()`，
经 LL 773–791 访问 `SCL_SP_CONF` 的九脉冲清总线状态机。该路径会明确停在
`0x60004080`，不能用读回“已完成”代替真实清总线。普通 NACK 的同步错误复位
走 `clear_bus=false`，并不必然访问该偏移。

共享引擎既有 NACK 行为会停止后续命令并清掉选中器件集合；不能据本次掩码扩展
推定器件已收到后续 STOP 回调。新的 END/STOP 用例分别观察两种事件；NACK
用例确认失败时尚未执行程序中的 STOP。无多主机冲突、时钟拉伸、总线恢复脉冲
或物理芯片验证，也没有新增 SHTC3、固定成功帧或通用 ACK 器件。

## 检查和复现

静态检查命令：

```sh
python3 docs/evidence/c5-i2c-factory-contract-2026-11-25/verify-sources.py --source-root target/c5-i2c-source-contract --fetch
```

有原审计 typed 文件时，另传 `--app-typed <app.typed.dis>` 与
`--iram-typed <iram.typed.dis>`，先校验整个输入 hash，再核对 14 个指令字。
[verification.json](verification.json) 记录本次仅静态检查的结果。

控制器用例位于 `esp32c5/tests/i2c.rs`，新增三个用例，共五个：完整使能掩码、
选择性 WTC/CLR、使能与 RAW 独立、END/STOP 区别、地址与数据 NACK、拒绝的
额外使能位、失败后的真实重试。测试设备仅为该测试的协议探针，不代表板载传感器。
本审计尚未执行 Rust 控制器测试；统一集成检查须运行：

```sh
cargo test -p esp32c5 --test i2c
```

`git diff --check` 与证据隐私扫描另行执行。原厂 guest、native/WASM 全链、完整
CI 和两个 Clippy 由集成检查负责；此报告不把静态合同称作整机兼容验收。

隐私处理：只保留公开来源、固定 hash、指令地址与字和聚合检查结果；省略原始
反汇编、日志及其路径标签，不含用户名、设备标识或无关进程记录。没有修改既有
历史证据，因此无需替换其原始/脱敏 hash；上述输入 hash 仍对应未改动原件。
