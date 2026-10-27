# Unit C6L 与 Mesh Kit 的 Meshtastic 固件

2026-10-27：Unit C6L（U202）同时出现在控制器与配件目录。两个入口通过 `catalogRoles` 相互关联，共用 `knowledge/m5stack-u202.json` 与厂家 CAD 资产。C6L 内置 ESP32-C6，可独立运行；作为扩展节点时，主机协议取决于它自身的固件，不能自动当作 UART LoRa 模块。当前两个入口均未实现其 CPU/板级行为；加入配件不会额外启动第二颗 CPU。

| 产品 | Meshtastic 的准确目标 | 稳定版 | Alpha |
| --- | --- | --- | --- |
| Unit C6L U202 | `m5stack-unitc6l` / ESP32-C6 | 2.7.26.54e0d8d | 2.8.1.8e6a88d |
| Cardputer Mesh Kit K152 | `m5stack-cardputer-adv` / ESP32-S3 | 2.7.26.54e0d8d | 2.8.1.8e6a88d |

依据：[C6L 官方说明](https://docs.m5stack.com/en/unit/Unit_C6L)、[官方 Meshtastic 教程](https://docs.m5stack.com/en/guide/unit_c6l/meshtastic)、[Mesh Kit 官方说明](https://docs.m5stack.com/en/core/Cardputer_Mesh_Kit)，以及 Meshtastic 官方发布的逐板型 `.mt.json`。Mesh Kit 是 Cardputer-Adv 加 Cap LoRa-1262；这里的匹配不扩展到没有该扩展板的普通 Cardputer/Adv。

`meshtastic` watchlist 保留一个项目，设置四个明确的设备/发布渠道目标。发布前和手动 `check-firmware-update` Action 共用原有同步入口：读取 GitHub Release 元信息，再只读取 `release.meshtastic.org` 的小型目标清单。核对 `repo`、`version`、`platformioTarget`、`mcu` 与唯一的 `.factory.bin` 文件名；设备不存在时继续检查该渠道的较旧版本，不按 ESP 芯片族分配整包 ZIP。网站访问者仍读取静态快照，不调用 GitHub API。

**本次四个镜像已在开发机内存核对 SHA-256、官方 MD5、ESP 启动/应用段校验和及分区容量；没有执行、刷写或将 BIN 保存到磁盘/仓库。** 收据见 [image-checks.json](evidence/meshtastic/image-checks.json)。网站只保存 URL 和元信息，用户浏览器可按原有策略缓存。`.factory.bin` 不包含另行发布的 LittleFS 和 OTA 辅助镜像；本次不宣称网页、OTA 或 LoRa 功能已验证。

当前官方目标清单只提供 MD5。自动更新不会把 MD5 当成 SHA-256，也不会下载 BIN 计算 hash：新版本没有来源提供的 SHA-256 时，快照保留候选版本/URL，报告 `candidate-missing-sha256`，已固定固件继续保留。后续官方提供 SHA-256 可沿现有更新流程写入；手工核验新版本也可通过 PR 更新。逐板型清单不获取 GitHub token。

手动复核示例（固件仅驻留进程内存；不在 Action 中运行）：

```sh
python3 tools/audit-meshtastic-images.py --flash-mib 16 --manifest-url \
  https://release.meshtastic.org/2.8.1.8e6a88d/firmware-m5stack-unitc6l-2.8.1.8e6a88d.mt.json
```

容量须依据具体硬件填写；C6L 为 16 MiB，Mesh Kit 为 8 MiB，不按芯片名称猜测。

M5Stack 官方条目仍排第一。Mesh Kit 保留已收录的 M5Stack v2.7.22 出厂镜像；C6L 官方页面仅找到 User Demo Easyloader 恢复入口，版本化独立 BIN 未确认，因此默认项明确为恢复工具资料入口。第三方 MeshCore 条目及 Meshtastic 上游新版本均不冒充 C6L 原厂烧录版本。

两台设备当前没有对应板级模拟器，运行按钮隐藏并显示“尚未完成适配”。固件入库与模拟器就绪分别记录。来源 CORS 仍由上游服务器决定，页面提供官方刷机站链接；不以本站白名单绕过浏览器 CORS。
