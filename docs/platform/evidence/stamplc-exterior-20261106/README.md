# StamPLC K141 外观修正（2026-11-06）

本次修正 Type-C 的坐标方向、上下两排端子高度、接线腔、四侧接口与贴纸，以及屏幕的显示范围和嵌入深度。保留原厂外壳的 22496 个三角形和毫米坐标，没有拉伸壳体。当前模型仍为 `partial`：已补建可见外观，未取得完整板级装配 CAD，不宣称各角度照片级 100% 复刻。

[六面照片／模型／叠图](six-view-comparison.webp) · [修正后斜视](front-left.png) · [Type-C 近景](usb-close.png) · [官方固件实际画面](workbench-default.png)

## 问题与修正

| 部位 | 原因与现在的处理 |
| --- | --- |
| Type-C | 旧插口只绕 Y 轴旋转，把 8.9 mm 长边沿机身厚度放置，形成竖口。现在长边沿机身 Y、短边沿 Z，朝 +X 开口；增加金属套筒、凹入的舌片和可见触点。口沿在 X=35.7 mm，内部射线进入 3.48 mm 后才命中后壁。尺寸来自照片估算，不冒充具体连接器厂商 CAD。 |
| 上排 11、下排 12 颗螺钉 | 旧螺钉最高约 Z=13.98 mm，却使用 Z=10.4 mm 的端子面板，因而悬在面板前方。现在螺钉头顶为 Z=9.10 mm，槽纹最前点为 Z=9.135 mm，低于面板 1.265 mm；保留图纸的 5 mm 针距及继电器组间距。 |
| 接线口 | 补齐上排蓝色 8 路输入／红色 3 路公共与电源端子，以及下排四组三路继电器端子。每孔有斜口、通道和后壁，正对插口的 23 条射线分别测得约 3.63–3.98 mm 可见深度。隐藏夹线机构没有精确 CAD，未作为已还原内部结构。 |
| 四侧接口 | 补建 DC5521 电源口、橙色 HT3.96-4P PWR485、带键位的 XT30(2+2) PWR-CAN、两枚终端电阻拨动件、GPIO.EXT 2×8 插座和独立 microSD 槽。外露部件与照片位置对应，隐藏长度和接触结构仍为估算。 |
| Grove A／C | 将键槽转到照片所示的上边，略微下移并内嵌口沿；同时反转局部 `pinOrder`，保持世界坐标左到右 GND／5V／信号 1／信号 2 不变。插口继续使用原有连接锚点。 |
| 屏幕 | 原 25.24×14.2 mm 来自“1.14 英寸”标称对角线换算，内嵌后会穿入内侧倒角。按正面照片约 249 px 的窗口与 721 px／72 mm 壳宽，改为约 24.9×14.00625 mm，保持 240:135 比例。中心仍为 [−9.7,16] mm，Z 从 13.8 降到 13.55 mm，位于 Z=14 的口沿后方 0.45 mm。窗口尺寸约 ±0.3 mm，玻璃深度未实测。 |
| 贴纸 | 使用原图像素补齐输入／输出接线标识、左侧总线说明和右侧 GPIO／SD／扩展脚位说明。右贴按原厂 CAD 的 X=35.8／36.0 mm 台阶分成两个承载面，避免文字被边框遮住。开口区域排除照片中的 USB、卡槽和插座，屏幕从不贴静态照片。 |
| PCB 占位 | 主板仍是 `UNKNOWN`。其原来估算的 62×62 mm 空白包络进入下排接线腔，缩为估算的 60×55 mm；没有改动任何已知 PCB CAD。半透明完整装配中仍能读到 [UNKNOWN](unknown.png)。 |

## 来源与可复现性

