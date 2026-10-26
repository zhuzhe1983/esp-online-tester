# StopWatch 模型：来源与校对结果

当前 StopWatch 使用 M5Stack 公布的 STL 外壳和按钮，替换了原先的圆角方盒示意图。可在 [照片对照页](../../web/platform/model-review.html) 切换正、背、右侧、底部；网站地址为 `/platform/model-review.html`。

来源为 [官方产品页](https://docs.m5stack.com/en/core/StopWatch)、[固定提交的结构文件](https://github.com/m5stack/M5_Hardware/blob/e29c139cbc7c80b5ccc5557e5ab8025a7a19a109/Products/C152_StopWatch/Structures/StopWatch.stl)、[官方尺寸图](https://m5stack-doc.oss-cn-shenzhen.aliyuncs.com/1242/C152-StopWatch-model-size.pdf) 与产品照片 06/07/08/09。逐文件 URL、UTC 抓取时间和 SHA-256 见 [知识记录](../../web/platform/knowledge/m5stack-stopwatch.json)；转换和纹理处理见 [模型记录](../../web/platform/assets/m5stack-stopwatch/model.json)。

STL 有 8 个连通组件：0/2/5/7 是已经装配的外壳和三键，其他 4 个是移到旁边的重复件。选择原装配，作 `(-z,y,x-7.75)` 刚体坐标转换，保持毫米，不非均匀缩放。玻璃层、连接器内芯、螺丝、插座和内部遮挡体是补建部分，不能当作厂商电子装配 CAD。电源按钮仅展示外观，未冒充已实现的电源模拟。

板级公开资料的核查见 [PCB 与爆炸图方案](exploded-view-plan.md)。没有板级 CAD 时使用稳定 ID `pcb-main` 的空白 PCB，占位板正背标注 `UNKNOWN`，不绘制内部芯片或走线。圆板直径 44 mm、厚度 1 mm、中心 `[0,0,0]` 均为壳内展示估算，未作为内部 1:1 尺寸验收。外露 USB/Grove 端子仍依据外部照片摆放，不暗示已知它们在主板上的真实焊接方式。

查看器提供原色、黑透、紫透和金色外壳，以及自定义颜色/不透明度。它们是展示材质，原色用于下述照片验收。透明设置不改变屏幕 framebuffer、按键材质或接口事实；隐藏在壳体背后的按钮仍不会被点击穿透。占位 PCB 的正背标识位于板面，正常装配时可能被屏幕和背贴遮挡，后续爆炸查看可单独检查。

背贴由用户提供的 [官方高清照片 07](https://m5stack-doc.oss-cn-shenzhen.aliyuncs.com/1242/C152_stopwatch_mainpictures_07.webp) 按像素轮廓裁切，记录了原图 hash、裁切框、mask 顶点和 UV 映射。它显示 v1.0.1；知识库保留 v1.0 与 v1.0.1 电源触点差异，不以贴图确定实物版本。厂家 STL 附仓库 MIT 许可；官网照片单独注明 M5Stack 版权，未核实独立开放许可，不按本项目 MIT 重新授权。

## 验证结果

| 检查 | 结果 |
| --- | --- |
| 源 CAD → 浏览器顶点最大误差 | 0.00000024 mm；容差 0.05 mm |
| 外壳 X/Y 外径 | 51.94694 / 51.94579 mm；图纸 φ51.95 mm |
| 厚度 | CAD 外壳 15 mm，按图纸补玻璃 0.5 mm；这不是独立厚度测量 |
| 正面轮廓 IoU | 98.69% |
| 背面轮廓 IoU | 99.14% |
| 右侧轮廓 IoU | 95.11% |
| 底部轮廓 IoU | 98.34% |

照片均按固定圆心/外轮廓定尺度，以正交相机对照；不改变模型比例。轮廓取 800×800 图片的 min RGB <205 最大连通外轮廓，预设阈值 95%。完整 [验证记录](evidence/stopwatch-model/verification.json) 绑定模型、几何、渲染器、相机代码、检查脚本和参考/渲染图片的 hash。

第一次侧视为 92.27%，原因是玻璃只有正面片而缺实体厚度，修正为 0.5 mm 实体后达到 95.11%。保留 [初次失败记录](evidence/stopwatch-model/initial-verification.json)。对照图：[正面](evidence/stopwatch-model/front-comparison.webp)、[背面](evidence/stopwatch-model/back-comparison.webp)、[右侧](evidence/stopwatch-model/right-comparison.webp)、[底部](evidence/stopwatch-model/bottom-comparison.webp)。

轮廓通过只说明可见外形接近，不证明全部部件或制造精度。背贴纹理和背面参考来自同一照片，因此背贴本身不能算独立验证。照片镜头误差、表面材质、连接器内芯和屏幕光学边界仍有近似；未做实物卡尺测量。屏幕保持真实 framebuffer，不使用宣传图画面。

## 复现

从仓库根目录执行，需 Python 3.11+、curl、Node 和浏览器。模型工具使用独立 venv，不改变网页的免构建部署。

```sh
python3 -m venv .venv-model
.venv-model/bin/pip install -r tools/requirements-model.txt
.venv-model/bin/python tools/build-stopwatch-model.py --fetch
```

下载仅包括公开 CAD 和指定产品照片，均先核对固定 SHA-256；当前文件重建完全同 hash。`--sources` 可指定已下载来源目录，`--output` 可指定隔离输出目录。厂商许可随输出复制。新的下载回执在来源目录的 `model-download-receipts.json`，不会冒充原先的资料抓取时间。

启动静态服务后，用仓库要求的 Playwright wrapper 无头打开 `/platform/model-review.html`，等待 `body[data-ready=true]`。调用 `window.modelReview.showView('front'|'back'|'right'|'bottom')`；将 `#render` 设置为 800×800 白底，分别保存为 `output/playwright/stopwatch-{view}-render.png`。只关闭本任务的浏览器 session。

```sh
.venv-model/bin/python tools/verify-stopwatch-model.py
node --test tools/platform-model-test.mjs
```

验证脚本在尺寸或任一照片缺失/未通过时返回非零；`--allow-unpassed` 仅用于明确保留诊断报告。模型检查还验证网页渲染代码和相机代码是否仍对应现有报告。修改模型或相机后需要重新截图和验证。
