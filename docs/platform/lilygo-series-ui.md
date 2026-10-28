# LILYGO 系列与具体型号选择

厂商选择保留原有行为。产品声明 `family: {id, name}` 时，工作台增加系列筛选；“全部系列”按系列分组列出具体型号。系列只用于筛选，不创建可配置或可运行的产品。未声明系列的 M5Stack 目录继续使用原来的型号列表。

切换到 LILYGO 默认显示全部系列。手动选择“全部系列”后，切换具体型号不会收窄筛选；修改筛选且当前型号仍在范围内时，不会重置固件。搜索可以跨厂商和系列，匹配系列名称、具体型号及 Spark 产品 ID；选择搜索结果、导入新的型号或打开分享时，会定位该型号所属系列。分享继续保存本地稳定产品 ID。

没有板级适配的型号标注“尚未适配”，不能加载或启动模拟固件。无模型条目显示“资料与配置”；未知屏幕显示“屏幕资料待核对”，不会绘制假定屏幕。硬件版本为 `unknown` 或 `unspecified` 时不显示为已知版本。新增系列筛选与其他配置控件一同在加载期间禁用。

## 验证

使用真实 Spark 导入目录进行无头 Chromium 检查：127 个 LILYGO 产品、21 个系列组、T-Deck 6 个具体型号。M5Stack 的 84 个型号保留。验证涵盖筛选、跨系列搜索、工作台与嵌入分享、未知屏幕、未适配运行限制、保留固件选择及加载忙态恢复。没有页面异常。

复现时先准备 WASM、ROM 和完整目录，再通过本地 HTTP 服务打开工作台，并使用 Playwright skill wrapper：

```sh
mkdir -p output
~/.codex/skills/playwright/scripts/playwright_cli.sh open "$WORKBENCH_URL"
~/.codex/skills/playwright/scripts/playwright_cli.sh run-code --filename tools/platform-lilygo-series-browser-check.js
~/.codex/skills/playwright/scripts/playwright_cli.sh close
~/.codex/skills/playwright/scripts/playwright_cli.sh list
```

浏览器脚本不下载固件；忙态检查拦截一个明确的测试 manifest 请求并返回 404。该检查证明选择、分享与状态限制生效，不证明新增型号已具备固件执行能力。

输入哈希、测试结果及截图哈希见 [验证记录](evidence/lilygo-series-ui-2026-10-28.json)。截图保存在忽略的 `output/` 目录，不纳入源码；记录不包含机器用户名、浏览器会话标识或完整日志。
