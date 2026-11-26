# M5Stack StackChan · K151

模型现包含官方十件打印结构、CoreS3 前壳与传感器支撑件，以及独立的端子、双舵机外壳、五块 PCB 层、RGB 封装和光窗、电池、底垫、螺丝及照片标签。整体状态仍为 **partial**：内部板型、电极铜箔、芯片位置和线束没有公开布局资料，不能从原理图推造。舵机端耳的矩形包络与支架仍有约 0.55 mm 名义重叠，实际圆角、槽位和安装余量尚未核实；该负结果保留在 [内部空间检查](../../../web/platform/assets/m5stack-k151/internal-fit-review.json)。

CoreS3 前部复用 [前部装配契约](../../../web/platform/assets/m5stack-k151/cores3-front-assembly.json)，只接入前壳、传感器支撑、玻璃和光学窗口。主板、端子与底座由 StackChan 自己保留。LCD 中心为 `(0,0,38.2532)` mm，显示面积为 40.64×30.48 mm；摄像头、麦克风和 LTR 窗口位置来自官方 CoreS3 DXF。屏幕只显示运行时 framebuffer。

装配全部使用毫米和刚体变换，没有拉伸原 CAD。MainBody 后平面到侧孔的 16.9/8/8 mm 与官方尺寸图相符；CoreS3 后接合面 `z=0.25` 对接 MainBody 的 `z=22.6532`。原壳与前部联合深度为 61.5 mm。底座平移 `Z+12.75,Y−1.6`，来自尺寸图侧视地标；支架平移 `Z+12.75`，ServoCover 下移 10 mm，使下口与支架开口配合。底座地标和内部插合仍是重建估值，不是实物装配测量。

| 独立尺寸依据 | 目标 | 导出测量 | 范围 |
| --- | ---: | ---: | --- |
| K151 官方尺寸图：宽 | 54 mm | 54.0018 mm | MainBody 与 CoreS3 前壳 |
| K151 官方尺寸图：高 | 70.5 mm | 70.5000 mm | 顶部至底座 |
| K151 官方尺寸图：头部深度 | 61.5 mm | 61.5000 mm | 不含玻璃 0.05 mm 展示偏置 |
| CoreS3 DXF：玻璃 | 50.8×50.8 mm，R3.4 | 同值 | 独立玻璃面 |
| SCS0009 图纸：主壳 | 23.3×12.1×25.25 mm | 同值包络 | 分段外壳近似，不是厂家舵机 CAD |

`pcb-main`、`pcb-touch`、`pcb-power`、`pcb-adapter` 和 `pcb-ring` 都保留带有 **UNKNOWN** 标识的估算板层；`pcb-power` 朝外面另有精确型号背照的局部可见外观，朝内面仍为 UNKNOWN。原理图只用于确认板的身份与功能；板形、厚度与位置依据壳内空间估计。未画芯片、走线、焊盘或触摸铜箔。12 个 LED 封装保留独立实体；发光区域直接由两条原 CAD 导光曲面各分六段，响应固件 RGB 值。没有额外贴在外壳上的灯片。550 mAh 容量来自官方资料，电池外形与位置属于空间估计，不附加臆造厂标。

侧面、底面和 CoreS3 顶面纸贴由官方产品照片确定性裁切、透视校正并加透明遮罩；它们是独立表面，换壳色时保持原样。螺丝六角暗部只是凹槽的视觉表达，未建真实螺纹。Grove A/B/C 与两处 USB-C 有独立开口和内壁；未核实的插接线序继续禁止连线。

来源与装配计划见 [model-plan.json](../../../web/platform/assets/m5stack-k151/model-plan.json)、[model.json](../../../web/platform/assets/m5stack-k151/model.json) 和 [补充抓取回执](../../../web/platform/assets/m5stack-k151/detail-source-receipts.json)。厂商 STL 固定在 M5_Hardware `e29c139cbc7c80b5ccc5557e5ab8025a7a19a109`；原始文件、抓取 UTC、字节数和 SHA-256 均保留。厂商照片及其衍生纸贴沿用来源版权，不纳入项目 MIT 授权。

