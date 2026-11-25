# LCD273 SHTC3 有时序数字子集

基于仓库 `aac053f097f22727bfa2b7a2b631e1e8718acd56`，仅新增共享器件
`esp-periph/src/i2c/shtc3.rs`、模块注册和 LCD273 接线及专项检查。
没有修改 I²C 控制器、trait、Bus、SoC、固件或 ROM。

固定厂家提交
[`418de818f4f40c5a04c4292dbe293bfaa1351068`](https://github.com/waveshareteam/ESP32-C5-LCD-2.73/tree/418de818f4f40c5a04c4292dbe293bfaa1351068)
的原理图标明板载 U7 为 SHTC3，VCC3V3、SDA27/SCL26，与 RTC/QMI 共用
HP I²C0；7-bit 地址为 `0x70`。只接 LCD273，不推定其他 Waveshare 板的连线。

## 现有实现与 PR 检查

实现前检查了当前仓库与上述基线：已有 SHT4x、ENV-IV，没有 SHTC3。
SHT4x 使用不同命令和换算，因此不把它改名充当 SHTC3。
[open-pr-audit.json](open-pr-audit.json) 保留检查时上游全部六个开放 PR 的文件列表，
本 fork 无开放 PR。#382 的当前 head 为 `964feccb0d19223f7cef8203c85967f33098864b`，
新增 AHT20/BMP280/LSM6DS3 等器件，模块注册补丁中没有 SHTC3。
这是实现前的时点检查，不保证以后没有新的 PR。

## 固定来源与原厂调用

主要协议依据为 Sensirion
[Datasheet SHTC3，Version 4，2022-12](https://sensirion.com/media/documents/643F9C8E/63A5A436/Datasheet_SHTC3.pdf)，
743217 B，SHA-256 `dbe979c975fe520663606b5c278d2b9130f4e402e6d9d943ca06c4e4f38e63d9`。
另外核对了官方
[embedded-sht 固定版本](https://github.com/Sensirion/embedded-sht/tree/94967774f25985bf65356bad3689fec4e8fef2b8/shtc1)，
其 `shtc1.c/h` 明确支持 SHTC3 的相同唤醒、睡眠和测量命令。
没有添加该库额外的序列号协议。

所有新读取文件的 URL、最终 URL、时间、媒体类型、长度与 hash 见
[sources.json](sources.json)。厂家 SHTC3 C/H、sdkconfig 和原理图均固定到同一厂家
提交；未提交第三方原始二进制、PDF 或私有运行日志。

| 原厂阶段 | 固定厂家驱动 | 新模型 |
| --- | --- | --- |
| 唤醒 | `35 17`，成功才等待 240µs | 必须完整两字节；进入 240µs 客体时间等待 |
| 产品 ID | `EF C8` 后 repeated START 读两字节，不读 CRC | 支持两字节提前 STOP；完整读取另有 CRC |
| Sensor 页测量 | `78 66`，设备 100kHz，延迟 20ms 后接收六字节 | 温度在前、无拉伸、正常模式，等待 12.1ms 才允许读取 |
| 睡眠 | `B0 98` | 清除未读结果；睡眠期间只接受唤醒 |
| 软件复位 | 头文件声明 `80 5D`，该路径未调用 | 仅 idle 接受，清内部状态并等待 240µs；环境刺激保留 |

固定厂家 BSP 忽略 `shtc3_get_id()` 的错误。读取失败后打印的 ID 是未初始化栈
内容，不能作为真实 ID；测量函数也忽略收发错误并无条件返回 ESP_OK。本模型
没有把这些返回值当作测量证据，也没有采用运行中出现的 `0x2638`。

## 状态、环境与明确限制

共享 API 为 `Shtc3State::new(hz)`、`advance_to(cycle)`、
`set_environment(temperature_c, humidity_rh)` 与 `set_power(powered)`；
`Shtc3::new(Arc<Mutex<Shtc3State>>)` 实现现有 `I2cDevice`。
环境 setter 拒绝非有限值、−40…125°C / 0…100%RH 以外的输入，失败不改原值。
测量在完整命令的第二字节接受时锁存环境；之后修改环境不覆盖在途结果。

LCD273 当前使用**固定虚拟 25°C / 50%RH**，名义 USB 3V3 供电，报告中明确标注。
现有网页环境消息只分发到 S3/C61 的已配置配件，未扩展到 C5；不宣称网页可调
LCD273 的环境。共享器件的 setter 可供后续独立接线使用。

- 上电、唤醒、idle 软件复位采用 240µs；四个无拉伸测量命令
  `7866/58E0/609C/401A` 分别提供温度或湿度在前、正常 12.1ms 或低功耗 0.8ms。
  时长选择 datasheet Table5 的最大值，是明确的确定性数字策略，无主机时钟、
  事务计数、零延迟或物理转换标定。
- 测量忙期和启动等待期间，读写地址都 NACK。睡眠时写地址须能接收唤醒命令，
  其他命令不创建响应；残缺、未知和四个拉伸命令被拒绝。没有通用寄存器 RAM、
  通用调用复位、序列号读取、SCL 拉伸或主机 ACK 波形模型。
- 两组数据各附 CRC-8，poly31、initFF、无反射、xorout00；独立公开向量
  `BEEF → 92`。换算分母为 65536，四舍五入到代码并限制到 0…65535。
  虚拟 25°C / 50%RH 的测量字为 `6666/8000`。原厂用了 65535，显示值的微小
  差异属于原厂换算，不倒改传感器数据迎合它。
- ID 的已规定产品位是 `(id & 083F)==0807`；未规定的位取零，因此虚拟
  产品 ID 为 `0807`，另计算正确 CRC。这不是原厂实物身份或唯一序列号。
- 当前子集在接受 read header 时消耗待读响应，STOP 可提前结束两字节 ID。
  无新命令的结果重读未建模并 NACK；完整 ID/测量之外的读取返回 FF。
  现有 trait 不传递逐字节 master ACK，不能据 STOP 单测宣称已复现其波形。
- controller/CPU reset 保留有电的器件及转换期限；只有模型供电边沿清状态。
  单调客体时间不倒退；未来期限无法用 u64 表示时保持忙，不回绕为立即完成。

## 实际客体时间接线

检查了基线中的真实调用链，而非仅查看 trait 的声明：

1. `C5Bus::advance()` 增加客体 cycles；I²C deadline 到期调用 `C5I2c::sync()`。
2. `C5I2c::read/write()` 也先调用 `sync()`；`sync()` 在控制器字节回调前实际
   调用 `bus.board.advance_to(bus.cycles)`。
3. LCD273 已有 `advance_to()`；这里只追加共享 SHTC3 状态的时间同步。
4. C5 reboot 保存原 board 对象和 cycles；CPU-only reset 不重建外设。

相应三个未修改源文件的 hash 见 sources。空闲时不必每 tick 运行传感器：它没有
自治 IRQ，在下一次访问时按真实客体 cycle 判断期限即可。大批追赶时器件看到
当前同步 horizon，不是逐字节校准过的物理时刻。新 MMIO 用例采用不超过 256
cycle 的 deadline 步进，避免一次大幅 advance 掩盖上电等待；该用例尚待集成运行。

## 已运行检查与待运行检查

独立 harness 直接 include 本次器件文件，并提取当前未修改的 `I2cDevice`
声明，无复制引擎、Cargo、CPU、控制器或固件执行：

```sh
python3 tools/check-shtc3-model.py --receipt docs/evidence/c5-shtc3-2026-11-25/isolated-tests.json
clippy-driver --edition=2021 --test -D warnings --crate-name shtc3_contract target/shtc3-model-check/shtc3.rs -o target/shtc3-model-check/shtc3-clippy-tests
```

7 个器件测试全部通过；独立 Clippy 与编译均无警告。测试覆盖期限前一 cycle 和
恰到期、四种测量次序、独立 CRC/代码向量、非法命令和半命令、睡眠/复位、
在途采样锁存、供电边沿和极端时间。[isolated-tests.json](isolated-tests.json)
保存实际 sensor/trait hash、工具版本及聚合结果。

LCD273 另新增两个 MMIO 用例：原厂命令与实际时间推进、正确/错误引脚及地址、
控制器和 CPU reset 后的忙状态保留。这两个用例及完整构建由集成侧运行，本 lane
尚未运行；应执行：

```sh
cargo test -p esp-periph i2c::shtc3::tests
cargo test -p esp32c5 --test lcd273
```

未运行 guest、WASM、浏览器、基准或物理设备，不宣称原厂完整 Sensor 页已验收。
`git diff --check`、JSON/Python 检查与证据隐私扫描通过后提交；原始公开资料留在
忽略目录，只保留来源收据、hash 和聚合结果。未修改历史回执；省略私有日志与
个人路径，不影响上述纯模型测试的复现。
