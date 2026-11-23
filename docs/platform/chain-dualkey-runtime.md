# M5Stack Chain DualKey C147

C147 使用独立板型 `chain-dualkey`。设备有两颗热插拔机械键、两颗 WS2812B 和两路 Chain 端口。目录标为部分支持。板级模拟接通了两键 GPIO 和灯光供电及数据路径，3D 依据厂商机械图与照片建立。[厂商资料](https://docs.m5stack.com/en/chain/Chain_DualKey)

| 元件 | 来宾固件的可观测行为 |
| --- | --- |
| KEY1 与 KEY2 | G0 与 G17 低有效，释放后为高。在工作台可点按 3D 键帽，也可使用 A、B 键或面板按键。 |
| 两颗 WS2812B | G40 作为 GPIO 驱动低电平时供电；G21 收到 RMT 数据。断开供电后灯色清零。传输顺序为 KEY2 下方的灯、KEY1 下方的灯。 |
| 显示 | 该设备没有屏幕。应用日志走 UART0 或 USB，具体端口取决于固件。 |
| Chain 口 | 原厂 V0.8 初始化 UART1（TX48/RX47）和 UART2（TX6/RX5），速率都是 115200。真实外接 Chain 节点和供电未建模。 |

## 官方出厂固件

目录默认选择官方 User Demo V0.8 的完整 `0x0` 镜像。实际 WASM 通过 ROM 启动，USB 日志显示项目 `M5DualKey-UserDemo`、应用自报版本 `V0.7-2-g82d7fa6` 与 ESP-IDF `v5.5.1-dirty`。来宾开机 ADC 测试读取到 0.00 V，两颗灯亮红色，随后日志到达 BLE 初始化。单键和双键均改变模拟 GPIO 输入，约 9.6 个来宾秒内未观察到原厂键盘回调或灯色响应，因此保持试运行标记。[官方发布](https://github.com/m5stack/M5DualKey-UserDemo/releases/tag/V0.8) · [运行记录](evidence/chain-dualkey/runtime.json)

GitHub 发布资产的下载响应缺少浏览器 CORS 头。在线运行依赖站点中继，用户也可下载官方 BIN 后本地导入。UiFlow2 v2.5.5 是同型号的备选固件，M5Burner 下载需要验证，尚未取得文件。该型号未发现官方小智版本。[固件使用指南](https://docs.m5stack.com/en/guide/chain_dualkey/usage)

## 按键与灯光验收

内部 ESP-IDF 小程序读取实际 G0/G17，并由来宾 RMT 驱动两颗灯。分别验证了两键按下、释放、组合键和录制回放。UART0 出现 `C147 KEYS 1 0`、`1 1`、`0 1`、`0 0`，同次采样的两颗 LED 状态随之改变。Chromium 工作台实测了键盘 A、3D KEY1 和面板 KEY2，已用独立嵌入页核对共享产品身份。该验证覆盖板级 GPIO 和 RMT，原厂 HID 与网页配置仍待验证。

三档滑动开关、外部 Chain 协议和插拔、电池 ADC、USB/BLE HID、低功耗和物理开关时序均未建模。模型外形来自尺寸图和公开照片，内部主板使用标为 UNKNOWN 的占位层。端口保留外观，装配与插合深度待实物核对。[模型核验](evidence/chain-dualkey/model-verification.json) · [来源记录](evidence/chain-dualkey/sources.json)

## 复验

按平台文档构建 WASM 并准备 ESP32-S3 ROM。用官方原文件和自己的夹具输出运行探针。仓库只保留夹具源文件、固件链接和摘要。

```sh
cargo test --release -p esp32s3 --lib board::chain_dualkey::tests
idf.py -C tools/fixtures/chain-dualkey-firmware -B "$PWD/output/c147-build" \
  -D "SDKCONFIG=$PWD/output/c147-build/sdkconfig" build
idf.py -C tools/fixtures/chain-dualkey-firmware -B "$PWD/output/c147-build" \
  -D "SDKCONFIG=$PWD/output/c147-build/sdkconfig" merge-bin -o "$PWD/output/c147-fixture.bin"
node tools/probe-chain-dualkey-factory.mjs --factory /path/to/official-v0.8.bin \
  --fixture output/c147-fixture.bin
node tools/rebuild-chain-dualkey-model.mjs output/c147-model
```
