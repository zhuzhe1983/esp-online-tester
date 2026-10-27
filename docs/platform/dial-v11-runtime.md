# Dial v1.1 K130-V11 板级模拟

K130-V11 现在使用独立身份 `dial-v11`，共享已核对引脚的 Dial 屏幕、触摸、按压与编码器模型。目录标为 `partial`。已用官方 UIFlow2 v2.5.3 和官方 EasyLoader 内的原始出厂镜像运行真实 ROM / WASM；没有实机测试，不代表整机功能全部兼容。

## 版次与实现范围

[官方 v1.1 产品页](https://docs.m5stack.com/en/core/Dial_v1.1)将核心板改为 Stamp-S3A，仍为 ESP32-S3FN8、8 MiB Flash，无 PSRAM。GC9A01 的 DC4 / MOSI5 / SCLK6 / CS7 / RST8、FT3267 的 SDA11 / SCL12 / IRQ14，以及编码器 A41 / B40 与原版相同；前部按压连接 GPIO42，低有效。16 个定位点、64 脉冲/圈是 v1.1 官方标注，正向四相和屏幕拖动方向仍是未经实机校准的平台约定。

官方页面仍链接同一份 [Sch_M5Dial.pdf](https://m5stack-doc.oss-cn-shenzhen.aliyuncs.com/499/Sch_M5Dial.pdf)，其中核心板标注 Stamp-S3；不能把它说成新绘制的 S3A 原理图。v1.1 页面另行明确 GPIO38 控制 RGB 供电。本阶段不实现该 RGB、BM8563 RTC、WS1850S RFID、PWM 背光、蜂鸣器音频、电源锁存/睡眠或射频。RTC/RFID 不伪造 ACK，按 USB 供电状态运行。完整抓取地址、时间、字节数、hash 和源码版本见 [来源收据](evidence/dial-v11/sources.json)，相应引脚事实也写入产品知识 JSON。

## 两份官方固件与出厂入口

- **UIFlow2 v2.5.3**：M5Burner 新 API 明确标为 `OFFICIAL`，目标是 Dial。v1.1 官方页面指向同一 Dial UIFlow2 教程。新旧下载入口的 8,388,608 字节完全相同，SHA-256 为 `6a0187dad13c491129e9228ca36b9e04adb46ebf9d8d51fca8eca5f9b7332a53`。它作为可运行的官方替代固件加入目录及 watchlist，不冒充出厂固件；后续新版本重置为 trial。当前离线开发页、旋钮切换页和触摸选择密码输入项已验证，联网仍不可据此认定通过。
- **官方出厂 UserDemo**：官方产品页提供 EasyLoader EXE。只读取 .NET 资源，不运行 EXE；`MiniBurner.Resource1.resources` 的 `firmware` 是 3,170,128 字节的完整镜像，`StartAddress=0x0`，SHA-256 为 `a61edfd7c2f6ece2e1570968b931b809545372b8bdac20864f03093a57b1276e`。原字节启动后，GPIO42 进入/退出 LCD TEST、旋钮切换图样、FT3267 在客体坐标 (100,120) 画出蓝点均已通过。
- M5Burner 名为 M5Dial-UserDemo 的 0.1 由 F4HWN 社区上传，其 8 MiB 镜像与官方 EXE 资源不同，不能当作同一官方镜像。该差异已保留。

当前网页加载器不读取 EXE 资源容器，出厂条目仍为 `reference` 并继续排第一，产品 `defaultFirmwareId` 保持出厂恢复入口；没有把 EXE URL 填成 BIN，也没有把提取的厂家镜像发布到本站。浏览器可直接运行的是 UIFlow2；用户手工提取的完整出厂镜像可按本地文件加载。未来网页支持经过校验的 EasyLoader 资源提取后，再将同一出厂条目改为可直载。

## 复现真实执行

从 [官方恢复入口](https://m5stack.oss-cn-shenzhen.aliyuncs.com/resource/docs/products/core/M5Dial/M5dial%20Factory%20Demo.exe)取得 EXE，放入忽略的 `output/`。下面的提取器不会执行 Windows 程序，只接受显式双 hash，拒绝已有输出和网站资产目录。

```sh
uv run --with dnfile==0.18.0 python tools/extract-dial-easyloader.py \
  --input output/factory-easyloader.exe \
  --container-sha256 c65ce95728faf0d3765d2136e15e391ec7306bca2db65579a7a972715b47fc49 \
  --image-sha256 a61edfd7c2f6ece2e1570968b931b809545372b8bdac20864f03093a57b1276e \
  --output output/dial-factory.bin
tools/wasm-build.sh
node tools/probe-dial-official.mjs --mode factory --file output/dial-factory.bin \
  --sha256 a61edfd7c2f6ece2e1570968b931b809545372b8bdac20864f03093a57b1276e
node tools/probe-dial-official.mjs --mode uiflow \
  --url https://m5burner-cdn.m5stack.com/firmware/b9b74d37d8fefa8ec70a77234011fe42.bin \
  --sha256 6a0187dad13c491129e9228ca36b9e04adb46ebf9d8d51fca8eca5f9b7332a53
node tools/probe-dial.mjs --build output/dial-build --product m5stack-k130-v11
```

内部源码 fixture 的构建见 [README](../../tools/fixtures/dial-firmware/README.md)。同一个源码在 `dial` 和 `dial-v11` 两个真实板身份分别执行，检查每个屏幕像素、GPIO42、触控坐标/IRQ、四相 GPIO 中断、录制及回放。不会把 fixture 加进公开固件库。

## 模型与边界

厂家 CAD 的 `front-ring` 绑定前部按压，`rotating-ring` 绑定编码器，圆屏显示客体 framebuffer。它们分别使用绿色、蓝色、红色悬停提示。几何、贴图、装配变换和 UNKNOWN PCB 位置不改变；每个 Grove 插口仍保留尚未核实方位的限制，不凭猜测显示连接线。

公开重建工具核对固定原始 CAD 与照片，保留原有尺寸和照片对照偏差，不将本次输入适配写成 1:1 还原验收。截图和完整日志留在忽略的 `output/`，提交 compact JSON、输入 hash 和复现命令。双 Clippy、黄金输出、平台回归与浏览器结果见本目录证据收据；不修改通用 SoC bus、中断路由或 per-tick 路径，没有新增 CPU 性能主张。

整合到 `106164f8` 后重跑的官方固件、完整回归和真实浏览器结果见 [整合收据](evidence/controller-integration/checks.json)。Dial UIFlow 下拉按钮会按住重复切换；验证唯一选项高亮，不承诺主机短按的固定重复次数。
