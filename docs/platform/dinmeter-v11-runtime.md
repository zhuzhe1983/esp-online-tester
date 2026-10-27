# DinMeter v1.1：ST7789P3 与旋钮输入

K134-V11 使用独立 `dinmeter-v11` 板型。固定官方 UserDemo v0.1 已在原生内核和实际生产 WASM 中从 ROM 启动：显示 240×135 主菜单，四步进入编码器测试，正转改变计数，反转逐像素恢复原画面，按压退出。原版 K134 继续使用 `dinmeter`，不以相同外壳推断芯片外围完全一致。

## 来源与边界

[来源收据](evidence/dinmeter-v11/sources.json) 固定官方 v1.1 页面、原理图、ST7789P3 datasheet、M5DinMeter/M5GFX 源码和固件的地址、抓取时间、大小与 SHA-256。Stamp-S3A 使用 S3FN8 / 8 MB Flash；运行配置不接入 PSRAM，未将未标明的实物容量改写成测量结果。

| 项目 | 本次数字模型 |
| --- | --- |
| ST7789P3 | CS7/SCK6/DC4/MOSI5/RESET8，复用有界 ST7789 DCS/GRAM；P3 的 RDDID/RDID1/2/3 为 `81 81 B3`，复位后保持 P3 身份 |
| 屏幕 | 135×240 玻璃横置；物理裁切 `(col,row)=(52+y,279-x)`，240×135 输出；开关、睡眠、复位保留原 DCS 语义 |
| 旋钮 | A41/B40 四相 GPIO；正数步进 A 先下降，官方 half-quadrature PCNT 每完整步增加 2 |
| 按压 | 低有效 GPIO42，语义 ID `encoder-press`；键盘 A 和 CAD 旋钮中心走同一路径 |
| 未实现 | RTC I²C11/12 明确 NACK；PWM 背光、电池 ADC、蜂鸣器、供电锁存/深睡、Grove 路由、Stamp-S3A GPIO38 RGB 电源与无线功能 |

P3 识别值来自 Sitronix ST7789P3 Version 0.1（March 2022）第 126 页；V2 的 `85 85 52` 保持不变。固定 M5GFX `65c816a3` 接受 0x81 或 0x85，但不声称分发的旧固件使用这一份源码。官方出厂工程为 Arduino 2.x / IDF4.4 系列；没有改动 SoC 寄存器、时钟、中断或每 tick 路径。固件未打补丁，RTC 错误和画面上的 Bat 0.0V 没有隐藏。

## 模型与固件目录

沿用该 SKU 厂家原始 CAD、刚体装配和照片；旋钮中心估算半径 4.5 mm 为按压，外环为拖转，悬停分别为绿色和蓝色。每圈 16 个操作步进是界面约定，未校准机械档位、方向或实物时序。Grove 朝向/线序未验证，因此保持禁止显示已插接线缆；内部 PCB 仍标注 UNKNOWN。

七项源文件、照片和几何资产从已提交来源重建一致；模型 JSON 初次仅键顺序不同，语义完全一致，随后采用重建输出。原有正/背面轮廓 IoU 0.99231/0.99019 保留；深度 29.5500 mm 对标 32 mm 未通过，侧面仅定性检查。缺少完整后部 PCB、电池座、螺丝和安装夹，不能写成整机 1:1 通过。

官方 v0.1 仍排在第一并作为默认。v1.1 官方页面明确链接同一 DinMeter UIFlow2 教程，已把该 SKU 加入固定 OFFICIAL DINMETER 项目与 watchlist；UIFlow2 v2.5.3 仍为未验证 trial，不继承出厂固件验收。官方页面及已审计的官方目标映射中未找到精确 DinMeter 小智条目，未用通用 S3 构建代替。

## 复现

```sh
tools/wasm-build.sh
node tools/probe-dinmeter-factory.mjs --product m5stack-k134-v11 --url https://m5burner-cdn.m5stack.com/firmware/fc3c7b0f51aa57c5ec0b04b7f4ce8084.bin --sha256 b0d8d63fa9a8e30f712de37cf58c340328690f61c90f94ce7a32da3050589d40
uv run --python 3.11 --with-requirements tools/requirements-model.txt python tools/rebuild-product-model.py build m5stack-k134-v11 --scratch output/dinmeter-v11-rebuild
```

[WASM 收据](evidence/dinmeter-v11/factory-wasm.json) 对真实 Worker 执行的官方镜像检查菜单、PCNT 正反转和 GPIO42 退出。固定固件为 2,090,144 字节，SHA-256 `b0d8d63fa9a8e30f712de37cf58c340328690f61c90f94ce7a32da3050589d40`。固件仅保留在本地忽略的测试输入或用户浏览器缓存，不发布到 Pages。

完整 CI 检查通过 1,434 个 Rust 测试（132 组）、双 Clippy、11 个 WASM 场景、100 个平台测试与 schema、真实录制回归、111,126 个 JIT 差分用例及 24 个 GPIO 波形用例，未更新 golden。详细命令见 [checks.json](evidence/dinmeter-v11/checks.json)。[浏览器收据](evidence/dinmeter-v11/factory-browser.json) 验证旋钮中心进入彩条测试、键盘 A 返回、外圈拖动切至亮度菜单；五个操作录制回放后的最终帧相同。分享 mini 无需手动启动即显示官方菜单。回放最后累计延迟约 0.1325 客体秒，不是逐周期一致；未使用实物或执行刷机。

[CPU 对照](evidence/dinmeter-v11/cpu.json) 使用固定 main `84e67571`，S3/C3/C6 三组 hello 各五对交替运行；中位 user 时间差为 0%、+1.717%、−1.266%，逐对 UART、指令与周期完全一致。计时期间关闭本任务浏览器，所有构建与其他客体均已退出。跨版本对照只用于回归观察，不能归因为本次优化；没有修改 S3 核心，不需要 Pocket 用例。复现：`python docs/platform/evidence/dinmeter-v11/cpu.py --baseline-bin <固定参考二进制目录> --output <收据.json>`。
