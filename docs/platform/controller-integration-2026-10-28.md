# 16个控制器适配的统一集成

以 f14d5634 为基线，保留原作者逐链整合 C124、PaperDIY、PaperColor、PaperS3、StampS3、StampS3A、AtomLite、AtomMatrix、NanoC6、Nesso N1、Air Quality、Air Quality v1.1、DinMeter v1.1、Atom Voice、Atom VoiceS3R、AtomMatrix v1.1；并合入主线 6b39ca45 的 S31 独立实现、品牌显示及 Pages 缓存目录修复。

目录共86项；M5Stack 27/84、Metalio 1/2 有板级模型。所有型号保持部分支持与按固件的限制，Atom Voice 蓝牙崩溃、联网缺项、软件GPIO灯驱动等原始负面收据保留。旧独立型号的官方固件验收不是本轮重新逐个执行。

集成保留各 SKU 的 Rust 注册、芯片识别、输入和 IMU 路由，合并厂家模型的独立按键、连接器开口、PCB 标签偏移与派生 LED 字段。删除重复的派生 LED 注册；PaperColor 知识记录补齐已有来源归档名并同步原有更新时间，不重写抓取时间。浏览器发现 PaperDIY 默认固件源的 `format:merged` 不能进入便携场景，改用已有 `bootMode:rom` 保持严格 ROM 启动，并新增所有可执行目录源的场景往返验证。

本轮已完成：

- Rust stable 1.99.1；原生和 WASM 双 Clippy，警告视为错误，均通过。
- 全工作区 Rust 测试含 ignored、跳过 external_*：1558项通过，golden 未更新。
- 平台 Node：101个测试文件通过；另增目录源场景验证通过。
- 11个实际 WASM 场景通过；真实 worker 录制18个事件，回放 framebuffer 一致。
- 原生 VQ 43项、timing/BLE ABI、worker pacing/channel、输入 UI 11项通过。
- 目录 schema：86产品、7配件、150固件、104 watchlist。
- Headless 浏览器逐项选择16个新增型号，CAD 均加载完成；品牌显示与计数正确。PaperDIY 的无效源字段另行修正，随后须复查；此前截图保留为负面证据。

生产 WASM：4,213,648字节，SHA-256 `2a913ce4b9d5db3d8eb144ef7e6bfbeaa5da4d0418d9316e9046bd93bdc87226`；无 DWARF，保留名称。JIT 差分、profiling 编译、benchmark工具与历史 x5 合同在本次核心交付时尚待补跑，不声称完整 CI 已结束。没有新增整合性能测量，保留各实现阶段的对照收据，不作整体性能无回退声明。

真实录制收据见 [recording.json](evidence/controller-integration-2026-10-28/recording.json)。未新增线上固件二进制。任务浏览器和独立服务器均已关闭。
