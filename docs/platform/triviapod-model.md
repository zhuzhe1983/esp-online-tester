# TriviaPOD：独立产品与官方外观模型

在左侧选择 **M5Stack → 其他主控 → TriviaPOD TP-01-A · 白色**，即可查看独立产品资料与 3D 模型。它不再放在 StopWatch 的衍生模型菜单中，场景 ID 保持 `m5stack-triviapod`。`firmwareTargetId: m5stack-stopwatch` 提供共用固件目录；运行采用显式的 StopWatch 兼容模拟配置，保留 32 MB Flash、8 MB PSRAM，并支持真实帧缓冲、触摸与 A/B 按键。共享固件仍按试运行显示。

目录列出官方已确认的 **USB-C 充电接口（DC 5V 输入）**。USB 数据、Host 能力、内部路由与引脚尚未核实；接口名称不代表可连接某种 USB 配件或已实现电气行为。

TriviaPOD 有独立的官方模型，不能只给 StopWatch 换贴图。[官方比较表](https://docs.m5stack.com/en/core/TriviaPOD)确认其 Flash 为 32 MB，取消 Grove、后部扩展总线与背部磁吸。官网[旋转展示的 glTF](https://triviapod.com/assets/models/TriviaPod3D.gltf.js)包含白色封闭后盖、浮雕、扬声器孔、三颗按键、黑色屏幕和独立 USB-C。这里保留全部 6 组源网格、81,532 个三角形，未拼入 StopWatch 的连接器或后部结构。

## 来源与变换

- 原网站文件 SHA-256：`fd222026f75612a56fba1eaa3dfe81a9c61c79133f0026c54effaa4ed8f889b3`。只解析 `window.GLTF_B64` 的字符串数据，不执行下载的 JavaScript。
- 解码 glTF SHA-256：`d2870244aa86264b14f44932bb47d78a7f0b0b5ecf5c3e2844b28a7dc672d07a`，原字节保存在 [source.gltf.gz](../../web/platform/assets/m5stack-triviapod/source.gltf.gz)，gzip 时间固定为 0。
- 先应用原始节点变换（包括独立 USB-C），再整体转为 `(-Y, -Z, X - 7.75)`。保留源顶点、三角形及法线；白色恒等顶点色和没有贴图的 UV 不进入渲染二进制。没有重新缩放零件或猜测装配偏移。
- **单位仅为展示推定。** glTF 通常以米为单位，但本资产外径数值约为 51.947，官网展示器按包围盒重新归一化。平台保留源数值、按毫米显示，使其与 StopWatch 外形量级一致。官方 TriviaPOD 实物尺寸和重量仍是 WIP，不能把这些坐标称为已核验的产品尺寸或生产 CAD。
- 原材质定义保留在每组零件的 `sourceMaterial`。白壳显示色取 `#d5d5d5`，防止平台强光淹没浮雕；这不是测得的塑料反射率。黑色屏幕采用不透光的标准材质，不添加游戏截图或假帧缓冲。清漆和透射没有光学还原。
- 未找到板级 CAD、布局或内部装配资料。`pcb-main` 是估算的 44×44×1 mm 中性空白占位，厚度、板形、位置、颜色均未核实。透明模式从背面可见 `UNKNOWN`；不计入内部精度验收。

来源时间、URL、字节数与哈希在 [source-receipts.json](../../web/platform/assets/m5stack-triviapod/source-receipts.json)，硬件及固件调查在[知识记录](../../web/platform/knowledge/m5stack-triviapod.json)。官网模型没有查到独立开放许可；官方照片保留 M5Stack 权利。两者都不套用项目或 M5_Hardware 的 MIT 许可，见 [SOURCE-RIGHTS.txt](../../web/platform/assets/m5stack-triviapod/SOURCE-RIGHTS.txt)。网页只加载仓库内的渲染资产，不执行来源脚本或调用第三方模型 CDN。

## 复现与验证

这是官网 glTF 的专用导入器，不走 STL/STEP 的通用重建计划。安装 NumPy 后，从仓库根目录离线重建：

```sh
python tools/build-triviapod-model.py --output /tmp/triviapod-rebuilt
cmp web/platform/assets/m5stack-triviapod/model.json /tmp/triviapod-rebuilt/model.json
cmp web/platform/assets/m5stack-triviapod/geometry.bin /tmp/triviapod-rebuilt/geometry.bin
```

两份输出逐字节一致。模型对照页是 [triviapod-model-review.html](../../tools/fixtures/triviapod-model-review.html)，通过仓库静态服务器打开即可。`review.show('front'|'backOblique'|'left'|'right'|'bottom')` 切换方向；`review.appearance({shell:{color:'#9d74cf',opacity:.24,finish:'plastic'}})` 查看透明壳。对照页使用固定的展示光源，外壳经过与正式页面相同的默认 plastic 外观处理。

已与保留的官方照片 05–08 核对后壳结构、按键方向和 USB-C 开口，并检查白色、金色、透明紫色。照片机位和光照没有标定，实物尺寸未知，所以这只是可见结构和方向对照，不宣称通过尺寸 1:1 或轮廓重合率验收。官网网格的分面与浅浮雕仍可见，没有人为补猜隐藏细节。

![白色后盖](evidence/triviapod-20261105/final-backOblique.png)
![USB-C 底侧](evidence/triviapod-20261105/final-bottom.png)
![透明壳的未知内部占位](evidence/triviapod-20261105/final-transparent.png)

首次导入时曾采用 StopWatch 衍生预览，工作台、独立 mini 和 iframe 的旧版本检查保留在 [verification.json](evidence/triviapod-20261105/verification.json)。其中的菜单切换、StopWatch V0.5 启动与 468×466 引导页结论仅适用于该旧版本，不代表 TriviaPOD 固件运行。原始截图与哈希不改写为新版本证据。

当前独立条目的目录/知识校验、可检索与场景往返检查，以及模型重建哈希见 [standalone-update.json](evidence/triviapod-20261105/standalone-update.json)。本次只更新目录关系、USB-C 资料与模型限制文字；原网格、材质、图片和源文件均未变化。

TriviaPOD 已显式设置 `runtime.board: stopwatch`、`status: partial`，保留自身 32 MB Flash 和 8 MB PSRAM。显示、触摸、电源与 A/B 引脚采用 StopWatch 兼容模拟配置；这是模型约定，官方 TriviaPOD 原理图和内部引脚仍未独立核验。物理 Grove/后部接口及 GPS 测试通道不随 board 名继承，存储与分享仍以 TriviaPOD 标识隔离。

固件库按[固件目标规则](firmware-targets.md)列出 StopWatch UserDemo、UIFlow 和小智候选；首次选择以 UserDemo 开始试运行。没有将这些条目登记为 TriviaPOD 出厂软件。官方问答软件由 Etermax 专有开发，当前只确认机内 Wi-Fi 更新入口，未找到公开固定出厂镜像。模拟运行验收另见 [2026-11-06 记录](evidence/triviapod-runtime-20261106/README.md)，未做实机刷写。
