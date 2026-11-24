# CloudZao 独立发布导入 · 2026-11-24

本次基于最新 `origin/main` 的 `9e0f09c3f5507e0c50be1bb69d5653a40b42a3d4`，仅导入 Cloudzao 适配，不包含 Waveshare 001–029 的提交或改动。远端推送、Pages 发布和最终运行检查由中央集成步骤完成。

交付包为 `cloudzao-adaptation-20261123 (1).zip`，705,963 字节，SHA-256：
`79468f6dd346216362d18682796b9e178580189532d93447a3d8a319498aa911`。
包内 `cloudzao-adaptation.patch` 为 1,345,916 字节，SHA-256：
`e158b39a1dd18fb11a055fd440d33f0ab5317ad06837158ef3959606e58fe3fe`。

原补丁基线为 `4d8f3d892e9171a61e8ff0008a029db6aea0e4bf`。已在独立临时索引实际应用并核对目标 tree `7f7dc8fad2dd805c3b649d0f21195690f1eb4b1f`，包内八份文件与 66 个交付目标文件均通过 SHA-256 校验。

## 合并与保留内容

原包 66 个文件全部导入，另增加本说明及[机器可读收据](cloudzao-publish-import-20261124.json)。56 个交付文件与原包目标逐字节一致；10 个共享文件保留最新主线内容并加入 Cloudzao 变更。

三方应用产生 7 个冲突：五个目录/知识索引 JSON、运行时按键路由和英文词典。JSON 按稳定 ID 合并，所有原有 ID 保留，非 Cloudzao 对象逐项完全不变。现有触摸能力、特殊电源键和其他品牌按键路由保留，仅加入 Claw 1.54 的盖板按键。目录 `revision` 标记本次 Cloudzao 导入；较新的索引时间及快照原有紧凑格式保留。

| 目录 | 主线原有 | 导入后 | 新增 |
| --- | ---: | ---: | --- |
| 产品 | 269 | 271 | 两款 Claw |
| 固件 | 363 | 365 | 两份 Claw 官方条目 |
| watchlist | 156 | 158 | 两个 Claw 发布发现项 |
| 发布快照 | 156 | 158 | 两个 Claw 发布发现项 |
| 知识索引 | 277 | 279 | 两款 Claw |

五份 JSON 中所有 Cloudzao 修改对象与原包目标，以及此前联合导入提交 `c88abcdc7cee18b424df24a84e150e15d8749ac7` 中的对象完全相等；保留内容随各自基线不同，不把联合分支中的完整目录复制到主线。逐项断言与文件摘要见收据。

两款 INK 的旧产品 ID、默认固件 ID、下载 URL/SHA 和 CAD 保持不变。Claw4 仍为 `runtime.board:null` / `unavailable`；Claw 官方镜像的完整模拟和实机验证标志均保持未验证。原交付的功能边界见 [Cloudzao 适配报告](cloudzao-adaptation-20261123.md)。

## 检查与待办

全部导入后统一执行的 14 项 Cloudzao 目录/模型测试全部通过；目录 schema 与唯一 ID 校验、证据隐私检查通过。

中央集成尚需完成：native/WASM Clippy、Claw 板级与真实客体 Rust 测试、生产 WASM 重建、真实 Worker/WASM 运行时测试、实际浏览器的品牌/旧名搜索及模型输入检查，随后推送 `origin/main` 并确认 Pages。轻量检查不替代这些步骤。

首个可重建的本地镜像是项目原创诊断：`tools/fixtures/cloudzao-claw154-probe.mjs` 的 `cloudzaoClaw154Probe()`，5,760 字节，SHA-256 `666d7b2e553d6788aafc48282ebfec1fb121b76849295437d098b8f55d4ca4f8`。使用 `cloudzao-claw-154`、16 MiB Flash / 8 MiB PSRAM、`app-direct`。串口输出 `CLOUDZAO CLAW154 SRAM READY` 与 `FRAME COMPLETE`；初始全红，BOOT 按下输出 `BUTTON DOWN` 后全绿，释放输出 `BUTTON UP` 后恢复全红。该诊断不代表原厂完整应用已验证。
