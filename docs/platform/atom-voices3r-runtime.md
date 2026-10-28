# Atom VoiceS3R C126-ECHO 适配记录

本次型号为 ESP32-S3-PICO-1-N8R8、8 MB Flash / 8 MB Octal PSRAM 的 C126-ECHO（旧称 Atom EchoS3R），不是 classic C008-C。官方产品页和音频板原理图确认：ES8311 的 SDA45 / SCL0，CE 接地、地址 0x18；I2S MCLK11 / BCLK17 / WS3 / 芯片输出48 / 芯片输入4，NS4150B 使能18，用户按键41。没有可由 ESP32 驱动的 RGB 或屏幕；IR47、复位监督 MCU、录音和 USB 音频须单独适配。

来源：[官网](https://docs.m5stack.com/en/core/Atom_VoiceS3R)、[主板图](https://m5stack-doc.oss-cn-shenzhen.aliyuncs.com/1182/Sch_M5_AtomS3R_v0.4.1.pdf)、[音频板图](https://m5stack-doc.oss-cn-shenzhen.aliyuncs.com/1182/Sch_M5_AtomEchoS3R_Audio_v1.0_20250716.pdf)。实际抓取时间与 hash 见 [sources.json](evidence/atom-voices3r/sources.json)。厂家主板图部分为通用 AtomS3R 电路、相关器件标 DNP，不能因此给 Echo 增加屏幕、RGB 或 IMU。

## 出厂 UAC 恢复包的布局冲突

官网 [Easyloader](https://m5stack-doc.oss-cn-shenzhen.aliyuncs.com/1182/Atom_EchoS3R_Easy_Loader_for_win.exe) 是 Windows .NET 程序，本次只解析资源和 IL，没有执行它。`MiniBurner.Resource1.resources/firmware` 原样提取为 383,728 字节，SHA-256 `e4e4b272a7f33b022554b02e41edada89c284dc7030c2e4ff51bd14a11c7f664`。

文件前 0x1000 全为 FF；S3 bootloader / 分区表 / app 实际位于 0x1000 / 0x9000 / 0x11000，但表内 app 目标地址为 0x10000。应用描述是 `atom_echos3r_usb_uac` / `bdfdb73` / IDF v5.4.1。资源 `StartAddress` 标称 0x1000，按钮处理 IL 实际硬编码 `write_flash ... 0x000`，并原样写出 byte array。按这个实际命令放入模拟 Flash 后，原始 S3 ROM 输出 `invalid header: 0xffffffff`。

因此这份恢复包不作为本次可直接运行的 merged BIN，也不把自动裁掉前缀当成原始官方镜像验收；保留资料入口和负结果。未实机刷写，不能据此声称所有厂家硬件或其他版恢复包有同样问题。

## 当前实现与探测

独立板型 `atom-voices3r` 接入 G41 与 ES8311 控制总线初始化子集，复用已有 pin-aware I2C 和共享寄存器容器，不声称模拟音频 ADC/DAC。S3 PICO package identity 沿用 AtomS3R 冷启动标识。native 原始镜像实测：

- 官方 UIFlow2 v2.5.3 原始完整 BIN（`3fdc3de97d399fa77f5404ac155051ca1309e7a14dccc9353270f339d80ce33c`）进入 MicroPython v1.27.0 REPL，自报 `M5STACK Atom EchoS3R with ESP32-S3-PICO-1`。保留两条 I2C port-not-initialized 日志，不据此推断完整音频成功。
- 官方小智 v2.2.2 原始完整 BIN（`7997414bec17383bf5b9c5e30cac65c8a17fd013e9f21cce5a3b6c15c2676e0f`）进入配网 AP / Web server 状态。日志不代表宿主浏览器已经能连接该 AP，网络传输与麦克风/音频仍未验收。
- 官方示例仓库固定树 `1bc38736e61a362080d1dffc4020b3a2a1dff6ae` 未找到 EchoS3R/UAC 项目。当前 M5Unified 参考固定为 `368ad60807161625aa65baad37a4a3af2a7eccea`，并非已证明的 UIFlow 发布二进制源码修订。

ES8311 完整寄存器资料采用厂家 rev10.0 / January 2021，由 [Waveshare 提供的 PDF](https://files.waveshare.com/wiki/common/ES8311.DS.pdf) 第 29 页确认 FD/FE 默认值为 83/11；M5Stack 原链接是精简资料。实现仅保存初始化控制字节、共享两路控制器状态，并限定 SDA45/SCL0；完整复位、只读寄存器、时钟状态、ADC/DAC 都未实现。厂商恢复包仍第一项、默认选中；UIFlow2 与小智保持 trial，确认后允许运行。

## 实际网页与输入

原始 UIFlow2 v2.5.3 经生产 WASM、工作台和分享 mini 进入 REPL；镜像未修改、没有执行 stub。API、模型正面 U 形按键和键盘 A 均通过真实 GPIO41 输入，被 MicroPython 的 `M5.update()` / `M5.BtnA.isPressed()` 读成 0→1→0。REPL 表达式只是读取板上按键，不替代物理按键输入。快速释放后的第一次查询两次仍读到 1，稍后查询为 0；没有独立测量宿主最短保持和客体消抖各占多少，不能称为周期精确。

本机 mini 首次加载一个静态 JS 请求发生连接重置，重新加载后确认框、实际启动与 REPL 均通过；该重试保留在收据。设备没有屏幕/RGB，因此 frame 为 null、leds 为空是预期，不绘制虚构输出。音频、麦克风、USB UAC、IR、复位监督器及网络传输仍未验收，也没有实机验证。

## 模型与复现

厂家 STL 的前后壳保持原几何，打印排版副本不当作内部板；公开尺寸 24×24×16.8 mm 校验通过。增加正面实际 U 形按键命中区，移除未建模复位键的交互；扬声孔后为照片推算的暗色遮挡片，位置和厚度只是外观近似，不冒充扬声器 CAD。UNKNOWN PCB 标记上移，避免文字穿出扬声孔；透明紫壳仍能看到 UNKNOWN。背面纸贴、连接器和扬声孔遮挡片不随金色外壳改色。

对照固定官方正、背、侧照片，正面外轮廓、U 形键与孔列已核对。照片与厂家 CAD 的孔列高度、侧面连接器简化和拍照透视仍有差异；保留部分还原，未声明完整 1:1。旧照片 IoU 收据保留原 hash；本次最终资产的尺寸、渲染和人工叠图收据独立放在 [model.json](evidence/atom-voices3r/model.json)，不把旧数值当作新资产测量。

```sh
uv run --with dnfile==0.18.0 --with dncil==1.0.2 python tools/inspect-voices3r-easyloader.py <downloaded-official.exe> output/uac-original.bin
node tools/probe-atom-voices3r-uiflow.mjs <official-version-https-url> <sha256>
python tools/rebuild-product-model.py plan m5stack-c126-echo
```

原始下载和提取的 BIN 仅在忽略目录，不发布到站点或 Git。执行命令、二进制/来源 hash、负结果和限制见同目录收据；持续沿用厂商精确型号的 UIFlow2 与小智 watchlist。

## 回归

[检查收据](evidence/atom-voices3r/checks.json)：稳定版 Rust 1.99.1，两项 Clippy（warnings denied）、1,434 个 Rust 测试/132 组结果、11 个生产 WASM 场景、100 个平台测试及 CI 的 JIT/时序/BLE/性能工具检查通过，golden 未改。新增完整 codec 数据手册时遗漏同步知识条目的更新时间，最后平台复查发现并修正，完整复跑 100/100。

[CPU 五对对照](evidence/atom-voices3r/cpu.json)使用固定 main `84e67571`：S3 0%、C3 0%、C6 +1.961%，每对串口/指令数/周期一致。计时分辨率与跨修订范围限制小幅百分比解释；不归因为确定的板型开销。没有 S3 core 改动，无需重复 Pocket。独立浏览器与 8101 服务已关闭，未改用户 8088。
