# TriviaPOD 兼容模拟运行验收

TriviaPOD 的固件目录已指向 StopWatch，但原配置仍是 `runtime.board: null`，因此型号被标记为尚未适配、固件无法运行。现在显式采用 `stopwatch` 兼容模拟模型，保留 **32 MB Flash、8 MB PSRAM、独立产品 ID、存储空间与外观**。共享 UserDemo、UIFlow2 和小智条目继续按试运行提供，不改为 TriviaPOD 原厂问答软件。

## 依据与边界

[官方对比表](https://docs.m5stack.com/en/core/TriviaPOD)确认共同核心规格及 TriviaPOD 的 32 MB Flash、无 Grove/后部扩展/背磁差异。本次公开资料下载记录见 [sources.json](sources.json)。未发现 TriviaPOD 独立 GPIO、触摸控制器或电源原理图，故内部控制器和 A/B 映射明确记为 **StopWatch 兼容模拟配置**，不作为实机引脚已核验的声明。

物理配件路由保持真实产品 ID 隔离；GPS 注入也补上相同的产品范围条件。TriviaPOD 接口筛选只有 USB-C，不因共用 board 获得 Grove 或后部排针。USB 数据与实机刷写未验证。官方 Etermax 问答固件仍未找到公开固定镜像。

## 实际执行结果

使用远程官方 StopWatch UserDemo V0.5，原镜像 SHA-256 为 `6395799089341e0ae96072c2a3418d469d6797b2ab8ee0cab1c9ed24a42acbbe`，3,854,192 字节。完整 ROM 启动后，实际存储作用域为 `m5stack-triviapod`、33,554,432 字节；未把容量改成 16 MB。原文件摘要与填充到设备容量后的存储摘要分别保留。

[official-controls.json](official-controls.json)保留 14 个帧检查点及输入回执。断言通过：A+B 长按进入菜单，B 下一项、A 返回前项，触摸进入应用；秒表启动、计圈、暂停后计时区域保持不变，复位后恢复初始主体像素。检查区域排除了独立时钟，不能仅靠整帧变化冒充按键有效。无固件 panic，真实 framebuffer 为 468×466。

```sh
node tools/probe-official-stopwatch.mjs \
  https://m5burner-cdn.m5stack.com/firmware/6ea670900726398df06ab72c2915db3e.bin \
  6395799089341e0ae96072c2a3418d469d6797b2ab8ee0cab1c9ed24a42acbbe \
  output/triviapod-controls 10 controls m5stack-triviapod
```

真实浏览器检查 `tools/fixtures/triviapod-workbench-check.js` 从产品选择和共享固件库启动官方镜像，核验 32 MB 存储、3D/2D 真实画面、USB-C 配件筛选及 mini 分享恢复。脚本早期的 mini 清理与按钮选择检查有误，分别改为仅暂停运行中的实例、使用 mini 的 `start-runtime` 按钮后重跑；前期失败不计为通过。没有请求串口或刷写实机。

模型输入与屏幕覆盖检查另见[模型验收](../triviapod-runtime-model-20261106/README.md)：使用诊断纹理检查几何，原始几何未改。这里的 `triviapod-official-3d.png`、`triviapod-official-2d.png` 则来自真正运行的官方固件。

## 回归与复现

- 348 项平台测试通过，包含容量/存储身份隔离、固件可试运行、物理接口与 GPS 隔离。
- 目录 schema 通过：217 产品、10 配件、172 固件、118 watchlist。
- 完整本地 CI 通过，含最新稳定 Rust、两个 warnings-denied Clippy、release workspace（包含 ignored、排除 external 输入）、WASM/native/JIT。命令与原日志摘要见 [ci-result.json](ci-result.json)。未改 Rust、寄存器、执行循环或 golden 输出。
- 最终模型、目录与 UI 另经平台测试及真实浏览器检查。来源及最终文件摘要见验证 JSON。

原始网页、固件和包含本地路径的日志留在忽略目录，不提交到仓库；只保留可复核结果、公开来源摘要、虚拟设备输入和实际帧图。这里验证的是兼容模拟配置上的所列操作，不代表 TriviaPOD 实机、专有问答软件或共享固件全部功能通过。
