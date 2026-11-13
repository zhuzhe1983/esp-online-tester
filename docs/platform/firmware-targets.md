# 产品与固件目标

产品可声明 `firmwareTargetId`，用于共用固件目录。省略时使用产品自身 ID。当前值引用目录中的一个规范产品：目标必须存在、芯片相同且指向自身，不允许链式引用或循环。

```json
{
  "id": "m5stack-triviapod",
  "firmwareTargetId": "m5stack-stopwatch",
  "runtime": {"board": null, "flashMb": 32, "psramMb": 8, "status": "unavailable"}
}
```

这条关系决定固件来源，不修改产品身份、运行板型、容量、引脚、接口、模型、存储隔离或分享场景。TriviaPOD 与 StopWatch 的核心规格相近，见[官方对照表](https://docs.m5stack.com/en/core/TriviaPOD)；这里选择 StopWatch 作为候选固件目录。TriviaPOD 另行显式配置 `runtime.board: stopwatch`、32 MB Flash、8 MB PSRAM，采用兼容模拟模型；这不是从固件目标字段隐式继承的硬件配置，也不代表实机内部引脚已核验。

- `firmware.productIds` 保留固件的直接产品映射。属于当前产品的条目优先；共享条目仅在读取时生成，不向原始固件或 watchlist 追加产品 ID。
- 共享镜像显示为 `trial`，没有镜像则仍为 `reference`。不继承 `role: factory`、模拟或实机验证结果，也不继承旧版本的验证证据。来源发布者、URL 和 hash 保持不变。
- `defaultFirmwareId` 仍用于本产品已明确的默认固件。TriviaPOD 没有已找到的公开出厂镜像，不将 StopWatch 出厂固件登记为 TriviaPOD 出厂固件。
- 工作台、嵌入播放器、分享恢复及运行确认使用同一产品范围解析器。版本检查按来源目标查找发布信息；历史记录和问题报告仍归实际产品。
- M5Burner 搜索会查询目标产品，并显示共用目录说明。LILYGO Spark 仍遵循独立的精确 SKU、仓库路径和镜像白名单；本字段不会绕过这些规则。
- 共用目录不增加板级运行支持，不放宽镜像芯片、容量及布局检查，也不放宽实机刷写的直接产品映射约束。TriviaPOD 已单独接入兼容模拟配置，共享条目可以试运行；不同产品的持久化数据与分享身份仍隔离。

修改映射后运行 `python tools/validate-platform-catalog.py` 和 `node --test tools/platform-firmware-target-test.mjs tools/platform-catalog-selection-test.mjs tools/platform-firmware-policy-test.mjs tools/platform-firmware-history-test.mjs`，并检查实际设备切换、远程搜索、版本查询及分享恢复。

PaperMono-Lite（C153-Lite）显式指向 `m5stack-papermono`。两款官方产品页的出厂恢复均指向 [PaperMono UserDemo](https://burner.m5stack.com/firmware/2089640807996628993/)，CrossPoint 均指向[同一项目](https://burner.m5stack.com/firmware/2091144466157694978/)；[官方 UserDemo 源码](https://github.com/m5stack/M5PaperMono-UserDemo)明确列出 C153 和 C153-LITE。2026-11-13 核对的 Burner 设备 API 只有 `PAPER_MONO`（`2093000000000000020`），未单列 Lite，见[目录核对记录](evidence/papermono-lite-burner/receipt.json)。Lite 继续使用独立板型；现有明确映射到两款的 User Demo 保留原验收范围，其余共用候选不继承完整版的兼容结论。
