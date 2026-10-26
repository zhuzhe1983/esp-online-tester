# Unit GPS v1.1（U032-V11）模型与知识

这次刷新现有 `m5stack-gps-u032-v11`，没有创建重复商品。它采用 AT6668；旧 `m5stack-gps-u032` 的 AT6558、9600 baud 配置与示意模型保持独立。用户手里的硬件版本仍须核对 SKU，不根据网页版本推断。

外壳直接采用 [M5Stack 官方 STL](https://github.com/m5stack/M5_Hardware/blob/e29c139cbc7c80b5ccc5557e5ab8025a7a19a109/Products/U032_V11_Unit_GPS_v1_1/Structures/Unit_GPS_v1.1.stl)，固定提交 `e29c139cbc7c80b5ccc5557e5ab8025a7a19a109`，SHA-256 `cfb75ed7e65c36ab005d88beb4a81c293a2f07edffd2a9dd13ec14ce8b1a9b0b`。厂家许可 MIT 随原始 STL 保存。官方照片与衍生标签仍属 M5Stack；未核实独立开放图片许可，不套用项目 MIT。

知识记录 [m5stack-gps-u032-v11.json](../../web/platform/knowledge/m5stack-gps-u032-v11.json) 保留官网、尺寸图、照片、数据手册、原理图和许可的来源地址、实际 UTC 抓取时间、字节数和 SHA-256。未抓取的 MAX2659/接收机协议手册仅登记链接。官网简介采用摘要，没有镜像整页正文。

## 装配与未知项

源 STL 有 4 个连通分量：0、3 已组成同一外壳；1、2 是横向平移的重复件。导出保留 0、3，仅统一平移 `[40,0,-1.2]` mm；没有缩放、非均匀变形或重新猜装配位置。坐标为 X 左右、Y 长边、Z 厚度，正面 +Z，Grove 朝 -Y。最终 3,580 个三角形，几何二进制 257,760 字节。

| 部件 | 依据 | 边界 |
| --- | --- | --- |
| `shell-front`、`shell-back` | 同 SKU 厂家 CAD | 保留原始三角形位置 |
| `front-label`、`back-label` | 官方正反照片固定裁切 | 无生成图片、无镜头校正；照片亮度不是材料数据 |
| `grove-insert` | 10 × 5.2 mm 开口图纸及插座照片 | 蓝色壳、U 形开口与可见四触点近似；隐藏尺寸未验证 |
| `rear-fastener` | 背照与 CAD 孔中心 | 螺钉头和内六角近似 |
| `pcb-main` | 显式占位 | **UNKNOWN**，20 × 28 × 1 mm、位置 `[0,6,0]` 均为估算；无芯片、走线、丝印或焊盘 |

官方 v1.1 页链接的原理图，内部标题却为 `Revised V1.0`、`10/16/2018`、`GP-02`。这项冲突已登记为未知，不能据文件名将它当作 AT6668 板级布局。裸 GNSS 模块手册也不能证明完整 Unit PCB 的轮廓、电平容限或元件位置。

模型返回 `{group, parts, connector, ready, dispose}`。同步 Grove 锚点是 `[0,-24.15,1.4]` mm，外法线 `[0,-1,0]`；0.15 mm 是显示插头的外移间隙，不是制造公差。异步加载不会改变锚点。`accessory-view.js` 只为 U032-V11 选择此模型，其他配件保持既有路径。失败时保留 24 × 48 × 8 mm 占位和接口锚点；移除时调用 `dispose`，取消下载并释放资源。

白色壳的显示反射色适配现有强照明；照片标签使用不受光照影响的材质，以免重复曝光。它们是查看器外观参数，并非实际树脂配方或反射率。背面浅压印 M5、插座细节和螺钉外观不能按照片像素宣称完全一致。

## 核验结果

完整数据见 [verification.json](evidence/gps-model/verification.json)。独立目标来自 [官方尺寸图](https://m5stack.oss-cn-shenzhen.aliyuncs.com/resource/docs/products/unit/Unit-GPS%20v1.1/module_size.jpg)，容差预置为 0.05 mm：

| 项目 | 官方目标（mm） | 模型实测（mm） | 结论 |
| --- | ---: | ---: | --- |
| 宽、长、厚 | 24、48、8 | 24、48、8 | 通过 |
| 两安装孔孔径 | 5 | 4.99989、4.99989 | 通过 |
| 孔中心间距 | 16 | 16.00000 | 通过 |
| 孔中心距底边 | 12 | 12.00000、11.99999 | 通过 |
| 导出与刚体变换后原 CAD 最大偏差 | 0 | <0.00000008 | 通过 |

[正面](evidence/gps-model/front-comparison.webp)与[背面](evidence/gps-model/back-comparison.webp)按固定正交相机对照，外轮廓 IoU 约 0.961 / 0.954，超过预置 0.95。掩膜使用 RGB 最低通道 <248 后的最大填充外轮廓；孔、标签及内层不参与轮廓评分。公差和掩膜方法见脚本，不能以高轮廓分数替代细节审查。

[插座斜视](evidence/gps-model/oblique-comparison.webp)与[背面斜视](evidence/gps-model/rear-comparison.webp)保留实际差异；照片透视与镜头参数未知，目前仅做定性对照，没有声称精确相机对齐通过。正反标签来自对应验证照片，标签相似不构成独立证据。[UNKNOWN 主板检查图](evidence/gps-model/pcb-unknown.webp)可见空白占位标识。

**通过的是厂家外壳的尺寸和正反外轮廓核验。** 内部结构、制造精度、材料、真实卫星信号及实物测量均未验证。模型不扩展模拟能力：GPS 仍是向逻辑 UART1 排队输入 NMEA，不模拟 GPIO 路由、波特率、电源、射频、搜星或模块控制命令。

## 重生成与回归

Python 依赖固定在 `tools/requirements-model.txt`；网页本身仍免构建、纯静态。

```sh
python3 -m venv .venv-model
.venv-model/bin/pip install -r tools/requirements-model.txt
.venv-model/bin/python tools/build-gps-model.py --fetch
python3 -m http.server 8041 --bind 127.0.0.1
```

在独立的 headless Playwright 会话打开 `http://127.0.0.1:8041/web/platform/gps-model-review.html`。等待 `document.body.dataset.ready === 'true'`，依次调用 `gpsModelReview.showView('front'|'back'|'oblique'|'rear')`。画布内部尺寸是 800 × 800；截图时将画布固定在整数坐标 `(0,0)`，按 800 × 800 clip 保存，避免 DOM 元素的小数坐标被截成 801 像素。输出路径 `output/playwright/gps-{view}-render.png`，检查 UNKNOWN 后另存 `gps-unknown-render.png`。照片、尺寸目标、相机参数和裁切坐标均固定在脚本与资产 JSON 中；不能通过拉伸模型调整吻合度。

```sh
.venv-model/bin/python tools/verify-gps-model.py
node tools/platform-gps-model-test.mjs
node tools/platform-scene-test.mjs
node tools/platform-config-test.mjs
node tools/check-evidence-privacy.mjs
```

以上 Node 测试检查来源哈希、SKU 隔离、知识未知项、毫米尺寸、同步/异步锚点一致、线缆方向、404 占位和取消/释放；不冒称真实固件或硬件验收。照片与核验报告一起保留。浏览器完成后通过同一 Playwright wrapper 关闭本任务会话并确认退出。
