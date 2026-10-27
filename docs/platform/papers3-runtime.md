# PaperS3 C139 适配检查点

固定官方 v0.5 出厂镜像已经下载，SHA-256 `d6733a0ca378f95335fa5fba4d4d992fb1dd97c17557b20e9aebfca08ba6d624` 与现有目录一致；仍未执行验收，不启用产品。来源见 [抓取收据](evidence/papers3/sources.json)。

官方 UserDemo 固定 `5e275ad4b70abb85f7193fda137844730e64c4db`，依赖 IDF 5.3.3、M5GFX 0.2.15（`c6f92dc03226cdc04d67c705a2020f62ad21ad01`）、M5Unified 0.2.10（`5580ff6923a868cc71d5b30c962186bde2c85b67`）。当前源码不是二进制版本对应的证明。

屏幕为 960×540 ET047TC1 并行面板，数据 G6/14/7/12/9/11/8/10，XCL16、XSTL13、XLE15、SPV17、CKV18；与 PaperDIY 有可复用的实际 LCD_CAM i80 数据通道，但不是同一板型。厂家概览把 G45 写成 PWR；原理图和固定 M5GFX 还区分 G46 的 boost power 与 G45 的 output enable，必须分别检查，不能套用 PaperDIY 的 M5PM1 供电。

GT911 位于 I²C SDA41/SCL42、IRQ48，原始范围 540×960，驱动配置 offset_rotation=1，屏幕 offset_rotation=3。现有 Waveshare GT911 是始终 ready 的轮询简化实现，本次需要保持既有黄金输出，同时补充单触点的扫描/ready/清除语义；不把屏幕 960×540 的原生行扫描维度直接当作触摸坐标。BMI270 0x68 与 BM8563 0x51 可分别调查，不用虚构 RTC 走时。TF 是独立 SPI G39/G38/G40/CS47。HC1.25 接口供电是 3.3V。

电源按键由 PMS150G 控制，不作为普通 ESP GPIO；G44 是关机脉冲控制线。公开 v1.0 原理图不能自动证明 v1.1/v1.2 所有改版。官方程序启动先显示 FactoryTest V0.5、黑白和灰阶测试，之后进入电源/SD/RTC/蜂鸣器/IMU/Wi-Fi 面板；验收需要看到真实画面并通过触摸改变固件输出。计划先支持有界数字黑白更新，不宣称校准 16 灰阶光学波形。

已查看 upstream 开放 PR，没有 GT911 或 ET047TC1 的重复实现；GP-SPI #312 / SPI1 #313 不涉及本次 LCD_CAM i80 路径。此分支以已交付 PaperDIY `c0c66a57` 为依赖，后续提交单独交付；主分支和用户运行中的 WASM 不修改。
