# StampFly 底部光流组件修正

K138 与 K138-V11 的旧底面把光流封装、白色插座和外伸插头一起裁成了 PCB 照片。板体虽已存在，但模型缺少凸起封装和插座；旧板顶比托架底面高约 0.098 mm，照片裁切还把插头当成了板长。斜侧面因此像一张悬在电池下的照片。旧计划与模型元信息由各产品 `history/2026-11-04-before-optical-fix/source-snapshot.json` 固定原提交、路径与哈希，旧验收记录也保留在该目录，不以旧正面轮廓分数证明这次底部修复。

此次保留原始 frame、battery 和 battery_adapter 网格及坐标。中央白色电池托属于 `source-frame.stl`；`source-battery_adapter.stl` 是机尾的电池转接件，不能误作光流板的承载面。原始 frame 中有两条宽 13 mm 的底横梁，底面都在 Z = −7.247989654541016 mm，Y 范围分别为 [−9.08895, −6.58895] 和 [4.91105, 7.41105] mm。

| 部分 | 本次表达 | 依据与边界 |
| --- | --- | --- |
| 光流 PCB | 12 × 20 × 0.8 mm；中心 [0, −1.3, −7.647989654541016] mm | 长宽按该 SKU 板边照片估算 ±0.6 mm，板厚估算 ±0.2 mm；内侧与两个源横梁接触，接触面积共 60 mm²。没有取得板级生产 CAD。 |
| PMW3901MB 封装 | 6 × 6 × 2.28 mm 总体；独立方形本体与圆形镜片 | M5Stack 链接的 PixArt v1.10 第 7 页给出总体尺寸和镜片直径 3.45 mm。本体 1.8 mm / 镜片 0.48 mm 的台阶分配、轮廓细节和横向位置仍为照片估计。 |
| 插座、插头 | 独立有厚度的壳壁、底、顶和部分插入的插头 | 官方原理图 P1 为 KH-A1001WF-06A、六个触点；连接器几何依照片估算，未取得其精确 CAD，不声称接触配合精度。可见柔性线束尚未还原。 |
| 贴图 | 板面、封装顶、圆形镜片、插座顶四张独立纹理 | 各自使用该 SKU 原图。板图 alpha 剔除封装、插座，裁切不含外伸插头及线；照片面离对应实体外侧 0.01 mm。镜片承载几何也是圆形，不能靠透明角落掩盖悬空方片。 |
| 内侧 | 黑色 PCB 与 `UNKNOWN` | `分开查看` 将光流组件及其纹理一起沿 −Z 移动 18 mm；这是检查位置，不代表真实拆装行程或线束余量。恢复后全部局部变换一致。 |

原版 K138 继续显示源装配中的 300 mAh 电池，其底面到光流板内侧有 0.75 mm 间隙。源电池与托架内侧约 0.05 mm 的原有交叠未被修改，也不称为生产配合验证。官方图库底面和各侧面均为拆下电池的状态；本轮没有取得安装电池后的官方近景，因此只用这些照片核实光流板在托架下方的关系。K138-V11 延续已披露的拆电池展示，不把源旧电池冒充 320 mAh 版本。

底色与照片色仍受拍摄光线影响；黑色 PCB、镜头、插座、纸贴不随外壳金色或透明紫色变化。完整浏览器几何范围（包括派生零件，排除交互面和文字）仍为 81.842155 × 81.842155 × 31.527990 mm。官方整机尺寸与简化源之间的既有偏差保持公开，未缩放源模型。

来源为 [原版官方页](https://docs.m5stack.com/en/app/StampFly)、[v1.1 官方页](https://docs.m5stack.com/en/app/StampFly_v1.1)、[PMW3901MB 资料](https://m5stack.oss-cn-shenzhen.aliyuncs.com/resource/docs/products/app/Stamp%20Fly/PMW3901MB-TXQT.PDF)和[光流板原理图](https://m5stack-doc.oss-cn-shenzhen.aliyuncs.com/1069/Sch_PMW3901MB_SPI.pdf)。各自的原始 URL、下载时间、字节数与 SHA-256 在两个 `source-audit.json` 中保留。Ecosystem STL 是官方链接的简化模拟器资产，不能标为完整生产 CAD。

## 复现与证据

```sh
python3 tools/verify-stampfly-optical-assembly.py
.venv-model/bin/python tools/rebuild-product-model.py build m5stack-k138 --scratch /tmp/k138-bottom-rebuild
.venv-model/bin/python tools/rebuild-product-model.py build m5stack-k138-v11 --scratch /tmp/k138-v11-bottom-rebuild
node tools/platform-knowledge-test.mjs
node --test tools/platform-manufacturer-model-test.mjs tools/platform-model-inspection-test.mjs tools/platform-view-test.mjs
```

浏览器检查入口为 `tools/fixtures/stampfly-bottom-model-review.html?sku=m5stack-k138`，另一款传入 `m5stack-k138-v11`；通过同一生产 `createManufacturerModel` 加载器加载实际资产。`review.show('bottomDetail')`、`sideDetail`、`supportDetail` 与 `obliqueDetail` 为固定近景；`review.inspection(true/false)` 用于核验声明的检查位移。工作台的公开 `分开查看` / `恢复装配` 按钮也已逐款点击验证。这里只检验模型，不重复声称固件或真实硬件通过。

- [K138 回执](evidence/m5stack-k138/bottom-fix-review.json)：[底面](evidence/m5stack-k138/bottom-fix/bottomDetail.webp)、[侧面](evidence/m5stack-k138/bottom-fix/sideDetail.webp)、[斜底](evidence/m5stack-k138/bottom-fix/obliqueDetail.webp)、[源图对照](evidence/m5stack-k138/bottom-fix/source-comparison.webp)。
- [K138-V11 回执](evidence/m5stack-k138-v11/bottom-fix-review.json)：[底面](evidence/m5stack-k138-v11/bottom-fix/bottomDetail.webp)、[侧面](evidence/m5stack-k138-v11/bottom-fix/sideDetail.webp)、[斜底](evidence/m5stack-k138-v11/bottom-fix/obliqueDetail.webp)、[源图对照](evidence/m5stack-k138-v11/bottom-fix/source-comparison.webp)。

照片对照用于检查板边、承载关系、凸起和方向；不是独立制造尺寸或完整电子布局验收。底面纹理的局部对齐与它自己的来源照片有共同依据，不能把这类吻合分数当作额外测量证据。
