# Stamp-S3Bat S015 模型核验

S015 现在保留厂家外壳 STL，并加入官方 KiCad 文件内嵌的 PCB 装配 STEP：384 个原生实体／曲面，包括真实 PCB、通孔、USB-C、电池座、BTB 和电子元件。原来的 UNKNOWN 板和通用 USB 占位件已替换。S015 不焊排针，与 S015-DIP 分开维护。

来源为[官方产品资料](https://docs.m5stack.com/en/core/Stamp-S3Bat)、M5_Hardware `ec351c516644b44defc145728cc029ced77381ff` 的[官方 KiCad 容器](https://github.com/m5stack/M5_Hardware/blob/ec351c516644b44defc145728cc029ced77381ff/KiCad/Footprints/M5Stack.pretty/Stamp-S3Bat.kicad_mod)，以及保留原始来源版本的外壳 STL。容器 SHA-256 `830babe06213a16070cf1361baac7ba2d73bf5164b43d8e77338e5ca57d414d3`；内嵌 STEP SHA-256 `b304eac5dc0129563800b7637ad97a6aeb1ca9ad25fd87f4775245365d354f95`。原始获取时间、下载与内嵌校验记录见 [source-audit.json](../evidence/m5stack-s015/source-audit.json)。

模型只采用刚体平移，没有缩放厂家几何。PCB 原点为板中心、器件面 Z=0；在外壳坐标下，PCB 中心 Y=+0.9 mm、器件面 Z=−1.75 mm。外壳原始 17.7556 × 29.6240 × 5.5 mm 与整机包络分别记录，不能将外壳尺寸当作整机尺寸。旧版失败结果保留在 [previous-cover-only-verification.json](../evidence/m5stack-s015/previous-cover-only-verification.json)。

| 检查 | 模型值 mm | 独立图纸值 mm | 结果 |
| --- | ---: | ---: | --- |
| 整机宽度 | 18.0000 | 18 | 通过，容差 0.1 |
| 含 USB 整机长度 | 29.77993 | 29.8 | 通过，容差 0.1 |
| 整机厚度 | 5.5000 | 5.5 | 通过，容差 0.1 |
| PCB 宽／长／厚 | 18 / 28 / 0.999998 | 18 / 28 / 1 | 通过，容差 0.05 |

黑色 PCB 的背面照片只投影到原生 Z=−2.749998 mm 平面；6972 个投影顶点的最大平面偏差为 0.000000093 mm。孔洞由厂家几何保留，贴图不延伸到孔壁、侧壁或独立接口。金色和半透明紫色只改变外壳，PCB、丝印、纸贴、FPC 和接口材质保持不变。

前面纸贴的承载物需要分开说明。[官方拆解视频](https://m5stack-doc.oss-cn-shenzhen.aliyuncs.com/1222/Stamp-S3Bat-Housing-Disassembly-Tutorial.mp4) 9.8–12 秒先揭下纸贴，露出独立黑色 FPC 天线；24.5 秒的画面显示天线移除后的原生外壳槽孔。模型因此补上有照片／视频依据的 FPC 平面外形，而不填平外壳孔洞。FPC 的 0.2 mm 厚度关系来自 2.35/2.55 mm 台阶，属于估算；外形不确定性约 ±0.3 mm，弯折和隐藏铜箔未重建。实物纸贴本来跨过 FPC 下方的内部释放槽，该小范围跨槽保留；外缘及按键区单独遮罩，不能误剪掉真实印刷内容。参考帧裁切随证据保存。

官方照片、图纸和拆解过程没有可确认的螺丝或螺柱，因此不添加猜测件。板级 CAD 缺少侧面可见 RF 屏蔽罩的精确几何；USB 内部不同材料的颜色也没有完整分组，保留明确限制。完整原生 PCB 几何不等于整机每层材料和制造细节均已核验。

实际通用渲染器的正面轮廓 IoU 为 **0.972881**，背面为 **0.990810**；两项高于预先使用的 0.95 门槛。USB 边视图为 0.921411，仅作定性比较，未宣称正投影精度或光学材质校准。参考、渲染、叠图与差异图见 [verification.json](../evidence/m5stack-s015/verification.json)，材质、孔面、PWR 命中、LED 绑定与实际资产哈希见 [browser-review.json](../evidence/m5stack-s015/browser-review.json)。

PWR 热点绑定产品目录中的 `power`，原生 WS2812 光学体绑定 LED 0。真实工作台上的鼠标按下／释放与 KeyA 按下／释放通过本地 ESP-IDF 5.5.1 测试固件验证：固件新读到 PM1 状态后依次输出绿色、蓝色。见 [workbench-input.json](../evidence/m5stack-s015/workbench-input.json)。初次异步等待取得了陈旧快照，被判无效并单独保留；正式回执使用明确等待每次 `observe()` 的循环，要求同一快照同时具有新增串口行和匹配 LED 值。本项不是出厂固件或物理开关机验证；板级边界见 [S3Bat runtime](../stamps3bat-runtime.md)。

使用 [model-plan.json](../../../web/platform/assets/m5stack-s015/model-plan.json) 和公开重建入口可离线复现：

```sh
python tools/rebuild-product-model.py build m5stack-s015 --scratch <fresh-scratch-directory>
```

[重建回执](../evidence/m5stack-s015/rebuild.json) 核对 9 个生成资产及完整模型配置均一致。额外保留的 KiCad 容器单独核对哈希，因此元信息文件不要求逐字节相同。依赖按 `tools/requirements-model.txt` 固定，OCP 7.8.1.1 采用 0.05 mm / 0.2 rad 细分；STL 保留原始三角形。平台配置／视图测试通过。厂商照片及视频衍生帧仍归原作者，不套用项目 MIT；CAD 许可保留原文件。
