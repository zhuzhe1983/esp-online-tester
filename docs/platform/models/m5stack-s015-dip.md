# Stamp-S3Bat DIP S015-DIP 模型核验

DIP 版保留原厂外壳，加入官方页面明确共用的 Stamp-S3Bat 板级 STEP，并补上原来缺失的正面纸贴、黑色裸板照片和两排九针排针。原来的 UNKNOWN 板、实心电池座及通用 USB 占位件已由原生 PCB、SH1.0、BTB 和 USB 几何替换。该型号仍作为独立产品维护。

[官方 DIP 页面](https://docs.m5stack.com/en/core/Stamp-S3Bat_DIP)直接链接 [Stamp-S3Bat KiCad 文件](https://github.com/m5stack/M5_Hardware/blob/ec351c516644b44defc145728cc029ced77381ff/KiCad/Footprints/M5Stack.pretty/Stamp-S3Bat.kicad_mod)，产品对照表将两款差别列为是否焊接排针。容器 SHA-256 为 `830babe06213a16070cf1361baac7ba2d73bf5164b43d8e77338e5ca57d414d3`，内嵌 STEP 为 `b304eac5dc0129563800b7637ad97a6aeb1ca9ad25fd87f4775245365d354f95`。384 个原生组件／曲面完整保留。DIP 外壳 STL 与 S015 的字节完全相同；原生外壳 17.7556 × 29.6240 × 5.5 mm 不缩放为整机尺寸。

[该型号尺寸图](../../../web/platform/assets/m5stack-s015-dip/reference-dimension-drawing-4a95d4285f.png)明确标出 2.54 mm 针距、15.24 mm 行距、最下孔距板边 1.3 mm，以及 18 × 29.8 × 12 mm 整机包络。排针中心为 X=±7.62 mm、Y=−11.8+2.54i mm（i=0…8）；针尖 Z=−9.25 mm，与原生外壳顶部形成 12 mm 总厚度。塑料座 2.54 mm 厚度、0.64 mm 方针截面来自图纸比例和侧面照片估算，约 ±0.15 mm；细小倒角、焊锡形状并无原厂 CAD，不能称为精确制造模型。

| 核验项 | 实测 mm | 独立依据 | 结果 |
| --- | ---: | --- | --- |
| 原生模块宽／长 | 18 / 29.77993 | DIP 图纸 18 / 29.8 | 通过，0.1 mm 容差 |
| 原生模块厚度 | 5.5 | 图纸 5.5，不含排针 | 通过 |
| PCB 宽／长／厚 | 18 / 28 / 0.999998 | 共用原厂 PCB 与图纸 | 通过 |
| 含排针总厚度 | 11.999996 | DIP 图纸 12 | 浏览器实际几何通过 |
| 排针数量／间距 | 18 / 2.54 | DIP 图纸和照片 | 通过 |

导出器的尺寸检查只计算导出的原生几何，因此把 5.5 mm 命名为“不含派生排针的模块厚度”；实际浏览器另外计算全部可见几何的包络，排除透明纸贴画布与不可见点击平面。不能拿 12 mm 目标误判原厂外壳，或把透明画布计入实体尺寸。

背面使用 DIP 自己的高清照片，贴图只落在真实 PCB 的平面上，孔壁、侧壁和凸起排针都不接受照片投影。遮罩清除排针及 USB 焊脚区域；裸板边缘为照片有依据的黑色。6972 个投影顶点均为背面法线，最大平面偏差约 0.000000093 mm。

正面纸贴是独立材质，按 DIP 原图重新配准到同一原生凹槽，保留 U 形按键空区，并修正下端圆角，避免穿入高一级的外壳台阶。透明画布尺寸不代表物理纸张外缘。其下方独立黑色 FPC 由[官方拆解视频](https://m5stack-doc.oss-cn-shenzhen.aliyuncs.com/1222/Stamp-S3Bat-Housing-Disassembly-Tutorial.mp4)与 DIP 正面照片交叉确认；共用支撑分析和视频帧见 [S015 模型报告](m5stack-s015.md)。实物纸贴确实跨过小范围内部释放槽，模型保留该结构，不虚构塑料填孔。逐像素支撑核验见 [paper-support.json](../evidence/m5stack-s015-dip/paper-support.json)：86879 个有效像素中，85984 个落在 2.55 mm 支撑面，895 个位于真实内部释放槽；没有像素穿入更高台阶。初次发现的单个边角像素与 0.034 mm 相交记录单独保留，最终圆角已修正。FPC 厚度和弯折未精确测量，侧面可见 RF 屏蔽罩也缺少精确原厂几何。没有证据支持添加螺丝或螺柱。

实际渲染器的正面轮廓 IoU **0.958054**、背面 **0.983270**，均通过原定 0.95 门槛；侧面 **0.885581** 只作定性检查，不能当作透视和材质精度认证。侧面差异包括未建精确 RF 屏蔽罩、简化针座和针尖。金色及半透明紫色只改变外壳，纸贴、板色、FPC、排针和接口保持独立。图像、哈希与检查结果见 [verification.json](../evidence/m5stack-s015-dip/verification.json) 和 [browser-review.json](../evidence/m5stack-s015-dip/browser-review.json)。旧版只有外壳且尺寸失败的记录保留在 [previous-cover-only-verification.json](../evidence/m5stack-s015-dip/previous-cover-only-verification.json)。

PWR 热点绑定 `power`，原生 WS2812 光学体绑定 LED 0。真实工作台运行独立构建的 ESP-IDF 5.5.1 测试固件：鼠标点击 PWR 后，客体在 7.10 秒读取新的 PM1 按下状态并输出绿色；KeyA 在 7.40 秒输出蓝色，两次释放均有新的串口行。回执要求同一快照同时包含新 UART0 输出和对应 RGB 值；见 [workbench-input.json](../evidence/m5stack-s015-dip/workbench-input.json)。这不代表物理开关机、复位或出厂固件验证；运行时边界见 [S3Bat runtime](../stamps3bat-runtime.md)。任务浏览器已关闭并核实退出。

公开重建入口可离线恢复全部必要来源，9 个生成资产与模型配置逐项一致：

```sh
python tools/rebuild-product-model.py build m5stack-s015-dip --scratch <fresh-scratch-directory>
```

[重建回执](../evidence/m5stack-s015-dip/rebuild.json)保留工具与依赖版本、原始输入和最终资产哈希。额外保留的 KiCad 容器及尺寸图单独核对，不冒充本次生成文件。平台配置、知识和视图测试通过；未实机验证。照片版权仍归厂商，CAD 许可随源文件保留。
