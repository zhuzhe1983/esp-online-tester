# 网站访问统计

正式 GitHub Pages 网站使用 Google Analytics 4，媒体资源为 **ESP Online Tester**，网站数据流为 **ESP Online Tester · GitHub Pages**，衡量 ID 为 `G-425HV3H2P0`。报告时区为中国时间（UTC+8），币种为人民币；目标为网站流量与用户互动。

这是应网站维护者要求增加的可选第三方网站统计。`web/platform/analytics.js` 异步加载 Google 官方 `gtag.js`；模拟器启动和固件执行不依赖其成功加载。未使用 Measurement Protocol 密钥，没有在仓库内保存后台凭据。

## 范围

仅 `https://zhuzhe1983.github.io/esp-online-tester/` 下的工作台、独立播放器、智能体指南、嵌入示例、旧模拟器介绍和运行页启用。首页重定向只计入最终工作台页面；旧模拟器链接跳转到运行页时也只计最终页面。本地开发、其他部署域名、模型审核页以及嵌入在其他页面内的播放器不加载统计脚本。保留 buildless 及 GitHub Pages 子路径部署。

每个页面加载一次 `page_view`；GA4 提供其标准访问者、会话与互动统计。后台已关闭增强型衡量，代码关闭自动 page view，避免 History API 更新产生额外计数或自动读取页面表单、链接和下载地址。切换设备、运行固件、按键和传感器输入没有新增埋点。

上报的 `page_location` 只包含固定的正式页面路径，移除所有查询参数和 fragment；页面标题使用固定名称，来源只保留协议和域名。固件 URL、场景/录制、DIY 内容、串口输出、设备存档、Wi-Fi 凭据均不传给统计代码。Google 的常规网络和浏览器统计仍按其服务处理；这不是匿名化或零数据收集声明。

关闭 Google Signals 和广告个性化；统计 Cookie 使用 `eot` 前缀，并限于本站 `/esp-online-tester/` 路径。浏览器开启 DNT、Global Privacy Control 或该衡量 ID 的 Google 禁用标记时不加载。广告拦截器或网络不可达时，统计可能缺失，网站仍可使用。

## 验证与维护

```sh
node --test tools/platform-analytics-test.mjs
```

检查正式站点白名单、重定向只计一次、去除查询与片段、来源去除认证信息和路径、固定标题、禁用广告功能、重复初始化、隐私选择、iframe 和所有公共入口。平台回归使用 `node --test tools/platform-*-test.mjs`。上线后访问真实网页，并在该资源的实时报告确认数据；后台的标准汇总报告可能延迟。

如果更换域名、媒体资源或页面路径，更新本地模块及相应测试，并同步后台数据流网址。不要重新开启增强型衡量而不复核 URL 和表单数据边界。

官方依据：

- [GA4 页面浏览量配置](https://developers.google.com/analytics/devguides/collection/ga4/views)：`send_page_view` 与手动 `page_view`，包括增强型衡量历史事件独立触发的行为。
- [Google tag 配置参数](https://developers.google.com/analytics/devguides/collection/ga4/reference/config)：`page_location`、`page_referrer`、广告功能开关与 Cookie 配置。
- [增强型衡量事件](https://support.google.com/analytics/answer/9216061)：自动互动测量的内容与范围。

文档核对日期：2026-11-06。
