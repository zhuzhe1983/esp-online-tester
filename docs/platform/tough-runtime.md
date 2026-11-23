# M5Stack Tough K034 运行支持

Tough K034 使用经典 ESP32 的 `m5-tough` 板型，Flash 为 16 MiB。实物规格列出 8 MiB PSRAM，此后端映射 4 MiB。现有外壳采用厂家 CAD，屏幕位置沿用原有照片估算。原模型的正面轮廓对照仍有偏差。 [规格](https://docs.m5stack.com/en/core/Tough) [模型报告](evidence/m5stack-k034/verification.json)

| 部件 | 实现 |
| --- | --- |
| ILI9342C | 320×240 横屏，SPI CLK G18、MOSI G23、MISO G38、CS G5、D/C G15。读取显示 ID，接收 DCS 窗口、状态和像素数据。AXP192 的 LDO2 控制屏幕电源，GPIO4 释放复位，LDO3 控制背光开关。 |
| CHSC6540 | 内部 I²C SDA G21/SCL G22、地址 0x2E，低电平触摸 IRQ G39。支持 0x5A 中断模式和 0x02 起始的单点报告。A/B/C 通过同一触摸报告上报，默认键为 A/B/C。按区中心依据旧版 M5Tough 库，坐标和触摸板复位接线仍待实机核对。 |
| BM8563 | 内部 I²C 地址 0x51，复用客体设定的日历和虚拟时间。RTC 唤醒线、闹钟和实际晶振误差未模拟。 |
| microSD | 工作台备有可配置的 FAT32/IMG 卡，固件尚不能访问卡。 |
| 其他 | RS485、6–24 V 输入、I²S 扬声器、PDM 麦克风、物理电源键、机身 EN 按键和模拟电压等待实现。ILI9342E 批次仍待单独验证。 |

官方资料和原理图把 LCD 复位标作 AXP192 GPIO4。2023 年 M5GFX 还在初始化时单独写 GPIO1 的触控复位寄存器。当前板型以 GPIO4 和 LDO2 作为共享的数字复位门控，保留此差异。旧版 M5GFX 的显示 ID 读取先单独送一个空时钟，再请求 32 位数据。经典 SPI 接口把该空时钟取整为一字节，板型在目标引脚和 RDDID 命令匹配时保留后续 ID 的第一字节。 [原理图与来源收据](evidence/tough/sources.json) [历史 M5GFX 驱动](https://github.com/m5stack/M5GFX/blob/b9639027dca4a4f97bb9d637056bf95b1d8d7c80/src/M5GFX.cpp#L457-L469)

## 固件与验收

目录默认加载 Tough Tools v1.0.2。M5Stack 将它发布为设备测试程序，出厂预装镜像仍未定位。固件由发布方 CDN 提供，加载时核对固定 SHA-256。 [M5Burner 发布条目](https://burner.m5stack.com/firmware/1897200000000002585)

原样镜像通过 ESP32 掩模 ROM 启动，客体识别到 M5Tough 并绘出 320×240 菜单。等待菜单初始化后，在屏幕坐标 `(60,221)` 按下再释放，TOUCH 标签变黄。按住 `(147,108)` 时出现蓝色点。单独一轮在 `(260,221)` 触摸可打开 RTC 页，初始日期显示 2000-00-00，仍需由客体设定。固件尝试更新触摸控制器时报告失败，随后能进入测试程序。 [原生运行收据](evidence/tough/runtime-native.json)

```sh
cargo test --release -p esp32 --lib board::m5_tough
cargo build --release -p esp32sim --bin esp32sim

cat > tough-actions.txt <<'ACTIONS'
4.0 touch 60 221 1
4.25 touch 60 221 0
5.5 touch 147 108 1
ACTIONS

target/release/esp32sim --chip esp32 --board m5-tough --boot rom \
  --rom web/wasm/fw/esp32_rev300_rom.elf --flash-image "$TOUGH_FIRMWARE_BIN" \
  --flash-mb 16 --psram-mb 4 --max-seconds 6 --script tough-actions.txt \
  --console uart0 --tft-png tough-frame.png --no-dump
```

`TOUGH_FIRMWARE_BIN` 指向固定版本的原样 BIN。浏览器检验使用加载了相同 ROM 的本地工作台，执行 `playwright_cli.sh run-code --filename tools/probe-m5-tough-browser.js`。检验以客体时间等待菜单，验证 A 键、触摸像素和 2D / 3D 切换。 [浏览器运行收据](evidence/tough/runtime-browser.json)

已观察到的运行范围限于上述固件和功能。实机、整机断电和触控芯片升级仍待验证。
