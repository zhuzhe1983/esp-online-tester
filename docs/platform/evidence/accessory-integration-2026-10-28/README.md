# ENV-IV、DLight 与 PIR 集成验收

基线 `61e1808f`，保留该提交全部控制器、LILYGO、固件及知识条目；按 ENV-IV → DLight → PIR 顺序整合至 `85f1fed4`。另修复 ENV 客体探针的串口查询屏障，避免将启动默认采样误认为查询响应。

三个配件都沿 StopWatch Port A 的真实 GPIO/I²C 与 M5PM1 供电门控工作。ENV-IV 和 DLight 可经 U006 在不同 I²C 地址并联；PIR 的 G11 输出不能与该总线争用。三种冷输入、录制、面板与 mini 分支同时保留，不新增主机支持、不声称跨品牌通用接线。

- 双 Clippy、1575 项 Rust、114 项平台测试、11 组 WASM、真实录制回放通过。
- 三件真实 worker 共 37 组边界测试通过：ENV-IV 11、DLight 15、PIR 11；覆盖错线/地址、断电、拔插、客体/平台重启、回放及 Hub。
- 111126 项 JIT 差分、24 项 GPIO 波形、profiling 构建、VQ、Timing/BLE ABI、worker、benchmark contracts、retained x5 与 AArch64 核测试通过。完整命令见 `checks.json`。
- 工作台面板及分享 mini 使用同一生产 WASM。未验证固件先确认，再自动启动。最终读数来自 UART0 完整行；ENV-IV 为 22.501°C/61.201%/990.100hPa，DLight 为 4321.6667lx，PIR 为实际 G11 0→1→0。
- 模型资产与三个来源提交逐字节相同；复用原尺寸/照片验证，不改旧时间及指标。本次重新加载三个模型、检查连接器视图及实际场景。UNKNOWN、几何估算和光学限制保持原声明，未实机验证。

生产 WASM SHA-256：`6ed54509073ab7229ee721164b56692178b67d477581f70f4e0e6c72687b7a4e`，4261828 字节。JIT/profiling 检查后恢复同一生产产物。

CPU 对照以独立 `git archive 61e1808f` 构建为基线，三组 hello 各一对排除热身、五对交替测量；所有指令数与输出 hash 相同。S3 0.32→0.30 秒（−6.25%），C3 3.42→3.49 秒（+2.05%，4/5 对较慢），C6 2.42→2.37 秒（−2.07%）。这是共享宿主环境下的观测，不宣称提速或性能无回退；保留 C3 代价。未改 S3 CPU 核心，不重复 Pocket 计时；PIR 原先针对 `5a9b68e0` 的 Pocket 五对结果仍保留在该 SKU 的 `runtime/current-main/cpu.json`，不将其冒充本次组合测量。对应 EX216 的冷 Board 状态/回调布局检查。

复现：按各 `tools/fixtures/unit-{env4,dlight,pir}-firmware` 的源码构建说明生成内部镜像，分别运行 `node tools/probe-unit-*.mjs --build <build-directory>`。内部镜像仅位于 ignored output，不进入固件库或发布目录。`browser.js` 是 Playwright wrapper 的 run-code 回调，先打开调用者指定的仓库静态服务，再提供 output 下三份带 hash 的 fixture manifest；脚本以当前页面 origin 解析资源。CPU 命令为 `python bench.py --base-bins <baseline-release> --candidate-bins <candidate-release> --rom-dir <roms> --receipt <output.json>`。

保留首轮失败与修复依据于 `negative-results.json`。证据不保留用户名、原始进程清单或浏览器会话标识；截图与实际 UART 数值保留。任务自有浏览器和服务已关闭，用户 8088 未改动。
