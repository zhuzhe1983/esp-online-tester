# 2026-10-28 集成收尾验证

[收据](receipt.json) 固定集成源代码、生产 WASM 哈希、检查结果和限制。初次平台检查的两项失败保留在收据中；采用 `8736ff3f` 的测试入口修复后，最终 124 项平台检查全部通过。

- CI 命令见 `.github/workflows/ci.yml`；稳定 Rust、双 Clippy、1610 项 Rust、真实 WASM 示例、录制、BLE/计时、JIT 差分和证据检查通过。
- PaperS3 使用调用方提供的原始固件，命令为 `node tools/probe-papers3-sd.mjs --file <private-factory.bin>`。真实 worker 完成 FAT32 挂载、跨 5 簇文件写入、重启和新 worker 恢复；清空和导入只在隔离测试存储中执行。
- Chromium 隔离会话从本地文件入口启动同一镜像，确认 TF 卡写入 IndexedDB；完整刷新后重新加载镜像，卡内容哈希完全一致。未读取或清空用户正在使用的浏览器存储。
- M5Stack 系列筛选、PaperS3 存储入口经过实际页面检查。Atom-Matrix 灯光与拖拽的专门证据见相邻 `atom-matrix-led-20261028/` 和 `../../atom-matrix-runtime.md`；三项配件验收见 `../accessory-integration-2026-10-28/`。

私有固件、生成的文件内容和名称、原始串口以及固件页面截图均未纳入仓库。接口说明已纠正品牌限制，但并不代表模拟器已实现任意主控间的通用 GPIO/I²C 转接。
