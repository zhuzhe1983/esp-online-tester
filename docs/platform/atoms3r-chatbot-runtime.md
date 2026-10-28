# AtomS3R AI Chatbot Kit K147：屏幕和按键可运行，音频底座待实现

K147 新增独立板型 `atoms3r-chatbot`。原始官方 UIFlow2 v2.5.3 已通过 ROM 启动，输出真实 128×128 屏幕；点击模型屏幕上的物理按键，界面从 Access Code / Nickname 翻到 IP Address，键盘 A 可以翻回。工作台和 mini 分享页均已验证。当前仍是部分适配：小智能识别屏幕，但因音频底座未实现而报告 `Atomic Echo Base is disconnected`，不能进行语音对话。

## 硬件依据和边界

[官方 K147 页面](https://docs.m5stack.com/en/core/AtomS3R-AI_Chatbot)列出 AtomS3R 主机与 Atomic Voice Base，屏幕为 GC9107。现有 C126 板型对应 ST7735 批次，故保留其构造函数和窗口行为，另设 K147 变体。固定 M5GFX `65c816a38386062d2c3dc47d41237a9083acbc75` 给出 GC9107 ID `00 91 07`、128×160 GRAM 和 y32…159 的可见窗口；本轮不将该验证扩展到后来更换 ST7735 的套件批次。

主机 SPI 使用 MOSI21 / SCLK15 / CS14 / DC42 / RST48；G41 是可按下的屏幕按钮，不是触摸屏。LP5562 白光背光、BMI270 轮询子集和 PICO 包型 1 沿用主机路径，Flash / PSRAM 均为 8 MiB。主机 I²C 为 SDA45 / SCL0；音频底座 I²C 为 SDA38 / SCL39，I²S 为 G5 / G6 / G7 / G8。后者尚未挂载 ES8311，不伪造器件 ACK。

BMI270 的共享复位修正与 `b02b81c2` 完全相同：`ACC_RANGE=2`（8g），依据 Bosch BST-BMI270-DS000-08 §5.2.42。整合时保留一次即可。本轮仅用单元测试覆盖该复位语义，没有以 K147 官方固件证明 IMU 读数。场景姿态到真实传感器坐标尚未校准，`source:'scene'` 仍不开放。

## 官方固件

| 固件 | 实际结果 | 二进制 SHA-256 |
| --- | --- | --- |
| UIFlow2 v2.5.3 | 原生及生产 WASM 启动；实际 G41 和键盘 A 翻页；离线 Wi-Fi / UIFlow 状态为 NG | `91c0f97c904271d4c47d240165fbf5589d44aa0d753ca17d685bfdce5bff199a` |
| 小智 v2.2.6-NiHaoXiaoZhi | 原生识别 GC9107 并写屏，随后报告音频底座未连接 | `ccf81fdcc9cc3c8781965ea62c55409e85aea58e891086bd9215e00913948ef0` |

两份均为未经修改的官方 M5Burner 完整镜像，分别为 8,384,512 与 7,477,968 字节；只保存远程链接，不发布二进制。API 项目、设备、版本和抓取凭据见 [sources.json](evidence/atoms3r-chatbot/sources.json)，原生结果见 [native.json](evidence/atoms3r-chatbot/native.json)。本平台计算的文件 hash 不冒充发布者签名。

已调查 K147 产品页、官方助手教程与 Burner 项目，本轮没有找到明确属于整套 K147 的出厂恢复镜像。C126 主机的 UserDemo 不自动算作整套出厂程序。默认选中官方 UIFlow2 开发固件，保留其 alternative 角色；小智仍可试运行并显示未验证确认，不声称音频已兼容。

## 真实输入与模型检查

可复现官方 UIFlow 检查：

```sh
node tools/probe-atoms3r-chatbot-uiflow.mjs \
  https://burner.m5stack.com/api/v1/firmwares/1897200000000001772/versions/2098279973173547010/download \
  91c0f97c904271d4c47d240165fbf5589d44aa0d753ca17d685bfdce5bff199a \
  > output/chatbot-official-wasm.json
```

该脚本运行生产 worker / WASM，等待 4 秒客体时间，通过真实 G41 按下/释放，再比较实际屏幕标签像素。它不向串口注入界面内容、不替换 firmware、不以输入入队当作消费成功。[official-wasm.json](evidence/atoms3r-chatbot/official-wasm.json)保留客体时间、文件与画面 hash；[browser.json](evidence/atoms3r-chatbot/browser.json)记录实际鼠标、键盘和 mini 结果。IP/MAC 信息页原始像素不提交，公开证据只保留其 hash；公开截图中的 Access Code / Nickname 值为空。

厂家 CAD 几何和贴纸沿用原资料，仅把可按压屏幕面绑定到 `button-a`。独立重建的 8 项资产以及 model.json 与最终库内资产完全一致；正反侧视、原厂色、金色和半透明紫色均检查，纸贴与外壳材质分离。模型请求失败时回退到二维屏幕，未启动时没有伪造帧。历史 CAD 装配尺寸差异、UNKNOWN PCB 和接口空间线序未校准仍保留，未重新宣称内部 1:1 还原；未校准 Grove 不显示已连接线缆。见 [model-validation.json](evidence/atoms3r-chatbot/model-validation.json)及[重建报告](evidence/atoms3r-chatbot/model-rebuild.json)。

## 回归与未完成范围

28 组检查通过，包括 stable Rust、双 Clippy `-D warnings`、1,435 项 Rust 测试（132 组，含 ignored、跳过 external）、11 项生产 WASM、100 项平台测试、真实通用录制回放、JIT 和 schema。没有更新黄金输出。全部命令见 [checks.json](evidence/atoms3r-chatbot/checks.json)；三芯片 hello 五对 CPU 对照见 [cpu.json](evidence/atoms3r-chatbot/cpu.json)，这是跨版本回归检查，不是性能优化结论。

ES8311 / 麦克风 / 扬声器、联网与云端服务、红外、BMM150 AUX 和完整 BMI270 行为未完成；本轮没有得到官方 REPL，也没有真实硬件验收。K147 专属录制逐帧一致未验，通用实际客体录制回归通过不等于该固件确定性重放。所有任务浏览器和临时服务已关闭，用户会话不受影响。
