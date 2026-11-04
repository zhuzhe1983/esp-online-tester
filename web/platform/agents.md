# 用智能体开发和测试 ESP 产品

ESP Online Tester（ESP Workbench / 产品实验台）让智能体与人使用同一套 ESP32 产品运行环境：选择设备和配件，在浏览器的 WebAssembly worker 中执行固件，查看串口、屏幕和已实现的外设输出，再把场景分享或嵌入网页。

[打开实验台](https://zhuzhe1983.github.io/esp-online-tester/platform/) · [阅读网页版指南](https://zhuzhe1983.github.io/esp-online-tester/platform/agents.html) · [llms.txt 导航](https://zhuzhe1983.github.io/esp-online-tester/llms.txt)

## 选择适合任务的 skill

### esp-online-tester：操作与测试

适合已经在目录中的设备。让 coding agent 选择设备和固件、配置配件、触发按键或传感器输入，检查实际固件输出，保存录制并分享场景。平台提供 `window.espTester` 和 iframe RPC；无须另起远程模拟器。

- [阅读 SKILL.md](https://zhuzhe1983.github.io/esp-online-tester/platform/skill/SKILL.md)
- [获取完整 skill 目录](https://github.com/zhuzhe1983/esp-online-tester/tree/main/skills/esp-online-tester)
- [API 参考](https://zhuzhe1983.github.io/esp-online-tester/platform/skill/references/api.md)
- [自动化测试、音频与网络工作流](https://zhuzhe1983.github.io/esp-online-tester/platform/skill/references/agent-workflows.md)

可以交给智能体这样的任务：“阅读 esp-online-tester skill，在实验台中找到我的具体型号，检查支持范围，加载我的固件，并用实际串口或屏幕输出验证按键响应。”

### esp-product-onboarding：产品适配与入库

适合尚未适配或需要更新的产品、模块和固件。在仓库中核对厂家、SKU 和版本，采集有来源的规格与 CAD，制作和校验 3D 模型，维护产品知识、固件目录和验收证据，再通过 PR 贡献。

- [阅读 SKILL.md](https://zhuzhe1983.github.io/esp-online-tester/platform/onboarding-skill/SKILL.md)
- [获取完整 skill 目录](https://github.com/zhuzhe1983/esp-online-tester/tree/main/skills/esp-product-onboarding)
- [模型与贴图校验](https://zhuzhe1983.github.io/esp-online-tester/platform/onboarding-skill/references/model-verification.md)
- [固件来源与验证](https://zhuzhe1983.github.io/esp-online-tester/platform/onboarding-skill/references/firmware.md)

可以交给智能体这样的任务：“阅读 esp-product-onboarding skill，为这个准确 SKU 和硬件版本补充官方资料、模型和固件，分别报告外观、运行验证及尚未支持的部分。”

## 开始使用

1. 让支持读取网页的智能体打开对应的 SKILL.md，并按需读取其 references。需要本地安装时，获取完整 skill 目录（包括 references），按智能体工具的安装方式使用；文件链接本身不会自动安装。
2. 操作现有设备时打开实验台；导入产品时获取本仓库，并先读仓库的 AGENTS.md 和 skill。明确设备 SKU、版本和待验证的固件。
3. 检查能力与支持范围。加载固件、发送输入后，观察新的串口、framebuffer 或外设输出，再记录结果和限制。

各产品的模拟支持程度不同。模型可以显示、输入已排队或固件在目录中，都不能证明完整兼容。网络访问可能需要自行部署的 helper；具体边界以 skill、能力查询与实际运行结果为准。

## 维护者与项目

本 fork 由 **Zhu Zhe（zhuzhe1983）** 维护，围绕产品级 ESP32 设备开发可由智能体配置、由人操作、嵌入网站的实验台。

- [个人 GitHub：Zhu Zhe / zhuzhe1983](https://github.com/zhuzhe1983)：查看公开项目与贡献。
- [项目 GitHub：esp-online-tester](https://github.com/zhuzhe1983/esp-online-tester)：查看源码、提交 issue 或贡献 PR。
- [上游项目：mikroverk/esp32sim](https://github.com/mikroverk/esp32sim)：本项目基于这一模拟器继续开发，保留上游贡献署名。
