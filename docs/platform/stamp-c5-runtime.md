# Stamp-C5 SRAM 应用直启

S016 与 S016-DIP 使用各自的 `stamp-c5`、`stamp-c5-dip` 板型。当前支持真实 RISC-V 解释执行的 SRAM 应用，4 MiB Flash 仅用于装载文件，PSRAM 为0。它不执行 C5 ROM，也不补造 IDF 启动链。官方 UIFlow 仍保留试运行来源，但普通 IDF、合并 Flash 或 flash-mapped 镜像不在当前可运行范围内；独立诊断程序不会替代官方固件成为公开默认项。

在工作台选择该产品，打开“本地文件”并选择一个 `.bin`。校验成功后会明确显示“C5 SRAM 应用”及直接启动方式，然后可点击运行。文件只在浏览器读取，不上传。API/嵌入页面采用同一校验和执行路径：

```js
await espTester.loadFirmware({file, bootMode:'app-direct'});
await espTester.start();
// 本地 C5 单文件也允许省略 bootMode，校验后明确选择 SRAM 直启。
```

远程裸机 BIN 必须明确选择界面的“C5 SRAM 应用直启”地址类型，或传入 `{url, sha256, bootMode:'app-direct'}`。若文件确有完整 IDF app descriptor，可在同样严格的 SRAM 校验后自动识别；描述符并不能使 flash-mapped IDF 固件变为可运行。远程文件仍需满足 CORS、大小和 hash 要求；分享保留实际 SHA-256 与启动方式。

清单只允许一个 app 文件，且必须声明 `app_direct:true`：

```json
{
  "board": "stamp-c5",
  "flash_mb": 4,
  "psram_mb": 0,
  "app_direct": true,
  "files": {"app": {"url": "sram-app.bin", "sha256": "<实际 SHA-256>"}}
}
```

清单不得夹带 ROM、bootloader、分区表、ELF、Flash 分段、ROM stub 或 WiFi 配置。DIP 产品使用 `board:"stamp-c5-dip"`。镜像沿现有 kind3 App 通道暂存于 `0x10000`，只将经校验的各段复制进 SRAM 后进入入口，不从 Flash 窗口执行。

校验合同与 C5 Machine 一致：ESP magic0xe9、芯片ID0x17、1–16个非空段；地址与长度按4字节对齐，无重叠，全部落在 HP SRAM `[0x40800000,0x40860000)` 或 LP SRAM `[0x50000000,0x50004000)`。入口按2字节对齐，且至少有2字节属于已加载段。虚拟 revision100 必须位于 full min/max 范围，max0视为未设；废弃的 header byte14 不参与判定。按0xef初值计算各段 XOR、校验块填充必须为0；header23为0/1，有附加 SHA-256 时必须核对，余下文件只能是0xff。裸机应用不要求 IDF 描述符，有该 magic 时描述符必须完整。来源：[ESP-IDF v5.5.4 image header](https://github.com/espressif/esp-idf/blob/v5.5.4/components/bootloader_support/include/esp_app_format.h)、[revision 检查](https://github.com/espressif/esp-idf/blob/v5.5.4/components/bootloader_support/src/bootloader_common_loader.c)、[C5 Machine 证据](../evidence/c5-bringup/machine-runtime.md)。

暂停停止客体推进，并拒绝新输入。平台“重启”会重新建立机器、清空数字外设状态并重新加载同一个 SRAM 应用；它不是 ROM 启动，也不承诺支持固件自行调用 ROM 复位。当前可验收输入为 UART0；USB 控制台和 UART1 不作为已验证入口。板载输出是 G28 低有效蓝灯，不是 RGB。WiFi、音频、SD、OTA、持久 Flash 数据和物理串口时序未声明支持，运行能力不会继承其他芯片的默认值。

`node --test tools/platform-c5-test.mjs` 验证地址/入口/版本/完整性负例、两SKU的本地及远程模式、清单边界，以及不请求ROM和不启用存储/音频/网络的调度合同。该测试使用协议替身，不是固件执行证据。`node tools/probe-stamp-c5.mjs --firmware <GCC/esptool-built probe.bin> [--assets-root <repository>]` 则使用真实 worker/WASM，检查 UART0 中断→蓝灯、错误G27、暂停以及同映像重启。浏览器上传和实际执行的结果单独保留，不以加载回执代替输出。
