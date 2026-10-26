> Historical feasibility research and draft API. For the implemented platform, see [the current guide](../README.md) and [the current skill](../../../skills/esp-online-tester/SKILL.md).

# ESP Online Tester 设计草案

目标是一个部署在 GitHub Pages 的静态 ESP 设备仿真网站：选择产品、连接配件、加载固件、通过 3D 外观操作设备，并让 coding agent 使用同一套接口配置和测试。

本轮范围为研究、架构和 skill 草案，尚未实现或部署网站。首批参考硬件是用户持有的 **M5Stack PaperMono、StopWatch、GPS Unit**。

建议采用 **静态前端 + Worker 中的 WASM 仿真器 + 可组合的板级与配件模型**。优先验证开源 `mikroverk/esp32sim` 的 StopWatch 模型；`espressif/esp-emulator` 保留为第二后端候选。前者已有可修改的板级代码，后者当前公开 WASM 的外设扩展接口不足，不能直接承诺完整产品仿真。

- [可行性与选型](docs/feasibility.md)：上游现状、实际接口检查、三个参考硬件、静态部署的边界。
- [系统架构](docs/architecture.md)：三类目录、DIY 编辑器、3D 操作、固件加载、分享配置和 PR 流程。
- [验证与实施顺序](docs/validation-plan.md)：StopWatch＋GPS 第一闭环、PaperMono 第二闭环、实机对照和验收门槛。
- [Agent skill 草案](skills/esp-online-tester/SKILL.md)：系统发现、配置、操作、测试和结果报告；项目 API 尚待实现。
- [Agent API 合约草案](skills/esp-online-tester/references/api-contract.md)：统一的程序化操作和结果格式。
- [场景草案](examples/)：未配置固件，不是可运行示例。
- [WASM 检查记录](docs/evidence/esp-emulator-wasm-audit.json)：0.51.0 发行包哈希、实际导出方法及 Node 初始化结果。

核查日期：2026-10-26。具体兼容性必须绑定引擎版本、设备修订版和固件 SHA-256；“商品已收录”不表示“固件已验证可运行”。
