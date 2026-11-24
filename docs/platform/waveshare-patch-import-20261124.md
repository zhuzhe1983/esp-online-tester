# Waveshare 分批补丁导入与续接

补丁来源：https://waveshare-adaptation-batches-20261124.zhuzhe1983.chatgpt.site/

第 **001–029 批**已按顺序导入。原始与 gzip 文件均逐一核对字节数和 SHA-256；整条链先通过临时索引检查，随后逐批应用、提交并核对清单中的目标 Git tree。导入过程没有运行分批测试，所有测试在第 029 批完成后统一进行。

- 基线：`9e0f09c3f5507e0c50be1bb69d5653a40b42a3d4`。
- 第 029 批本地提交：`78ac0278f7418556f4afa3327b5b6ce28ae453c9`。
- 第 029 批精确 tree：`0487e31cf6605f3adc0a060afa54c699804fa175`。
- 当前清单：145 / 149 个型号家族，367 / 383 个 SKU，391 个配置；`complete: false`。
- 下一批：**030**。后续清单可能继续增加，未设置自动轮询或定时任务。

[机器可读进度](waveshare-patch-import-20261124.json)记录每一批的下载文件名、字节数、两个 SHA-256、发布方提交、目标 tree 和对应本地提交。发布方提交是交付清单中的来源标识；本地提交列才是本仓库实际创建的提交。原始补丁缓存位于 Git 公共目录下的 `waveshare-adaptation-20261124/downloads/`，未把压缩包重复提交到仓库。源站登录凭据不写入文件。

## 下次 resume

1. 读取本说明与 JSON 进度，获取源站最新的 `downloads/manifest.json`。检查 001–029 的已记录 SHA-256、父提交链和 tree 未被改写；若变化，保留旧缓存并调查，不覆盖既有记录。
2. 只下载 030 及之后的新批次。按编号拼接清单中的 `parts`，校验每个分片、完整 gzip 和解压后的 diff。源站并不直接提供完整 `.diff.gz` URL，页面也是由分片恢复文件。
3. 从上述第 029 批本地提交创建独立工作区。保留的 `codex/waveshare-delivery-20261124` 分支指向未经集成修复的精确交付 tree；即使该本地分支不存在，也可使用记录的提交。这样可以继续用发布方脚本的严格 tree 检查。
4. 在交付分支按序应用、核对 tree 并提交新批次，再将这些新增提交合入最新 `origin/main`。主分支可能已有导入记录或集成修复，不能假定它的 tree 仍等于交付 tree，也不能为适配补丁而重置主分支。
5. **先导完本轮全部新增批次，再统一测试。** 更新 JSON 的最后批次、各批提交、下一批和检查结果。通过必要检查后按项目规则推送 `origin/main`，确认 GitHub Pages 部署，并清理已合入且不再使用的工作区。

## 本轮统一验证

验证修复与发布实现提交为 `738dac99e1915aa9674fbc4799188ec13406d332`，完整命令、哈希、CPU 样本和限制见[验证记录](evidence/waveshare-import-20261124/README.md)。

- 原生和 WASM Clippy 通过；2,895 项 Rust 测试与 520 项平台测试通过，未重建 golden 输出。
- 统一验证真实 WASM、Worker、录制、输入、时序、BLE、网络及 JIT 差分；244 个不同交付模型通过生产加载器检查。
- 浏览器验证九类芯片代表、390 px 嵌入页和打包后的 C5 诊断输出；522 个发布模型的几何字节与来源记录保持一致，本地完整发布目录约 775.5 MB。
- 与已发布主分支比较 S3/C3/C6 hello 和 Pocket Tank，各运行 30 秒 guest 时间；所有测量的 guest 计数和输出一致。CPU 差异全部保留，未声明加速或零开销。

Cloudzao 已先独立推送并完成 [Pages 部署](https://github.com/zhuzhe1983/esp-online-tester/actions/runs/45318986272)，其发布主分支 `d148ef4e` 已成为 Waveshare 集成分支的祖先。下载网站未修改。当前发布目标为 `origin/main` 和 [GitHub Pages](https://zhuzhe1983.github.io/esp-online-tester/platform/)。

模拟器与目录通过检查不代表每个原厂固件或实物均已验证。保留各产品现有的 `trial`、`simulationVerified: false` 与已知限制。
