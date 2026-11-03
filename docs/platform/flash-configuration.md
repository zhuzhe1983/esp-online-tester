# 烧录时写入 UIFlow 配置

烧录窗口的“随固件写入配置”默认关闭。新建配置可填写名称、UIFlow1 / UIFlow2 格式、数据服务器、Wi-Fi 名称和密码，应用后供单台、并行和逐台连续烧录使用。服务器是主机名或 IPv4 地址，不是网页 URL；使用默认服务端口，不接受带协议、路径、端口或账号的地址。自定义服务器必须实现相应版本的 UIFlow 服务。

配置默认只保留在当前页面内，可在新批次中复用。明确勾选“保存到此浏览器（包含 Wi-Fi 密码）”后才将配置写入本站 localStorage；这些本地保存的数据没有加密。编辑时取消该选项或删除配置会移除其持久副本。密码默认遮挡；配置不进入场景、录制、分享、固件缓存、普通日志或公开状态，公开结果只包含配置名称和格式。

开始前更换配置会更新尚未写入的设备。写入开始后，每台任务固定所选配置；失败重试仍使用原配置。连续模式在当前设备完成后可更换配置，只影响之后明确添加的新设备。设备行保留当时的配置名称；UIFlow1 的 `node_id` 来自每台设备 MAC，API Key 逐台随机生成，同台重试沿用，不把整批设备设为同一身份。

并行模式没有平台设定的设备数量上限，所有明确选择的端口同时开始。浏览器仍逐台授权，成功设备重插不会自动再次写入。实际并行数量和速度受浏览器、系统资源、USB 带宽及供电影响。单台失败或取消不阻止其他设备完成。

## 格式与边界

配置通过浏览器内存中的固件副本生成，不修改原始文件、下载缓存或其 SHA-256。普通芯片、容量、安全状态、分区/镜像范围检查和 MD5 写后校验仍然执行；未选择配置时保持原写入范围。

- **UIFlow2**：只接受分区表中唯一且未加密的 `nvs` 数据分区，地址 `0x9000`、大小 `0x6000`，且分区不得重叠。替换该范围，保留其前后固件字节；分段镜像同样按实际范围处理。生成 NVS v2 的 `uiflow` 命名空间，写入 `server`、`ssid0`、`pswd0`，其他两个 Wi-Fi 槽清空。其余默认值与当前官方网页烧录器一致：Wi-Fi、DHCP、启动菜单 `boot_option=1`、GMT0 和三个 SNTP 服务器。**整个 NVS 配置被重建，其他已保存设置不保留**；不声称只更改原有 NVS 的三个键。
- **UIFlow1**：只接受从地址零开始、长度按 4 KiB 对齐的完整 merged BIN。JSON 配置位于原始 BIN 末尾，下一个扇区是旧版 Wi-Fi 数据块；两扇区必须在设备容量内，不得覆盖任何已声明分区（包括 OTA 数据、NVS、PHY 和文件系统）。部分 UIFlow1 镜像把尾部空间列入分区；缺少其专用布局依据时拒绝附加配置，不照搬网页的无条件末尾追加。JSON 以两字节大端 UTF-8 长度开头，随后是 JSON 和累加校验字节。Wi-Fi 名称和密码各使用 50 字节槽（长度、内容、累加校验，未用字节填 `0xff`），其余扇区填 `0xff`。不对输入做静默截断，UIFlow1 密码超过 48 字节即拒绝。
- 固件格式由用户明确选择。分区布局相符只证明可安全定位对应范围，**不能证明任意第三方固件会读取 UIFlow 配置**。不支持的布局禁用配置编辑，仍允许按原有规则烧录固件。
- 经典 ESP32 现在可通过独立的只读 eFuse 安全检查烧录；ESP32-S3 / C3 / C6 使用 security-info。见 [安全检查依据](flash-security.md)。不支持安全启动、加密或下载保护设备。

## 公开依据

检查时间：2026-11-03。原始网页脚本未复制进本项目；实现依据公开格式独立编写。来源的抓取时间、字节数和 SHA-256 见 [sources.json](evidence/flash-configuration/sources.json)。

- [M5Burner app 147 烧录实现](https://burner.m5stack.com/assets/FlashDialog-D4GFnLNb.js)：UIFlow1 末尾两配置块、设备身份；UIFlow2 的 NVS 地址、键和默认值。
- [M5Burner 配置块编码](https://burner.m5stack.com/assets/firmware-CR7hbs4Z.js)：UIFlow1 JSON 与 Wi-Fi 长度/校验格式，以及五个服务器选项。
- [M5Stack UIFlow2 默认 NVS](https://github.com/m5stack/uiflow-micropython/blob/f20d5a00e8c9106aae1d944f3d6f1128839f59cb/m5stack/partition_nvs.csv) 和 [16 MiB 分区表](https://github.com/m5stack/uiflow-micropython/blob/f20d5a00e8c9106aae1d944f3d6f1128839f59cb/m5stack/partitions_16mb.csv)：固件侧默认键和配置分区。固件默认时区为 GMT+8，官方网页生成器为 GMT0；本功能采用后者。
- [M5Stack 所用 NVS Python 生成器](https://github.com/m5stack/uiflow-micropython/blob/f20d5a00e8c9106aae1d944f3d6f1128839f59cb/tools/nvs_partition_gen.py)：独立二进制对照工具。

网页 UIFlow2 默认主机是 `uiflow2.m5stack.com`，所检查的表单没有按访问者 IP 切换该字段的逻辑。未调查服务端或 DNS 地域调度，不据此推断服务器地理位置。

## 验证

`tools/fixtures/flash-configuration/uiflow2.csv` 使用公开的虚构测试网络，生成物不含真实凭据。下载上述固定提交的 `tools/nvs_partition_gen.py`，运行：

```sh
uv run --with setuptools --with cryptography --with future python /path/to/nvs_partition_gen.py generate tools/fixtures/flash-configuration/uiflow2.csv tools/fixtures/flash-configuration/uiflow2-nvs.bin 0x6000
node --test tools/platform-flash-configuration-test.mjs tools/platform-flash-batch-test.mjs tools/platform-flash-security-test.mjs
```

浏览器编码的 24 KiB NVS 与独立官方生成器输出逐字节相同，fixture SHA-256 为 `983a2992c2e118dc15320932ffea97038a7c54206c8d4ae7e8b1ccb902f184cf`。另测 UTF-8/逗号输入、UIFlow1 长度和校验、配置区边界、原镜像不变、逐台身份、同台重试、经典 ESP32 安全拒绝、默认不持久保存密码、连续模式配置切换，以及 17 台并行的失败/取消隔离。

`tools/fixtures/flash-configuration-browser-check.js` 通过仓库 Playwright wrapper 执行，使用真实页面与虚构串口/loader，覆盖编辑、自定义服务器、密码遮挡、本页/显式持久配置、7 台同时写入时实际交给 loader 的 NVS、连续模式切换、状态脱敏和窄屏布局。`tools/fixtures/flash-browser-check.js` 复查原有烧录、换设备、重试、取消和嵌入页流程。

这些验证没有连接或刷写实机，没有确认任何真实 Wi-Fi 或 UIFlow 服务连接成功；配置格式通过不等于特定固件或服务器已经完成实机验收。
