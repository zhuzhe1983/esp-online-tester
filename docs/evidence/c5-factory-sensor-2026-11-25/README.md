# 原厂 Sensor：保留真实失败，保持模型合同

原厂 Sensor 页面能通过实际GPIO进入并更新布局，但其温湿度和IMU数值未通过验收。固定原FactoryOnly字节、虚拟25°C/50%RH环境、SHTC3最大转换时间和QMI大端语义全部保留。本记录不把页面入口、Photo成功或模拟器单元测试当作正确传感器读数。

| 实际结果 | 已核对原因 |
| --- | --- |
| SHTC3 −45.00°C / 0.00%RH | 原驱动在转换仍忙时读失败，忽略错误并换算零缓冲 |
| QMI Z约0.04m/s²、温度约0.10°C | 原驱动选择BE输出，却按LE组合数据 |

## SHTC3：2 ticks 不保证20ms

固定原源码和原BIN均使用正常、无拉伸命令7866。原BIN实际传给vTaskDelay的是2，CONFIG_FREERTOS_HZ=100；等待目标按当前tick加2，不是从调用时刻精确等20ms。两个真实事务从接受测量末字节到READ地址分别相隔2695536和2699184个240MHz名义counter周期，即11.2314ms、11.2466ms。模型按[Sensirion SHTC3 V4 Table5](https://sensirion.com/media/documents/643F9C8E/63A5A436/Datasheet_SHTC3.pdf)选择正常转换最大12.1ms（典型10.8ms），所以此时仍忙、地址NACK。这个固定最大值是明确虚拟策略，不证明每颗实体都需12.1ms。

wake与7866均ACK且TX返回0；RX实际返回hex0x103（ESP_ERR_INVALID_STATE，非timeout0x107），guest408559f8的六字节仍为零。原get_th忽略wake/measure/RX/sleep结果、不验CRC，最终返回ESP_OK；原公式把零转换为−45/0。随后sleep同样遇忙NACK。200ms参数是SDK事务等待上限，不能当作busy NACK自动重试。原先证据中“延迟20ms”描述的是源码请求；本次记录了原BIN实际2ticks及真实间隔，不倒填为20ms已观察。

## QMI：配置和解析端序不一致

实际全部ACK：CTRL1=60、CTRL2最终24（8g/4096LSB/g）、CTRL3=43、CTRL7=03。两次35h开始12B均为`00 00 00 00 10 00 00 00 00 00 00 00`，33h温度为`19 00`。[QMI8658C RevA](https://www.qstcorp.com/upload/pdf/202210/13-52-27%20QMI8658C%20Datasheet%20Rev%20A%20%281%29.pdf) CTRL1 bit5选择大端，bit6自动增址；共享模型遵守该位。

按已选择BE，Z=1000h=4096（+1g），温度1900h/256=25°C。原驱动却按LE取得16与25：16×9.807/4096=0.03830859375m/s²、25/256=0.09765625°C，恰好显示0.04和0.10。不能通过强制模型总是LE来把错误数值称作正确模拟。这里描述的是原驱动与确定性数字刺激，不是实体精度或校准。

## 来源、观测与复现

[source.json](source.json)记录固定厂家/IDF553/官方规格及完整数学合同；[abi.json](abi.json)独立定位原BIN入口、返回点、栈偏移及错误码；[native.json](native.json)记录实际两轮命令、ACK、FIFO、guest缓冲、计数与恢复证明。原日志34729B/SHA c287dd715c3b909dc0d5ba3439a9b6dbc9d08e21fae4d3e64f820a5f9069d758、QMI摘要5050B/SHA a0d5e209b536109d06772c26b695fa80611acd11acf3578e4fa48a9b4cc8dd89保留于私有ignored存储，不内嵌UART或原始帧。

透明诊断补丁只转发原I2cDevice调用并打印其返回值/数据，native observer只读取既有内部SRAM。没有改guest PC、环境、ready、回读字节或固件；没有host调用应用函数。补丁和最终harness原字节随本目录保留，SHA见[diagnostic-source.json](diagnostic-source.json)。它们没有应用到生产模型。

在本提交的独立、干净临时checkout中复现，所有路径由调用者给定：

```sh
git apply docs/evidence/c5-factory-sensor-2026-11-25/diagnostic-i2c.patch
cp docs/evidence/c5-factory-sensor-2026-11-25/factory_sensor_boundary.rs esp32c5/examples/factory_sensor_boundary.rs
cargo build --release -p esp32c5 --example factory_sensor_boundary
target/release/examples/factory_sensor_boundary \
  "$ROM_ELF" "$ORIGINAL_FACTORY_BIN" "$CARD_IMG" "$FRESH_OUTPUT_DIR"
```

原输入为固定cf10 FactoryOnly/49693 ROM/F29卡，完整SHA见相邻Photo记录。运行后保留输出，再只还原本诊断补丁；不要将临时instrumentation当生产改动。原记录区分旧intermediate `diagnostic-final-source.json`的4573B harness和实际ready的4694B版本；后者含WAKE_TX_RETURN点。所有旧hash保留，未以新metadata替换旧原件。

结论是保留最大转换期限、真实NACK和BE行为，将原厂Sensor数值列为已知限制。修改原厂等待/重试、错误/CRC或端序代码应当作为另一份明确修改过的固件验收，不能回称本次未改FactoryOnly数值通过。
