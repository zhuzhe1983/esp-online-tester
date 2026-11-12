# 官方 v0.5 静音图标的显示差异核查

最终 WASM `8d8ccce…` 中，两次 GT911 触摸能切换并恢复静音状态，但同一 10×10 区域的黑像素是 `63→0→49`。这不是首末帧逐像素一致；不能只据此宣称显示正确。对实际输出进一步核查后，完整图标区域的 429 个差异均有确定来源。

[固定官方素材与 M5GFX 来源](evidence/papers3-display/sources.json)表明：背景 `img_bg.png` 中的图标与后续绘制的 `img_icon_mute_on.png` 不是同一组像素。应用源码 `AppBuzzer::onRunning` 在 `(298,380)` 绘制独立 88×85 图标；M5GFX `Panel_EPD::_draw_pixels` 的 quality 路径量化为 16 级，fastest 路径则用 4×4 Bayer 抖动量化成黑白。数字面板当前仅保留最后一次有效黑/白驱动，不模拟 quality 波形的光学积分。

离线核查器直接解码固定官方 PNG，读取固定 M5GFX 源文件中的 Bayer 表和 LUT，计算该数字端点应有的像素，再与实际页面 worker 输出的原始 RGBA 比较。没有读取客体应用 framebuffer，也没有改动运行中的显示结果。结果见[精简收据](evidence/papers3-display/icon-audit.json)：

| 实际画面 | 对应官方素材与模式 | 比较像素 | 不一致 |
| --- | --- | ---: | ---: |
| 首帧 | `img_bg.png`，quality 数字端点 | 7,480 | 0 |
| 第一次触摸后 | `img_icon_mute_off.png`，fastest | 7,480 | 0 |
| 第二次触摸后 | `img_icon_mute_on.png`，fastest | 7,480 | 0 |

背景图与独立静音图标的原始灰度有 1,148 个像素不同。两种素材经过对应模式后，实际首末帧有 429 个差异（284 黑→白、145 白→黑），全部由上述比对解释。因此这一区域的斜线、声波弧线和边缘没有额外漏擦、残留或行偏移。不能把差异统称为实物残影，也不应为了消除差异把不同素材强行画成同样的图案。

复现先运行 `tools/probe-papers3-factory.mjs --output <frames>` 保留三张实际 `.rgba`，再按来源收据下载固定 PNG 和 `Panel_EPD.cpp`：

```sh
node tools/check-papers3-factory-display.mjs \
  --assets <fixed-factory-main/assets> \
  --panel-source <fixed-Panel_EPD.cpp> \
  --frames <frames>
```

核查器先校验全部来源 SHA，再断言三个完整图标区域精确匹配。PNG、原始画面和完整串口留在本地；仓库只保存摘要、数字结果和复现脚本。本结论限于官方 v0.5 的上述图标与数字黑白路径。quality 的 16 灰阶、其他窗口/波形、实物残影和 EDCBook 阅读/翻页均未据此通过；没有为此改动共享 ET047TC1 模型。
