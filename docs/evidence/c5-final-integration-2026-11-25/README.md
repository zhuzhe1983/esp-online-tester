# C5 LCD273：离线原厂界面整合

本次交付为固定 Waveshare ESP32-C5-LCD-2.73 原厂镜像增加实验性正常 ROM 启动及明确选择的离线 Wi-Fi 驱动兼容模式。默认严格模式保留；原厂 BIN/ROM 字节不改。原厂菜单、三个 GPIO 按键和 TF JPEG 读取/解码/显示已有原生与生产浏览器证据；不能据此称完整 Wi-Fi/RF、所有原厂应用或其他 C5 产品通过。

| 范围 | 结果与收据 |
| --- | --- |
| 正常 ROM/bootloader 与独立厂家03_SD/02_LVGL参考、项目独立诊断 | [分别保留的参考证据](../c5-lcd273-references-2026-11-25/README.md)，不替代原厂验收 |
| 离线驱动 API / 默认严格控制 | [固定入口、物理映射、ABI、权限与事件合同](../c5-wifi-driver-compat-2026-11-25/README.md) |
| 原厂菜单、GPIO输入、返回和正常卡重载 | [两轮生产浏览器结果](../c5-factory-browser-2026-11-25/README.md)，旧Photo失败仍保留 |
| TF JPEG真实消费与显示 | [缓存普通未对齐访问修复](../c5-factory-photo-2026-11-25/README.md)；原生/浏览器 RGB565 和 RGBA 全字节一致 |
| Sensor页数值 | [未通过的时序/端序证据](../c5-factory-sensor-2026-11-25/README.md)；保持最大期限、NACK及BE，不伪造正确值 |
| 全局功能检查 | [3513 Rust、具名原厂1、VQ44、双Clippy、26组Node/平台554、14固件、WASM JIT差分与profiling](../c5-local-checks-2026-11-25/README.md) |
| 最终支持说明 UI | [真实DOM与servedhash](catalog-ui.json)：默认strict、trial、离线控件和五段限制文本正确；本检查不运行固件 |
| 非C5 CPU控制 | [24个原样本](../c5-final-cpu-2026-11-25/README.md)：同工作/串口，S3+2.03%、C3−0.94%、C6−0.35%、Pocket+3.66%，范围重叠，不声明零开销 |

实际生产 WASM 为8902103B、SHA256 `dee308ca758e02e5574d1888b3b68d7f7b7071da6d0f193347a4cf8628ceb28f`。功能测试和CPU的编译检查点为本地保留的c8232bfc；后续仅修正冷native示例的一行观察条件，并补充证据/支持文字。[源码对应](source-identity.json)逐文件绑定验证源码与公开树，保留实际检查点身份，不把后继文件倒填为旧运行。

原厂严格控制仍在PC408142c6读取私有MAC600ad858时停止，steps44199490/cycles44199486/UART4536B原样保留；兼容模式只能显式启用，关联和IP均false，Wi-Fi配置仅session-RAM。源API边界、真实guest事件队列和driver虚拟槽单列，不能据GUI出图宣称原生RF。

全工作区golden未改。两例外部network-relay检查因未提供caller目标跳过，平台554项及四个私有C5输入用例均实际执行；0用例错误filter、早期有意中断、observer误选/部分帧、Sensor失败和CPU较高median均保留于各自收据。功能wall包含编译/等待，不是速度样本。

公开提交基于已发布main930731a2，以精简后的最终文件树整合，保留Cloudzao、Waveshare至030以及已发布Adv修复。私有原始捕获祖先不进入公开历史，后续批次游标031。CI与Pages由本公开提交的GitHub Actions另行验证，不从本地收据推导成功。

原始BIN/ROM、输入卡、UART、帧及日志留在私有ignored存储，公开目录只包含必要事实、参数化小脚本/补丁、来源、数字和hash。[curation.json](curation.json)记录组件草稿到公开版本的对应及删减影响。这里不声称私有捕获已有公开下载，也没有实机访问、物理校准或完整硅片验收。
