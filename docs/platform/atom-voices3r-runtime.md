# Atom VoiceS3R C126-ECHO 适配记录

本次型号为 ESP32-S3-PICO-1-N8R8、8 MB Flash / 8 MB Octal PSRAM 的 C126-ECHO（旧称 Atom EchoS3R），不是 classic C008-C。官方产品页和音频板原理图确认：ES8311 的 SDA45 / SCL0，CE 接地、地址 0x18；I2S MCLK11 / BCLK17 / WS3 / 芯片输出48 / 芯片输入4，NS4150B 使能18，用户按键41。没有可由 ESP32 驱动的 RGB 或屏幕；IR47、复位监督 MCU、录音和 USB 音频须单独适配。

来源：[官网](https://docs.m5stack.com/en/core/Atom_VoiceS3R)、[主板图](https://m5stack-doc.oss-cn-shenzhen.aliyuncs.com/1182/Sch_M5_AtomS3R_v0.4.1.pdf)、[音频板图](https://m5stack-doc.oss-cn-shenzhen.aliyuncs.com/1182/Sch_M5_AtomEchoS3R_Audio_v1.0_20250716.pdf)。实际抓取时间与 hash 见 [sources.json](evidence/atom-voices3r/sources.json)。厂家主板图部分为通用 AtomS3R 电路、相关器件标 DNP，不能因此给 Echo 增加屏幕、RGB 或 IMU。

## 出厂 UAC 恢复包的布局冲突

官网 [Easyloader](https://m5stack-doc.oss-cn-shenzhen.aliyuncs.com/1182/Atom_EchoS3R_Easy_Loader_for_win.exe) 是 Windows .NET 程序，本次只解析资源和 IL，没有执行它。`MiniBurner.Resource1.resources/firmware` 原样提取为 383,728 字节，SHA-256 `e4e4b272a7f33b022554b02e41edada89c284dc7030c2e4ff51bd14a11c7f664`。

文件前 0x1000 全为 FF；S3 bootloader / 分区表 / app 实际位于 0x1000 / 0x9000 / 0x11000，但表内 app 目标地址为 0x10000。应用描述是 `atom_echos3r_usb_uac` / `bdfdb73` / IDF v5.4.1。资源 `StartAddress` 标称 0x1000，按钮处理 IL 实际硬编码 `write_flash ... 0x000`，并原样写出 byte array。按这个实际命令放入模拟 Flash 后，原始 S3 ROM 输出 `invalid header: 0xffffffff`。

因此这份恢复包不作为本次可直接运行的 merged BIN，也不把自动裁掉前缀当成原始官方镜像验收；保留资料入口和负结果。未实机刷写，不能据此声称所有厂家硬件或其他版恢复包有同样问题。

## 当前实现与探测

独立板型草案接入 G41 与 ES8311 控制总线初始化子集，复用已有 pin-aware I2C 和共享寄存器容器，不声称模拟音频 ADC/DAC。S3 PICO package identity 沿用 AtomS3R 冷启动标识。native 原始镜像初探：

- 官方 UIFlow2 v2.5.3 原始完整 BIN（`3fdc3de97d399fa77f5404ac155051ca1309e7a14dccc9353270f339d80ce33c`）进入 MicroPython v1.27.0 REPL，自报 `M5STACK Atom EchoS3R with ESP32-S3-PICO-1`。保留两条 I2C port-not-initialized 日志，不据此推断完整音频成功。
- 官方小智 v2.2.2 原始完整 BIN（`7997414bec17383bf5b9c5e30cac65c8a17fd013e9f21cce5a3b6c15c2676e0f`）进入配网 AP / Web server 状态。日志不代表宿主浏览器已经能连接该 AP，网络传输与麦克风/音频仍未验收。
- 官方示例仓库固定树 `1bc38736e61a362080d1dffc4020b3a2a1dff6ae` 未找到 EchoS3R/UAC 项目。仍需为控制寄存器核对 codec 数据手册，并完成 WASM、真实输入与模型测试。

本记录是研究 checkpoint，未把目录标为已完成。尚无实体硬件验证，原始下载保留在忽略目录，不发布固件二进制。
