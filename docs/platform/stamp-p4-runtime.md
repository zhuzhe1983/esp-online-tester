# Stamp-P4 S013 裸模块运行子集

`stamp-p4` 使用独立 ESP32-P4 裸模块配置：16 MiB Flash、32 MiB PSRAM，没有继承 Waveshare 载板的屏幕、触摸、音频器件或 C6。S013 的身份和连接器依据[官方产品页](https://docs.m5stack.com/en/core/Stamp-P4)与[原理图](https://m5stack-doc.oss-cn-shenzhen.aliyuncs.com/1218/SCH_Stamp-P4_2026_03_16_17_23_06.pdf)。外露 GPIO、MIPI、SDIO 和 USB 连接器的存在不表示相应外设已接入；本轮没有实机验证。

平台选择 rev < 3 的虚拟 P4，当前 eFuse 报告 v1.0。这与固定版本 [UIFlow sdkconfig.p4](https://github.com/m5stack/uiflow-micropython/blob/f20d5a00e8c9106aae1d944f3d6f1128839f59cb/m5stack/boards/sdkconfig.p4) 的配置一致，不推断用户硬件的硅片版本。前端按 P4 芯片 ID 18 检查镜像，完整 Flash 的 bootloader 位于 `0x2000`；要求 rev >= 3 的镜像在此产品明确拒绝，不静默换用另一 ROM。其他 P4 CLI 配置仍沿用已有的镜像版本选择机制。

实际 WASM worker 执行内部 ESP-IDF 6.0.2 测试固件，完成 32 MiB PSRAM 初始化、跨 2 MiB 每页及末字的读写检查。UART0 输入指定种子，客体向 PSRAM 写入 4096 个 xorshift32 值，再读回求和；主机独立计算预期值。两组新输入、错误命令、暂停、软件重启和平台重启通过，见 [wasm.json](evidence/stamp-p4-runtime/wasm.json)。串口是明确的逻辑主机注入，不证明 UART GPIO 路由、USB 信号或 BTB 插合。裸模块没有固件 framebuffer 或映射的用户按键。

同一源码的 ESP-IDF 5.5.1 构建未能进入应用。其 bootloader 启用 ADC 熵源后等待 `ADC_INT_RAW` 的 SAR1_DONE bit 31；当前 P4 后端没有 ADC 转换完成模型。WASM 运行 19.84 秒客体时间仍停在这里，native 指令采样确认 `bootloader_random_enable+0x100/+0x102` 占 99.88%。IDF 6.0.2 的启动源码不再等待该标志，因此可以继续正常启动。这是保留的兼容边界，没有通过改写旧固件、伪造 ADC 完成或注入应用输出来掩盖：[idf551-negative.json](evidence/stamp-p4-runtime/idf551-negative.json)。

PSRAM 验证采用现有 P4 的数字 HEX/x16 存储控制器模型；官方产品页称“32MB Octal”。容量及应用读写经过验证，物理总线宽度、信号时序和模块硅片配置没有实测，不将这项表述差异写成硬件认证。模拟电源、低功耗、真实 RNG 熵、外接 MIPI/SDIO/USB 设备及网络功能仍未验收。

截至 2026-11-04，没有找到证明本型号出厂用途的完整镜像，未发现精确小智目标。官方 UIFlow2 v2.5.4 保留为 alternative/trial，固定 M5Burner 下载入口本次返回 HTTP 400 `BURN_CAPTCHA_INVALID`；没有取得镜像，也没有运行兼容性结论。公开 UIFlow 板配置引用外接 ESP-Hosted C6，SDIO CMD44、CLK43、D0–3 为 45–48、复位42。裸 S013 不自动接入外部 A172/C6。来源、固定版本及下载回执见 [sources.json](evidence/stamp-p4-runtime/sources.json)。

激活 ESP-IDF 6.0.2 的 Python 环境并 source `export.sh` 后，可重建内部验证固件；它不进入产品固件目录，二进制不提交或公开托管：

```sh
idf.py -C tools/fixtures/stamp-p4-firmware -B /tmp/stamp-p4-build \
  -D SDKCONFIG=/tmp/stamp-p4-sdkconfig reconfigure
cmake --build /tmp/stamp-p4-build --parallel 2
tools/wasm-build.sh
tools/fetch-rom-elfs.sh web/wasm/fw
node tools/probe-stamp-p4.mjs --build /tmp/stamp-p4-build
```

切换到 ESP-IDF 5.5.1 并使用独立构建目录可重现上述失败；该旧 SDK 不识别 `ESP32P4_SELECTS_REV_LESS_V3`，其原有 P4 目标就是 rev < 3。完整文件 hash、版本、检查及浏览器结果保存在本目录的 evidence 中。没有宣称整机固件或全部 P4 外设已适配。
