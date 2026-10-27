# DinMeter K134：屏幕与按压编码器

原版 DinMeter K134 新增独立 `dinmeter` 板型。固定官方 UserDemo v0.1 在原生内核与浏览器 WASM 中启动，显示 240×135 主菜单；3D 旋钮中心按压进入彩条测试，外围拖动切换菜单；编码器测试页从 0 变为 2。仅这些操作经过验证，不代表 RTC、Wi-Fi、供电或所有菜单功能兼容。K134-V11 继续保持未适配。

## 来源与实现

[来源收据](evidence/dinmeter/sources.json) 保留地址、抓取时间、大小和 SHA-256。引脚取自官方 `DIN_Meter_v1.0.pdf`，屏幕窗口和旋转取自 `m5stack/M5DinMeter-UserDemo` 的 `d2fdfe44e9baf810f82335faa6f5962e9fc59a44`；同时核对 M5DinMeter `8ccbad7297beb82a140f812e5606e060d8a5e0dc`、M5GFX `65c816a38386062d2c3dc47d41237a9083acbc75` 和 M5Unified `368ad60807161625aa65baad37a4a3af2a7eccea`。

| 器件 | 实现范围 |
| --- | --- |
| ST7789V2 | MOSI5/SCLK6/CS7/DC4/RESET8；复用 DCS GRAM；SLPOUT/DISPON/DISPOFF/SLPIN 与复位影响实际输出 |
| 屏幕方向 | 135×240 玻璃横放为 240×135；固定物理裁切 `(col,row)=(52+y,279−x)`；官方 rotation3 与新版 M5GFX offset_rotation2/rotation1 对应同一方向 |
| 旋钮 | A41/B40 四相 GPIO 输入；正数是 A 先下降。官方 `attachHalfQuad(40,41)` 每完整步 PCNT 增加 2；软件全四相解码为 −4 |
| 按压 | GPIO42，低有效；3D 旋钮中心或键盘 A；GPIO0 为 BOOT 输入高电平 |
| RTC / 其余功能 | RTC 的 I²C11/12 未装设备，明确 NACK。PWM 背光、电池 ADC、蜂鸣器、供电锁存/睡眠、Grove 配件、无线通信未实现或未验收 |

官方出厂源工程使用 espressif32@6.3.1（Arduino 2.x / IDF4.4 系列）及 LovyanGFX1.1.12；内部定向固件使用 IDF5.5.1。没有修改 SoC 总线、`device_set!`、中断路由或每 tick 路径，也没有修改 PCNT 寄存器模型。窗口裁切是玻璃方向，不随客体 MADCTL 重设，以避免重复旋转。

## 模型与交互

沿用厂家原始 CAD 的同一个旋钮组件，中心前表面半径 4.5 mm 区域按压、外环拖动旋转。悬停分别显示绿色和蓝色。这个命中区和每圈 16 格是界面约定，未做机械边界、顺时针方向或实物档数校准。

`model-plan.json` 与 `model.json` 同时保留这些语义；[公开重建结果](evidence/dinmeter/model-rebuild.json) 的 6 个几何/源文件/参考图产物逐字节一致，[语义修订收据](evidence/dinmeter/model-semantic-revision.json) 保留前后模型 hash。没有改动三角面、贴图或旧照片对照结论；[原有核验](evidence/m5stack-k134/verification.json) 中支架是否计入 53×30×32 mm 总包络、后视照片的不同装配状态等偏差仍然存在，内部 PCB 仍为 UNKNOWN。

## 复现与证据

官方镜像地址和 SHA-256 保存在 `m5stack-k134-factory-reference`，保持第一项默认固件。二进制只在下载测试的内存或用户浏览器缓存中使用，不添加到仓库/Pages。

```sh
# 激活 ESP-IDF 5.5.1 后；只构建内部定向测试，不是官方固件
idf.py -C tools/fixtures/dinmeter-firmware -B "$PWD/output/dinmeter-build" -D SDKCONFIG="$PWD/output/dinmeter-sdkconfig" build
tools/wasm-build.sh
node tools/probe-dinmeter.mjs --build output/dinmeter-build
# 使用目录内原版 K134 固定官方镜像地址和 SHA-256
node tools/probe-dinmeter-factory.mjs --url https://m5burner-cdn.m5stack.com/firmware/fc3c7b0f51aa57c5ec0b04b7f4ce8084.bin --sha256 b0d8d63fa9a8e30f712de37cf58c340328690f61c90f94ce7a32da3050589d40
uv run --with-requirements tools/requirements-model.txt python tools/rebuild-product-model.py build m5stack-k134 --scratch output/dinmeter-rebuild
```

[内部 WASM 收据](evidence/dinmeter/fixture-wasm.json) 逐点校验 240×135 四象限像素、GPIO42、GPIO 中断解码与 PCNT 两条路径、输入边界和录制回放。回放完成且最终 framebuffer 相同，不代表周期精确输入或全部执行状态逐位相同。

[原生运行收据](evidence/dinmeter/factory-native.json) 与 [浏览器收据](evidence/dinmeter/factory-browser.json) 列出固定镜像、ROM/WASM hash、操作和截图；固件的 RTC NACK 与 Bat 0.0V 保留为已知缺项。未连接实物，未测试 Web Serial 刷机。回归清单见 [checks.json](evidence/dinmeter/checks.json)，没有更新任何 golden。

[官方 WASM 可复现探针](evidence/dinmeter/factory-wasm.json) 通过四步选项导航、进入编码器测试、正向改变/反向逐像素恢复及按压退出；分享的 mini 页面无需手动启动即可运行。首次浏览器等待脚本过早调用 pause，改为明确轮询 running 与客体时间后通过；没有为此修改产品代码。截图只留在忽略的 output/，提交收据中的 hash、大小和操作步骤，不上传固件或原始捕获。

整合到 `106164f8` 后重跑的官方固件、完整回归和真实浏览器结果见 [整合收据](evidence/controller-integration/checks.json)。Dial UIFlow 下拉按钮会按住重复切换；验证唯一选项高亮，不承诺主机短按的固定重复次数。
