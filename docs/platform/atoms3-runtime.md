# AtomS3 C123 板级模拟

AtomS3 的 GC9107 版本已有独立 `atoms3` 板级模型。它执行真正的 ESP32-S3
指令，屏幕由客体 SPI 写入，按键走 GPIO41，IMU 由客体通过 I2C 读取。
目录状态为 `partial`；可以尝试匹配的固件，当前证据只覆盖下面列出的接口。
本轮没有遍历或认证官方/第三方固件库。

## 来源与边界

官方 [AtomS3 文档](https://docs.m5stack.com/en/core/AtomS3) 与
[M5GFX 固定版本](https://github.com/m5stack/M5GFX/blob/65c816a38386062d2c3dc47d41237a9083acbc75/src/M5GFX.cpp)
给出 SPI 引脚及面板窗口；[GC9107 驱动](https://github.com/m5stack/M5GFX/blob/65c816a38386062d2c3dc47d41237a9083acbc75/src/lgfx/v1/panel/Panel_GC9A01.hpp)
定义 128×160 GRAM。[M5Unified](https://github.com/m5stack/M5Unified/blob/368ad60807161625aa65baad37a4a3af2a7eccea/src/M5Unified.inl)
给出 GPIO41 按键；其 MPU6886 驱动及厂家提供的 DS-000193 rev1.1 给出传感器
寄存器、量程和复位值。每次读取的时间、固定源码地址、字节数与 SHA-256 保留在
[sources.json](evidence/atoms3-runtime/sources.json)，产品知识 JSON 引用相同来源。
来源原文的临时归档在忽略的 `output/atoms3-reference-sources/`；发布只保留引用与校验信息。

| 接口 | 已实现 | 当前缺项 |
| --- | --- | --- |
| ESP32-S3FN8 | 8 MiB Flash、无 PSRAM；沿用 S3 CPU/ROM/Flash 执行 | 不等于全部 Wi-Fi/BLE/USB 行为支持 |
| GC9107 | MOSI21、SCLK17、CS15、DC33、RST34；ID `00 91 07`，RGB565、地址窗口、MADCTL、睡眠/开关/复位；取 GRAM y32…159 输出 128×128 | 背光 GPIO16 PWM 亮度、模拟 gamma/电压/颜色反转、其他像素格式和 ST7735 批次未实现 |
| Button A | GPIO41 低有效，默认键盘 A；屏幕和屏幕盖板按压命中同一个按钮 | 机械行程/抖动、真实复位键长按下载模式未模拟 |
| MPU6886 | 地址0x68，SDA38/SCL39；硬件 I2C0/1 与软件 I2C 共享寄存器；ID、复位/睡眠/轴禁用、量程、数据就绪、连贯大端读数 | FIFO/中断引脚/ODR/滤波/偏置补偿/自测/真实温度未实现；FIFO 返回空，不凭空生成样本 |
| IMU API | `setImu` 的虚拟轴姿态和角速度；暂停后角速度清零，保留加速度 | 未校准物理装配坐标，因此暂不自动跟随 3D 拖动；`source:'scene'` 明确拒绝 |
| 3D | 沿用厂家 CAD；真实 framebuffer 映射到已有屏幕承载面；屏幕盖板/屏幕 `btnA` 命中 | 既有尺寸/照片与 UNKNOWN PCB 限制保持不变；本轮不是整机 1:1 认证 |

MPU6886 只在收到新主机样本后置数据就绪位；采样频率与中断时序没有按晶体时钟建模。
温度 ADC 固定为0（官方驱动换算25°C）。可读回的配置不代表相应滤波/FIFO功能已模拟。

## 可复现验证

模型交互定义同时保存在 `model-plan.json` 和导出器中。[公开重建证据](evidence/atoms3-runtime/model-rebuild.json) 使用 `tools/rebuild-product-model.py plan/build` 在 scratch 中验证几何字节不变、完整元信息语义一致；不依赖历史私有队列。旧轮廓测量结果未重跑或改写。

[内部 ESP-IDF 5.5.1 fixture](../../tools/fixtures/atoms3-firmware/README.md)
使用官方引脚，通过客体驱动发送 SPI、访问 I2C 和轮询 GPIO。源码在 `tools/fixtures/`，
生成的 binary 只在忽略的 `output/` 中；不会加入固件目录或发布到 GitHub Pages。

```sh
# ESP-IDF 5.5.1 环境中
idf.py -C tools/fixtures/atoms3-firmware -B "$PWD/output/atoms3-build" -D SDKCONFIG="$PWD/output/atoms3-sdkconfig" build
tools/wasm-build.sh
node tools/probe-atoms3.mjs --build output/atoms3-build
```

[fixture.json](evidence/atoms3-runtime/fixture.json) 保留输入 hash、逐像素 framebuffer hash
和真实客体串口证据。测试通过的五项：ROM/bootloader 启动与 IMU ID；128×128 每个像素
左红右绿；GPIO41 按下/释放均被客体读到；8g量程加速度与2000dps量程陀螺仪读数；
暂停后再次运行，客体读到 gyro=0 且 accel 保持。

[浏览器记录](evidence/atoms3-runtime/browser.json) 另外覆盖工作台的本地多文件加载、
3D 屏幕点击、键盘 A 和 2D 屏幕点击：每次操作后均等待新的客体串口出现
`ATOMS3_BUTTON 0` → `ATOMS3_BUTTON 1`。屏幕与黑色屏幕盖板命中按钮，白色外壳
保留旋转拖动。截图展示内部测试固件的真实输出，不是官方出厂固件兼容性证据。

浏览器还确认：3D 屏幕悬停识别为按钮，鼠标及键盘 A 的按下/释放均改变客体 GPIO41
读数，而非只有前端按键动画。真实屏幕帧来自同一 worker，3D 与2D共用 framebuffer。

首轮 fixture 使用256-byte无DMA SPI事务，被 ESP-IDF 自身以 `ESP_ERR_INVALID_ARG`
拒绝并重启。已按其64-byte FIFO上限分段；这不是板级模拟通过记录。另一次 probe 在暂停
状态发送按钮，按 API 合同返回 `NOT_RUNNING`，改为先恢复运行。两项修正未给模拟器添加
绕过路径。

Rust 单元测试还覆盖错 SPI 路由不响应、复位期间不响应、屏幕旋转/窗口、显示关闭、
I2C 状态共享、量程/饱和、睡眠、burst一致性及 FIFO 保持为空。验证命令：

```sh
rustup update stable
cargo +stable test -p esp-periph mpu6886
cargo +stable test -p esp32s3 atoms3
cargo +stable clippy --workspace --all-targets -- -D warnings
cargo +stable clippy --release --target wasm32-unknown-unknown -p esp32sim-wasm --features jit-tests -- -D warnings
ESP32SIM_ROM_DIR="$PWD/web/wasm/fw" cargo +stable test --release --workspace -- --include-ignored --skip external_
uv run --with jsonschema python tools/validate-platform-catalog.py
node --test tools/platform-config-test.mjs tools/platform-runtime-test.mjs tools/platform-input-test.mjs tools/platform-view-test.mjs tools/platform-imu-test.mjs
```

未修改 SoC bus、寄存器地址、`device_set!`、中断路由或通用 per-tick 路径；新增器件只由
`atoms3` 板调用。未进行或声称 CPU 性能优化，因此没有新性能实验条目。

其余产品按 [逐台适配队列](device-runtime-queue.md) 继续处理；下一台 AtomS3R 必须先核对
自己的 SPI 引脚、I2C背光控制和 BMI270/BMM150，不能借用 C123 连接定义。
