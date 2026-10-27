# AtomS3-Lite C124：按键与 RGB 灯

C124 现在有独立的 `atoms3-lite` 板级模型：GPIO41 低电平用户键和 GPIO35 的单颗 WS2812C-2020，通过实际 RMT 输出驱动灯色。8 MiB Flash、无 PSRAM、无屏幕。没有映射成 AtomS3 或 AtomS3R。

官方原理图的 USER_BUT 接 G41，10 kΩ 上拉，按下接地；SK_DIN 接 G35，灯供电 3.3 V。官方 M5AtomS3 的 LedDisplay 定义单颗 NEOPIXEL。资料的精确提交、实际抓取时间、最终地址、媒体类型和 SHA-256 见 [sources.json](evidence/atoms3-lite-runtime/sources.json)。共享原理图含 LCD 选装电路，不能据此给 C124 加屏幕或 IMU。

前盖按键用厂家 STL 内部的保守命中区域，避开外围壳体和 USB 开口。真实客体灯色显示在 CAD 已有的 3.0 × 0.6 mm 狭槽后方；2.8 × 0.4 mm 的光学显示面是估算代理，不表示已知内部 LED 封装位置。白壳、金色及紫透外壳均不得改变灯的独立材质。原有照片轮廓失败、PCB UNKNOWN 和连接器估算继续保留。

## 官方固件

- **UIFlow2 v2.5.3**：固定官方版本经 ROM/WASM 启动，RMT 输出蓝灯；仍为 `trial`。启动日志有内部 I²C 未初始化提示，没有证明联网、云端部署或全部 MicroPython API 可用。
- **官方出厂 EasyLoader**：不执行 Windows EXE。实际解析 `.NET .resources` 的 `firmware` ByteArray；同一资源中的 `StartAddress` 为 `0x0`。原始镜像在 ROM/native 与 ROM/WASM 启动，GPIO41 按下后真实程序打印 `BTN Pressed` 计数。红外发送日志不证明接收端或物理红外已实现。
- 出厂包仍是第一项和默认恢复入口。当前网页不解包 EXE，未找到独立公开 BIN，因此保持 `reference`，不将提取后的固件镜像到本站。UIFlow2 可直接远程尝试运行。

固定出厂包的哈希、精确切片范围及内嵌镜像哈希见验证收据。可从用户提供的该版本 EXE 中提取：先校验整个文件 SHA-256，再读取 `bytes[offset:offset+length]`，再次校验内嵌哈希；任何哈希不同均拒绝沿用这个偏移。它是此固定包的可复现提取说明，不是任意 EXE 解析器。

## 已知边界与负面结果

旧 WIP 使用 `Ws2812Chain::with_gpio_clock`，却没有 EX218 要求的 `uses_gpio_waveform` 调度选择。真实 WASM 测试不能得到软件 GPIO 初始颜色，普通解释器也未修复；原生默认 quantum 偶然得到颜色不能代表浏览器成立。因此最终实现仅接收 RMT 帧，不对普通 GPIO 波形生成灯色。

[EX218](../evidence/gpio-bitbang-waveform/README.md) 已实现可选每指令调度，会拒绝浏览器外部 JIT 块。本轮不重新实现或偷偷开启这项代价很大的功能。共享 CPU、总线、调度和每 tick 路径均未修改；没有新增通用 Board 方法，不作速度提升或无回退声明。IR、模拟电源/复位、无线联网和外部 Grove 配件电气路由仍未完成。

## 复现

```sh
# ESP-IDF v5.5.1；只生成本地测试输入，不发布 binary。
idf.py -C tools/fixtures/atoms3-lite-firmware -B output/atoms3-lite-build build
tools/wasm-build.sh
node tools/probe-atoms3-lite.mjs --build output/atoms3-lite-build

python tools/rebuild-product-model.py plan m5stack-c124
python tools/rebuild-product-model.py build m5stack-c124 --scratch output/c124-rebuild
```

真实 fixture 验证 RMT 红灯、按键切绿/蓝/灭/红、按下/释放的客体 GPIO 读数、平台重启释放输入和重新输出红灯；没有虚构 framebuffer。浏览器鼠标、键盘与 mini 的结果见 [验证收据](evidence/atoms3-lite-runtime/verification.json)。本轮未连接实物或刷机，协议正确性与光学/电气校准分开记录。