本轮验收见 [尺寸与检查回执](../../../web/platform/assets/m5stack-k151/detail-visual-review.json)。六视角照片叠图、材质对照、内部视图和动作截图保留在本地 ignored `output/playwright/stackchan-details/`，不随网站发布；回执记录各文件 hash、字节数和相机参数。照片只按轮廓作等比平移配准，未完成独立相机标定，因此不把叠图作为制造精度或整机 1:1 证明。板上元件缺失、色差和贴纸投影残差均保留。

复现命令（依赖见 `tools/requirements-model.txt`）：

```sh
python tools/rebuild-product-model.py plan m5stack-k151
python tools/rebuild-stackchan-rear-appearance.py --scratch /new/stackchan-model-rebuild
node --test tools/platform-servo-view-test.mjs
```

当前板级运行时已经实现舵机协议与反馈驱动、RGB 链、Si12T 三区触摸、CoreS3 触摸屏；实测范围和固件限制见 [StackChan 运行时](../stackchan-runtime.md)。视觉关节只跟随板级反馈，不生成动画轨迹；电机的齿轮、碰撞、负载和线缆扭转未模拟，正方向与真实硬件尚未标定。外观尺寸通过不能替代固件或真实机械验收。


本轮 RGB 穿模修正的依据见 [修正回执](../../../web/platform/assets/m5stack-k151/rgb-fit-review.json)。此前的矩形灯片在导光条原曲面之外最高约 0.179 mm，整条分区又向前偏移 2 mm，末格超出导光条端部 1.35 mm；单点正侧向射线测试没有发现这些问题。旧 geometry hash 和真实固件亮灯截图 hash 保留为负结果。

现在直接分割每条导光条的原始 99 个外冠三角形，沿源坐标 Y 切成六个连续区域，不生成端盖、叠面或凸起。每侧原冠面积为 59.8414916902 mm²；导出全部顶点、三角形中心和边中点均核对原 CAD 表面，面积误差小于 0.000051 mm²。原壳外角与导光冠面均为约 5 mm 半径，独立圆拟合给出了装配基准：两条导光条各沿 45° 内法线平移 0.212132 mm，使光面约内陷 0.05 mm。源网格的尺度和形状没有改变。

精确多边形截面以 0.05 mm 的 Z 步长积分，导光条与壳体的交叠由左/右约 7.864/7.452 mm³ 降为 0.02358/0.001235 mm³，端部小残差仍保留。12 颗封装的估算位置内移后，同样检查中与壳体、导光条均未交叠。空白 PCB 本身也是估算包络，封装放得下不表示恢复了真实板上布局；物理链序与导光效果仍未标定。

该修正可用 `tools/prepare-stackchan-rgb-surfaces.py --write` 更新计划，再运行通用重建工具。`tools/test-source-plane-clip.py` 检查多平面切分、面积、法线和非法参数；`tools/verify-stackchan-rgb-surfaces.py --output <receipt.json>` 独立检查最终曲面。截面复现工具 `tools/verify-stackchan-rgb-sections.py <assets> --dz .05 --output <receipt.json>` 除模型依赖外需要 `shapely==2.2.0`，圆拟合工具 `tools/verify-stackchan-rgb-circles.py <assets> <section-receipt> --output <receipt.json>` 不改变任何网格。修正前后的接合残差和失败候选分别保留在 [截面回执](../../../web/platform/assets/m5stack-k151/rgb-section-fit.json) 与 [圆拟合回执](../../../web/platform/assets/m5stack-k151/rgb-circle-fit.json)。
真实固件复验中，前/中/后触摸分别产生十二路红/绿/蓝，并驱动水平 +40°、俯仰 +60°、水平 −40°。斜视与透明材质下已检查圆端、曲面和姿态；未加载固件时恢复中性灯面。最新 209 项平台测试通过；[浏览器回执](../evidence/m5stack-k151/runtime/rgb-surfaces-browser.json)保留最终模型/WASM及截图 hash。此前外观回执对应旧模型，继续作为历史记录保留。


