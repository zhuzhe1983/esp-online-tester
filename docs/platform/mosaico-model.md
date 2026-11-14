# ESP-Mosaico 官方外壳模型

`espressif-esp-mosaico` 已采用官方 **ESP-MOSAICO.3mf** 的主壳和两件按键。显示盖板、PCB 占位层和背面外观分别保留来源标记；没有把三件外壳 CAD 称作完整整机 CAD。产品运行记录见[运行报告](mosaico-runtime.md)，本页黑屏截图只用于结构检查。

## 来源与许可

[官方 Core Unit 硬件页](https://mosaico.ai/en/hardware/bzWKymkXPu95)直接提供 [3MF](https://mosaico.ai/api/v1/files/attachments/1/9734120a8783.3mf)和[装配说明](https://mosaico.ai/api/v1/files/attachments/1/255888ae365a.pdf)。3MF 于 2026-11-14 16:03:31 UTC 获取，17,727,347 字节，SHA-256 为 `aa84074593344c37684cc2f36c58b5698ed5b695246e701ff05e49d325c176b3`。它绕开了此前无法直接取得文件的 MakerWorld 页面，但没有改变原文件的许可。

3MF 内的许可原文为 **MakerWorld Exclusive License**，作者为 ESP-Mosaico。转换结果保留该名称和[许可入口](https://makerworld.com/en/exclusive-model-policy)，不将厂家几何改标为 MIT，也不声明取得开放再分发授权。自行生成的显示/UNKNOWN 占位和文字 SVG 单独使用仓库许可；商标与参考照片仍保留原权利。原始 3MF 和第三方固件没有进入仓库。固定 URL、获取时间、字节数及摘要保存在 [source-manifest.json](../../web/platform/assets/espressif-esp-mosaico/source-manifest.json)和 [LICENSE-Model.txt](../../web/platform/assets/espressif-esp-mosaico/LICENSE-Model.txt)。

## 保留的几何与装配

原件共 **23,732 个三角形**：主壳 19,916，白色双键联体 2,976，橙色 AI 键 840。全部保留，没有简化、裁切、非均匀缩放或补猜背盖。主壳前后贯通；另一个磁吸背盖附件不在本模型中。

单位保持毫米。先应用各 component 变换，再应用 `Metadata/model_settings.config` 的 `assemble_item`；`3D/build` 的打印盘排版不参与装配。平台统一变换为绕 Z 轴旋转 180°、Z 平移 −5.550025463104248 mm：+X 向屏幕右，+Y 向上，+Z 向显示面外。全部原变换、原件边界、三角形数量和语义分组见模型的 `sourceGeometry`。原坐标 +Z 朝向显示面，是依据官方装配图、螺钉柱和按键内部肩台核对的方向推断；官方图没有标注坐标轴。BOOT/POWER 左右关系另据官方 FAQ 与 V1.2 照片确认。

白色联体按完整三角形分成 `button-boot`、`button-power`、`button-link` 三个渲染组，仍属于同一装配件；没有剪掉或复制中间连杆。BOOT 保持 USB 左侧 GPIO61，AI 保持顶部右侧 GPIO7。POWER 接硬件电源控制器，没有 GPIO 热点。射线检查覆盖两颗可操作键的最近可见面、POWER 无输入，以及屏幕中心和偏角触点的 UV 方向。真实运行时屏幕直接使用客体 framebuffer；静态参考图不会覆盖它。

默认颜色来自 3MF 打印配置：主壳和白色双键为 `#CBC6B8`，AI 为 `#FF671F`。这是厂家文件的打印耗材配色，不是生产塑料的实测色值。金色、半透明紫色只改变外壳材质，按键、屏幕、USB、PCB 与背字保持自身材质。

## 明确保留的未知项

3MF 没有显示屏、PCB、连接器、螺丝、电池或扬声器 CAD，也没有明确的 V1.2 机械版号。与 CoreBoard/BaseBoard V1.2 的对应关系来自官方装配说明及可见布局，不能据此认证全部机械差异。

显示盖板 **43.3×43.3×0.7 mm**、显示有效区 **38.39×38.39 mm** 来自 H0216F002AMT004-1 图纸；盖板圆角和装配高度仍为估算。盖板前面 Z=6.25，显示面 Z=6.26 mm。

`pcb-main`、`pcb-base` 保持稳定 ID 和可读的 **UNKNOWN**。背面深色表面是估算的 base PCB，不是官方背盖；背字、七个焊盘和四个可见螺丝头仍是照片派生外观，QR、走线、元件和隐藏螺纹没有补造。USB 是置于官方开口内的通用视觉口，插合深度未核实，`allowConnections` 仍为 false。厂家 CAD 已有侧孔、通风孔和键帽纹路，旧的估算孔洞标记已移除。

## 核验结果

[几何核验](evidence/mosaico-model-official/geometry-verification.json)绑定本次最终模型摘要。逐一比较 71,196 个原件顶点实例，Float32 导出最大误差 **0.000000479 mm**；23,732 个原三角形全部对应。主壳为 **44.8980027×44.8980027×11.10000038 mm**。完整查看器约 44.898×44.900×12.015 mm，包含派生显示/背板外观；它不是独立制造尺寸证据。

[照片对照参数](evidence/mosaico-model-official/alignment.json)继续采用官方 V1.2 原图中的独立盖板角点标定相机，模型没有拉伸。正斜视/另一斜视的盖板拟合 RMS 分别为 6.77/3.39 px，外壳边界角点平均偏差为 **10.09/8.03 px**。角点为直边外推，约 ±5 px；外壳圆角、照片透视、版次和估算的显示座高均影响结果。这是有限地标对照，**没有通过整机 1:1、完整照片或制造公差验收**。

无头浏览器检查了[原色](evidence/mosaico-model-official/mosaico-official-isometric.png)、[背面](evidence/mosaico-model-official/mosaico-official-back.png)、[USB/双键边](evidence/mosaico-model-official/mosaico-official-bottom.png)、[金色](evidence/mosaico-model-official/mosaico-official-gold.png)、[透明紫色](evidence/mosaico-model-official/mosaico-official-purple.png)及[完整展开](evidence/mosaico-model-official/mosaico-official-expanded-purple.png)。两层 UNKNOWN 清楚可读，反复展开/复位 20 次无位置漂移。完整检查和截图摘要见[本次回执](evidence/mosaico-model-official/receipt.json)。旧的[照片估算模型回执](evidence/mosaico-model/receipt.json)保留为历史，不作为本次官方几何的验收依据。

## 重建

导入器仅用 Python 标准库；必须提供固定摘要的本地来源，或显式允许网络获取。

```sh
python3 tools/build-mosaico-model.py --source /path/to/ESP-MOSAICO.3mf --output /tmp/mosaico-rebuilt
# 或直接从固定官方 URL 获取并核对摘要，不保存原始大包：
python3 tools/build-mosaico-model.py --allow-network --output /tmp/mosaico-rebuilt
cmp web/platform/assets/espressif-esp-mosaico/model.json /tmp/mosaico-rebuilt/model.json
cmp web/platform/assets/espressif-esp-mosaico/geometry.bin /tmp/mosaico-rebuilt/geometry.bin
python3 tools/verify-mosaico-model.py --source /path/to/ESP-MOSAICO.3mf
# 可选照片相机复算另需 numpy/scipy：
python3 tools/verify-mosaico-model.py --source /path/to/ESP-MOSAICO.3mf --photo-alignment
node --test tools/platform-mosaico-model-test.mjs tools/platform-model-inspection-test.mjs tools/platform-input-test.mjs tools/platform-view-test.mjs
python3 -m http.server 8187 --directory web
```

访问 `/platform/mosaico-model-review.html` 可复查三种配色和展开结构。此页不启动固件。产品的新默认固件为官方 **Vibe 0.3.0 raw 恢复镜像**，不含 Hello；完整 Hello 恢复项仍保留为 factory。下载链路和 CORS 检查见[下载回执](evidence/mosaico-download-20261115/README.md)，真实触控/按键后的固件结果仍由运行报告单独记录。
