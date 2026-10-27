# Dial 两版 PCB 占位穿壳修复

K130 的问题来自 36×36×1 mm 方形 `UNKNOWN` 占位，放在 z=0：
四角距离中心 25.456 mm，超过后壳约 24 mm 的外半径。K130-V11 使用
34×34×1 mm、z=-3 的方片，四角半径 24.042 mm；它落在安装螺母的高度内，
部分被螺母遮住，并不表示占位尺寸合理。[同角度修复前后](evidence/dial-pcb-fix/before-after.webp)
保留了原版的可见缺陷。

两版分别核对了[原版官方页](https://docs.m5stack.com/en/core/Dial)、
[V1.1 官方页](https://docs.m5stack.com/en/core/Dial_v1.1)、各自的正背侧照片以及
M5_Hardware 固定提交 `e29c139cbc7c80b5ccc5557e5ab8025a7a19a109` 的结构文件。
两条独立 STL 地址实际内容的 SHA-256 都是
`8032668cd4d31b9dc5b7ce7f900eb0cb0cc96863ff32fe4ddfeb36761db44fcc`。
这仅说明发布的塑料结构几何相同，不说明两版 PCB、Stamp-S3 / S3A 或固件可互换。
10 次实际抓取的最终地址、UTC、媒体类型、长度和 hash 见 [sources.json](evidence/dial-pcb-fix/sources.json)。

螺母是官方照片及装配 CAD 中的真实部件，本次保留。所有厂家零件、原始三角形、
装配变换、连接器、贴纸、K130 按钮及编码器语义均保留；没有缩放壳体或增加遮挡物。

## 由空腔约束占位

没有得到真实 PCB 布局，因此仍是中性空白 `pcb-main`，两面标注 `UNKNOWN`。
这次改为**保守圆形显示占位**，并非声称真实 PCB 就是直径 30 mm 的圆板。

| 参数 | 取值与依据 |
| --- | --- |
| 占位厚度 | 保留原有 1 mm 估算，不当作官方板厚 |
| 上方支撑件底面 | 厂家 CAD 的 z=-3.160004 mm |
| 占位中心 | z=-4.16 mm；板顶距支撑件约 0.50 mm |
| 完整板厚范围 | z=-4.66…-3.66 mm |
| 该厚度内最靠近中心的厂家三角形 | 半径 15.678609 mm，来自后壳；不依赖螺母遮挡 |
| 占位半径 | 留至少 0.5 mm 径向余量，再向下取到 0.5 mm 刻度，得到 15 mm |
| 最小径向净距 | 0.678609 mm |

检查会把每个原始装配三角形裁到完整板厚范围内，再计算所得凸多边形到中心的最小
XY 距离，包括多边形内部，避免只检查顶点而漏掉斜面相交。数值和两版旧、新配置见
[geometry.json](evidence/dial-pcb-fix/geometry.json)；回归测试直接读取当前模型与导出几何。
真实板形、板厚、孔位、装配高度、元件和走线仍未知，不能把这个占位用于 PCB 制造。

通用渲染器增加 `geometry.shape: "disc"`，旧矩形占位保持原行为。
圆片、文字与外壳材质独立；透明模式仍可看到内部占位，通过原厂开口看到内部也不当成穿壳。
圆片的语义 ID 保持不变，取得真实 PCB 数据后可以替换。

## 验证与复现

[浏览器检查图](evidence/dial-pcb-fix/views.webp) 包含每版原厂色正面、背面、侧面，
金色壳体、半透明紫壳及独立 `UNKNOWN`。每版另扫描 36 个视角：三个俯仰角、
每 30° 一个方位角。可见/隐藏占位的差异没有落到不透明装配的逐行外轮廓之外；
源 CAD 开口内仍可见占位的像素差异如实保留于 [browser.json](evidence/dial-pcb-fix/browser.json)。
这个轮廓检查补充几何间隙检查，不能替代照片标定或实物测量。

```sh
node --test tools/platform-dial-model-test.mjs
python tools/rebuild-product-model.py plan m5stack-k130
python tools/rebuild-product-model.py build m5stack-k130 --scratch output/dial-rebuild
python tools/rebuild-product-model.py build m5stack-k130-v11 --scratch output/dial-v11-rebuild
```

Python 环境按 `tools/requirements-model.txt` 安装。两版公开离线重建各验证 7 个资产，
完整 model.json 字节一致。浏览器先用任务自有静态服务器打开
`platform/review-lane-01.html?id=m5stack-k130`，再通过 Playwright CLI wrapper 执行
`run-code --filename tools/fixtures/dial-pcb-browser-review.js`；结束后关闭自有会话。

[检查记录](evidence/dial-pcb-fix/checks.json)：60 项平台测试（含实际 WASM runtime）、
目录验证、10 项重建工具测试及隐私检查通过。没有修改 Rust/板级运行行为。
原有照片指标、尺寸偏差和采集时间保留，旧照片并未重新做 1:1 认证；每版验证文件
通过修订记录区分旧指标与本次占位修复。K130 原验证文件的 model hash 早于按钮/编码器
元数据修改，修订中同时保留原记录值和实际基点值，没有用新时间覆盖历史证据。
