# AtomS3R C126 板级模拟

独立的 `atoms3r` 板级模型支持 **2026-05-14 起的 ST7735 批次**，目录状态为 `partial`。
它执行 ESP32-S3 客体指令，通过真实 SPI/I2C/GPIO 通道产生画面、背光和输入结果。
没有遍历官方固件库；目录固件可尝试运行，不代表整机兼容认证。

## 依据与范围

[官方产品页](https://docs.m5stack.com/en/core/AtomS3R) 记录了 GC9107 → ST7735 的批次变更。
[官方 v0.4.1 原理图](https://m5stack-doc.oss-cn-shenzhen.aliyuncs.com/680/Sch_M5_AtomS3R_v0.4.1.pdf)、
[M5GFX 固定版本](https://github.com/m5stack/M5GFX/blob/65c816a38386062d2c3dc47d41237a9083acbc75/src/M5GFX.cpp)
与 [M5Unified 固定版本](https://github.com/m5stack/M5Unified/blob/368ad60807161625aa65baad37a4a3af2a7eccea/src/M5Unified.inl)
用于核对接线、屏幕窗口、背光与按键。所有实际 GET 的最终地址、抓取时间、媒体类型、
字节数和 SHA-256 见 [sources.json](evidence/atoms3r-runtime/sources.json)。

| 部分 | 已实现 | 尚未实现或验证 |
| --- | --- | --- |
| ESP32-S3-PICO-1-N8R8 | 8 MiB Flash、8 MiB Octal PSRAM；创建机器时设 PICO package-version=1，重启保留 eFuse | 不代表完整 Wi-Fi/BLE/USB 支持；未实机验证 |
| ST7735 | MOSI21、SCLK15、CS14、DC42、RST48；ID、RGB565、窗口、MADCTL、睡眠/开关/复位；128×128 framebuffer | 旧 GC9107 批次、GRAM读取、其他像素格式、模拟 gamma/颜色反转尚未实现 |
| LP5562 | 地址0x30，SDA45/SCL0；寄存器读写/复位、芯片使能、内部时钟、W通道直控PWM、零电流/引擎映射关断；背光变化更新真实framebuffer | RGB可见输出、程序引擎、外部时钟、对数调光、启动延时、电流与亮度标定未实现；不能把存储寄存器值当作功能已执行 |
| 按钮 | GPIO41低有效，`button`，默认键盘A；3D屏幕与2D输入共用该GPIO | 非触摸屏；机械行程/抖动与真实复位键下载模式未验证 |
| BMI270 | 复用现有0x68轮询寄存器模型，SDA45/SCL0；ID、复位、完整8192-byte配置传输门槛、量程/电源与主机输入读数 | 配置程序不执行、不校验内容；FIFO/中断/ODR、滤波、温度、自测未建模 |
| BMM150 | 保留正确硬件资料：经BMI270 AUX连接，主控I2C不伪造直连地址 | AUX桥与磁力计未实现，依赖九轴初始化的固件可能失败 |
| 姿态API | agent指定虚拟轴的加速度/角速度；暂停清gyro，保持accel | 未校准物理装配坐标；拒绝`source:'scene'`，3D拖动暂不驱动IMU |
| 3D | 保留厂家几何，屏幕平面标记按钮命中，外壳保持拖动 | 旧照片轮廓失败、估算屏幕/端子、UNKNOWN PCB保持原有边界；本轮未重新认证1:1外观 |

ST7735窗口依据 M5GFX 的 `memory_width=132 / memory_height=132`、offset `(2,1)` 和
`offset_rotation=2`，初始MADCTL `0xdc`。驱动写列2…129、行3…130；固定屏幕坐标从
GRAM `[128-y,129-x]` 取像素。ID字节 `83 76 00` 在小端 `readData(32)` 中满足
M5GFX的 `id & 0xffff == 0x7683`；这只代表所选批次的探测响应，不冒充全部面板批次。

LP5562依据 [TI SNVS820B](https://www.ti.com/lit/ds/symlink/lp5562.pdf) §§7.5–7.6。
白光PWM以RGB565分量的线性缩放近似视觉亮度，非光学或电气测量。引擎映射时输出关闭，
不会用直控PWM假装执行程序引擎。其他产品没有实例化新LP5562器件。

eFuse配置只在 CLI/WASM 创建所选板时执行，未修改bus、`device_set!`、中断路由或通用
per-tick路径。字段引用ESP-IDF 5.5.1的 `EFUSE_RD_MAC_SPI_SYS_3_REG=+0x50`、
`PKG_VERSION[23:21]`；4.4.8相同。没有新执行速度实验或性能优化声明。

## 复现与证据

[内部测试固件源码](../../tools/fixtures/atoms3r-firmware/README.md) 使用 ESP-IDF 5.5.1，
通过驱动发送屏幕像素、LP5562写入、BMI270初始化与读取、GPIO41轮询和PSRAM读写。
Bosch配置数组保留BSD-3-Clause许可及固定上游commit；生成binary只保留在忽略的
`output/`，不加入固件库、不发布到Pages。

```sh
# 已激活 ESP-IDF 5.5.1 环境
idf.py -C tools/fixtures/atoms3r-firmware -B "$PWD/output/atoms3r-build" -D SDKCONFIG="$PWD/output/atoms3r-sdkconfig" build
tools/wasm-build.sh
node tools/probe-atoms3r.mjs --build output/atoms3r-build
python tools/rebuild-product-model.py plan m5stack-c126
python tools/rebuild-product-model.py build m5stack-c126 --scratch output/atoms3r-public-rebuild
```

[fixture.json](evidence/atoms3r-runtime/fixture.json) 记录真实worker/WASM的客体结果：
ST7735读ID、128×128每个像素红绿一致；按键按下/释放；由客体按钮处理函数写入背光
0/128/255后每个像素符合亮度；BMI270初始化、8g/2000dps读数；暂停gyro清零而accel保留；
PICO封装1、Octal PSRAM识别8 MiB，ESP-IDF初始化内存测试与额外4 KiB分配读写成功。
[浏览器证据](evidence/atoms3r-runtime/browser.json) 另核对UI多文件加载、真实画面、3D/键盘/2D输入。

[模型重建记录](evidence/atoms3r-runtime/model-rebuild.json) 使用公开离线工具重建，完整元信息与
几何均字节一致。原始尺寸/照片指标与测量日期保留；`verification.json` 的元信息修订记录
同时保留旧hash和新hash，没有把旧照片检查说成新测量。

[checks.json](evidence/atoms3r-runtime/checks.json) 保存检查命令与结果。首轮完整Rust测试缺少
独立工作树中的CI测试固件（P4/C6 hosted资源）；从已获取的公共测试资源补齐后重跑。
首次ESP-IDF激活选到不满足依赖约束的Python3.14环境，改用现有5.5/Python3.11环境，未更改依赖。
本地Python HTTP默认连接队列在并发模块请求时出现连接重置；改为128请求队列后浏览器加载正常。

下一台按 [队列](device-runtime-queue.md) 处理 AtomS3-Lite C124，分别核对SK6812与红外。
