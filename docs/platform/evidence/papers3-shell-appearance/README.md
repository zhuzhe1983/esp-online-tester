# PaperS3 3D 原色初始化

基线 `44a4cb59` 可稳定复现外壳变黑：`createManufacturerModel` 已创建灰色主壳，仍在等待 `paper-labels.png`；此时 `setAppearance(null)` 用 StopWatch 黑色默认值改写主壳，随后 `model.ready` 又把黑色保存为 `originalAppearance`。模型 JSON 的 `#b8bcba` 没有变化。

修复只调整 `scene-view.js` 的外观时序：等待模型完成，再记录原色并应用最后一次请求。加载期间的局部偏好在厂家默认值可用后重新解析；重置仍恢复每个壳体各自颜色，原有材质与光泽规则保持不变。没有修改模型资产或 PaperMono 模型。

[收据](receipt.json)保留真实 headless WebGL 负例、修复后的材质状态、加载期间的自定义/透明度/重置，以及工作台“原色→金色→恢复原色”结果。两张背面截图目视确认黑壳恢复灰色。六组平台检查通过，新增加载时序测试在基线失败、修复后通过。既定 runtime 检查复用了本机现有 WASM；其含有其他未提交工作，不能作为发布固件运行证据。

复现测试：`node tools/platform-view-test.mjs`。真实浏览器通过 Playwright wrapper 启动，拦住标签请求直至完成外观调用，再放行并检查材质与截图。原始截图和本地浏览器脚本保存在忽略目录 `output/playwright/papers3-shell/`；此任务的浏览器与静态服务已关闭。