重新获取[官方文档](https://docs.m5stack.com/en/core/StamPLC)中的 13 张图库照片及尺寸图，内容 SHA-256 均与保留的历史来源一致。当前官方 `master` 下的 StamPLC STL 也与已固定版本相同：`91745b5fc0d2a4bb7ac866271b03bcbd5324fbde2b7e51c220cb66fdfe4470d6`。原始／最终 URL、UTC 时间、媒体类型、字节数与哈希见 [source-refetch.json](source-refetch.json)。旧来源时间没有被覆盖。

[model-plan.json](../../../../web/platform/assets/m5stack-k141/model-plan.json) 保存全部几何尺寸、旋转、来源、照片裁切和透明遮罩。[model.json](../../../../web/platform/assets/m5stack-k141/model.json) 的最终 SHA-256 为 `3e6e0011bc00746965aa7abc29dfe16e86e2de48db1927072f98272910dc1f78`。28 个生成资产与完整模型配置离线重建一致，`modelJsonByteIdentical=true`，见 [rebuild.json](rebuild.json)。原厂几何二进制 SHA-256 仍为 `8eada24face950933c7ee7d2d649a1bcc2caf48c74402dbde4e583ad48565e31`。

照片版权属于 M5Stack，未另行宣称开放照片许可；CAD 沿用厂家原始许可。新贴图仅作产品标识与参考，没有生成或猜写原图无法辨认的文字。

## 验证结果与边界

- [浏览器几何回执](browser-check.json)：实际通用渲染器加载全部固定哈希资产；23 颗螺钉可见且内嵌；23 个接线孔及 USB 具有插入深度；完整外形为 **72.015×80×33.415 mm**，相对官方 72×80×33.4 mm 的差值在 0.1 mm 检查容差内。0.015 mm 来自薄贴纸／铜套防闪烁偏移，不是物理量测。
- [贴纸承载检查](paper-support.json)：五个侧贴面合计 109987 个有效 alpha 像素，全部落在对应原厂面上；无穿到前方的壳体面，无超过 0.05 mm 的悬空像素。检查包含原色、[金色](gold.png)、[半透明紫色](purple.png)与斜视；改色不改变贴纸、连接器、屏幕和键帽材质。
- [屏幕检查](screen-check.json)：正视及左右斜视，每个角度 132 个边缘像素中心全部可见。[彩色像素边框](pixel-border-front.png)仅用于几何检查。另用固定官方镜像实际启动至 `Dashboard onResume`，取得真实 240×135 framebuffer，验证 3D、旋转与 2D，见 [workbench-check.json](workbench-check.json)。没有进行实体硬件刷写或扩大固件兼容结论。
- [分享检查](embed-check.json)：独立 mini 和 iframe 均恢复 K141，模型状态为 ready；该项关闭自动运行，不把预览当作固件执行。
- 平台配置、运行时、输入、视图、知识五组测试通过；217 个产品、10 个配件、172 个固件及 118 个 watchlist 条目通过目录校验。`git diff --check` 和证据隐私检查通过，并人工检查新增文本与 JSON。

[六面比较](photo-comparison.json)保留照片、渲染、透明叠图及相机。正／背／右／左／上／下的外轮廓 IoU 约为 **0.9355、0.9222、0.9219、0.8698、0.9282、0.8448**。该指标受白色壳体边界、照片阴影、轻微透视和 CAD／实物版本差异影响，只辅助观察；并非整机 1:1 验收通过。尝试从侧面接缝估计约 1.8° 相机倾斜未改善一致性，负结果保留在 [photo-alignment-negative.json](photo-alignment-negative.json)，最终对照采用明确的正交视角。

仍未核实连接器隐藏深度、完整 PCB 布局、螺钉内部螺纹及光学材料参数；没有实物尺寸测量和相机标定。可见端子及 USB 的照片估算约 ±0.4 mm，不能据此用于制造或精密插合。

## 复查命令

从仓库根目录运行，Python 环境安装 `tools/requirements-model.txt`：

```sh
python tools/rebuild-product-model.py plan m5stack-k141
python tools/rebuild-product-model.py build m5stack-k141 --scratch <fresh-scratch-directory>
python docs/platform/evidence/stamplc-exterior-20261106/check-paper-support.py
node --test tools/platform-config-test.mjs tools/platform-runtime-test.mjs tools/platform-input-test.mjs tools/platform-view-test.mjs tools/platform-knowledge-test.mjs
python tools/validate-platform-catalog.py
node tools/check-evidence-privacy.mjs
```

按仓库浏览器生命周期约定，用 Playwright 包装脚本打开本地平台页，再执行：

```sh
playwright_cli.sh -s <task-session> run-code --filename tools/fixtures/stamplc-exterior-browser-check.js
playwright_cli.sh -s <task-session> run-code --filename tools/fixtures/stamplc-screen-browser-check.js
playwright_cli.sh -s <task-session> run-code --filename tools/fixtures/stamplc-exterior-embed-check.js
python docs/platform/evidence/stamplc-exterior-20261106/compare-photos.py
playwright_cli.sh -s <task-session> close
```

实际固件检查使用 `tools/fixtures/stamplc-screen-workbench-check.js`；先按脚本要求选择 SHA-256 匹配的本地官方镜像，二进制不进仓库。历史修改前截图及参数保留为 `before-*.png` 与 [before.json](before.json)，不会被重建命令覆盖。
