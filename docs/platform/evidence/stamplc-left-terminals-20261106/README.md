# StamPLC 左侧插座孔位修正

本次修正橙色 PWR485、黑色 PWR-CAN 及两枚终端电阻开关的装配偏移。旧模型把它们放在 Y=13.2／13.3／13.25 mm，原厂外壳的对应开孔中心实际均为 **Y=12 mm**，导致插座一侧被外壳挡住。PWR485 和 PWR-CAN 的上缘还分别进入孔沿约 0.134 和 0.128 mm。

[正左修正前后](left.webp) · [左前修正前后](left-front.webp) · [左后修正前后](left-back.webp) · [官方左侧照片（外部来源）](https://m5stack-doc.oss-cn-shenzhen.aliyuncs.com/1129/K141_12.webp)

## 依据与改动

依据是[官方 StamPLC 文档](https://docs.m5stack.com/en/core/StamPLC)、[正左照片 K141_12](https://m5stack-doc.oss-cn-shenzhen.aliyuncs.com/1129/K141_12.webp)、[斜侧照片 K141_07](https://m5stack-doc.oss-cn-shenzhen.aliyuncs.com/1129/K141_07.webp)，以及固定版本的[厂家 STL](https://github.com/m5stack/M5_Hardware/blob/e29c139cbc7c80b5ccc5557e5ab8025a7a19a109/Products/K141_StamPLC/Structures/StamPLC.stl)。沿原生外壳 X=−35.999 mm 作截面，独立提取开口边界；没有移动外壳来迁就插座。原厂源文件、照片哈希和真实历史获取时间见 [validation.json](validation.json)，本次没有改写获取时间。

| 部件 | 原中心 Y／Z（mm） | 修正后中心 Y／Z（mm） | 处理 |
| --- | --- | --- | --- |
| PWR485 | 13.2／6.35 | 12／6.1 | 座体、后壁、四个触点与套圈整组平移；两枚黑色卡扣分别对准 Y=15.9、8.1 mm 的原生槽口，底面与座体相接。 |
| PWR-CAN | 13.3／−3.75 | 12／−3.98 | 座体、后壁、两个电源触点及两个信号触点整组平移。CAD 开口中点 Z=−3.9763 mm，模型按 0.01 mm 取值。 |
| 上方电阻开关 | 13.25／−0.15 | 12／−0.25 | 框体与滑块一起对齐原生缺口。 |
| 下方电阻开关 | 13.25／−8.85 | 12／−9 | 框体与滑块一起对齐后壳原生开孔。 |

PWR485 口沿保持 X=−35.8 mm，PWR-CAN 保持 X=−35.7 mm；朝外法向均为 −X，局部横轴为 −Y。22 个派生零件只调整位置和依据说明，尺寸、旋转、镂空轮廓均保持不变。全部 22496 个原厂三角形、贴纸、屏幕和其他端子未改动；[alignment.json](alignment.json) 保留原生截面数值、整组位移和逐件前后坐标。

这两个工业接口当前是可见外观部件，没有已实现的线缆锚点或电气插接功能；没有为本次几何修正生成虚构连线。浏览器从真实网格测量它们的口沿和方向，并另外确认 Grove A／C 的现有插口锚点、法向和线序未变。

## 验证与限制

- [生产加载器检查](browser.json)：16 个口沿采样均直接命中对应座体，8 个触点采样可见，两枚卡扣底面与座体顶面相接；正左、左前、左后相机及整机包围盒一并记录。完整外形为约 72.015 × 80 × 33.415 mm，符合 72 × 80 × 33.4 mm 图纸的 0.1 mm 检查容差。
- 相同检查加载旧模型时在 PWR485 的 Y=21.1500、Z=6.35 mm 口沿失败，首个命中对象为 `shell-front`；负例见 [validation.json](validation.json)。修正后该采样随座体移至孔内，通过检查。
- [离线重建](rebuild.json)：28 个资产全部一致，模型 JSON 逐字节一致；再次执行定位脚本不会继续累加位移。
- 配置、输入、视图、知识四组平台检查及目录 schema 校验通过。这个独立工作树的首次 runtime 检查因缺少预置 ROM/WASM 未启动，完整平台运行时检查由集成工作树执行。本次没有改动运行时，也不以外观检查扩大固件兼容结论。
- 三视角使用相同相机对比修正前后。官方照片与模型的并排图只作定性外观核对，未重新标定实物相机或宣称照片级 100% 还原。当前连接器尺寸和隐藏深度仍沿用约 ±0.4 mm 的照片估算；模型保持 `partial`，未知 PCB 布局仍保留。

原始截图不进入 Git；提交的三张并排图均由模型渲染截图制作，小于 50 KiB，没有额外嵌入厂商照片面板。渲染沿用模型既有的贴纸纹理。含厂商照片的 `official-left.webp` 只保留在被忽略的 `output/playwright/stamplc-left/`，不作为站点证据资产。[images.json](images.json) 保留外部照片 URL、来源 SHA-256、已核对的本地面板哈希及三张渲染对比图的记录；`make-comparisons.py` 只做等比缩小和白边排版。

## 复现

Python 环境安装 `tools/requirements-model.txt`，在仓库根目录运行：

```sh
python tools/align-stamplc-left-connectors.py
# 从旧版计划重新定位时使用 --apply；再次运行应不再改变资产。
python tools/align-stamplc-left-connectors.py --apply
python tools/rebuild-product-model.py build m5stack-k141 --scratch <fresh-scratch-directory>
```

本地静态服务打开平台页后，用任务专属 Playwright 包装器运行 `tools/fixtures/stamplc-left-connector-check.js`。脚本禁用缓存，核验实际加载模型和原生几何哈希，截图写入被忽略的 `output/playwright/stamplc-left/`。完成后关闭该会话并确认退出。

`make-comparisons.py --before <before-captures> --after <after-captures>` 默认只生成三张模型渲染对比图；两个输入目录分别提供 `left.png`、`left-front.png`、`left-back.png`。如需在本地重做官方照片比较，先把上面的外部照片下载到 ignored `output/`，再添加 `--official-photo <downloaded-K141_12.webp> --full-left <orthographic-render.png>`。脚本核对来源 SHA-256 为 `e47b69f889acb70bc8b312ff781eb25d5c05a09a8334868c7b03a1fcc00fd7f3`，仅将含照片面板写入 `output/playwright/stamplc-left/official-left.webp`。