2026-11-26 的嵌套展开修正把关节重挂后的真实零件纳入公开查看器的 inspection，
模型为 `0b97c19cdf232fd91df3c0196b8e84a1123e4aac66b07c55393ac7010d806f6c`。
只有新组显式声明 `nested:true`；原产品的直接子级解析行为保留。偏移在装配状态
从模型轴换算到原父节点局部轴，之后随 pan/tilt 反馈转动；展开不重挂节点、不改
关节四元数，复位恢复当前关节姿态下的原局部位置。

| 查看分组 | 来源与行为 |
| --- | --- |
| CoreS3 前模块 | 厂商 CoreS3 外壳、玻璃/屏幕/传感器、主板占位、USB-C/Grove-A 与顶部纸贴一同右前移 |
| 舵机外罩 | 三份厂商 ServoBody/ServoCover/ServoSideCover 对应外壳和背部紧固件左后移；实际舵机/电池/适配板保留 |
| 底盖 | 独立 BaseCover、底纸、脚垫与可见底部紧固件下移；ring 板占位保留 |
| 头部外壳 | 厂商 MainBody、导光件、侧纸、触摸命中面及原轴臂/轴承附件左上移；touch 板及 LED 封装保留在电子层 |

这些大间隙是明确的查看安排，不能当作实物拆卸行程或碰撞/线束模拟。每组保留
CAD/照片 sourceIds 和说明。五块 UNKNOWN 各有可公开旋转到的中心及四内角
最近射线视角，且在原反馈姿态复位。原三组方案的 touch 标识仍被 MainBody 挡住
一角（4/5），该失败保留在[负例](../evidence/m5stack-k151/inspection-20261126/negative-layout.json)。
现在独立头壳组清开了这个标识，没有移动或悬浮 UNKNOWN 字面。

[完整展开 CPU 图](../evidence/m5stack-k151/inspection-20261126/inspection-oblique.webp)
来自同一生产 loader 的 inspection=1，全装配、原厂色、650 像素、16 samples、
2 CPU threads、相机方向 `[0,1,3]`，没有手工隐藏零件；图中 power/touch UNKNOWN
可读。其余面的可达视角见[真实 loader 回执](../evidence/m5stack-k151/inspection-20261126/loader.json)。
四组 × 四种 pan/tilt 姿态 × 0/25/50/100/0% 验证了屏幕最近命中、相对板贴位置、
原父级/材质和精确复位；已展开时再更新关节反馈也保持跟随。

旧原色/金色/紫透 CPU 回执仍明确绑定照片修正后的 `7f0dd1c0…`，没有改写旧 hash。
[组装态等价证据](../evidence/m5stack-k151/inspection-20261126/assembled-equivalence.json)
把新 JSON 只删除 `/presentation/inspectionGroups` 后，按 UTF-8、indent=2、
ensure_ascii=False 与末尾换行重新编码，逐字节恢复旧 `7f0dd1c0…`；旧/新真实
loader 的三种材质 mesh、属性/index、UV/纹理、层级与世界矩阵语义 digest 也各自相同。
新增展开 CPU 则独立绑定最终 `0b97c19c…`，不能把旧截图改名当作新展开验收。

当前版本用保留原网格的专用重建路径，已[逐字节复现](../evidence/m5stack-k151/inspection-20261126/offline-rebuild.json)：

```sh
python tools/rebuild-product-model.py plan m5stack-k151
python tools/rebuild-stackchan-rear-appearance.py --scratch /new/stackchan-rebuild
node tools/verify-stackchan-inspection.mjs /new/stackchan-loader.json
node --test tools/platform-model-inspection-test.mjs tools/platform-nested-model-inspection-test.mjs tools/platform-servo-view-test.mjs
node tools/export-m5-cpu-models.mjs --repo . --out /new/stackchan-cpu/export --ids m5stack-k151 --inspection 1
blender -b -t 2 --python tools/render-m5-cpu-qa.py -- \
  --repo . --input /new/stackchan-cpu/export --out /new/stackchan-cpu/render \
  --ids m5stack-k151 --size 650 --samples 16 --appearance original --oblique-eye 0,1,3
```

原 geometry 和 27 个既有 artifacts 保留不动；厂商背照派生图仍是部分照片外观，
没有新增 PCB CAD 或固件兼容性结论。完整检查、工具 hash、未运行项及图像保留政策
见[展开审核回执](../evidence/m5stack-k151/inspection-20261126/review.json)。
