# Stamp-C5 DIP（S016-DIP）独立产品与模型

此条目对应 [官方 S016-DIP](https://docs.m5stack.com/en/core/Stamp-C5_DIP)，与 S016 分开保存产品身份、来源、资产与知识。FPC 0.5 mm / 12P 座出厂已焊；两条 2.54 mm / 7P 排针随盒散装。**模型展示官方背面与侧面照片中的用户排针装配示例，不能据此认定排针出厂已焊。** 包装中的 IPEX-1 双频 FPC 天线只列为附带物，没有自动接上或宣称 RF 已模拟。

官方两页指向同一个 KiCad footprint 和原理图。沿用固定 M5_Hardware 提交 `e29c139cbc7c80b5ccc5557e5ab8025a7a19a109` 中内嵌的 `Stamp_C5_PCB_main_V0.3_20260127_0940.step`，SHA-256 `69f7f228edbcb001c3111e3f8f36f557222916b48473a9523fcee178d83b51d1`。历史 footprint、内嵌校验与获取时间原样保留；DIP 页面、5 张独立照片及尺寸图使用本次实际获取的 UTC、URL、字节数与哈希，见 [知识记录](../../../web/platform/knowledge/m5stack-s016-dip.json)。原理图 URL 标 V0.3，而标题栏为 V0.2，未消除这个来源冲突。

全部 301 个原生 CAD 组件、206,389 个三角面保留，几何文件与修正后的 S016 逐字节相同，未以空白板或 UNKNOWN 替换。304 个材料组区分 PCB 背面与 IPEX 绝缘体/触点/焊脚。源 USB 库中 215–227 号微小厂商标注保留原始几何和 ID，但显示透明度为 0；实物照片提供产品可见表面，不把库标注当产品印刷。

[模型计划](../../../web/platform/assets/m5stack-s016-dip/model-plan.json) 记录独立的盖板、空心边框、FPC 座和两条排针组，共 51 个补充部件。RF 罩有 IPEX 缺口，标识 alpha 位于实体盖内。FPC 的白色壳体、黑色锁片与 12 个金属触点分别建模，插入口朝模型 −Y，保留空隙；未声明电气插接或线缆匹配通过。排针中心直接取 CAD 的 `x=±7.75 mm`、原始 `y=1.38+2.54×i mm`，共 14 个 Ø0.9 mm 孔。0.6 mm 方针可以落在孔内；针长 8.3 mm、塑料体 2.45×2.45×2.5 mm、FPC 外形及隐藏结构都是照片估算，不冒充厂家零件图。

DIP 正面照片与普通版正面照片字节相同，但保留各自官方 URL 和获取时间。背面采用 DIP 自己的高清照片，屏蔽 FPC、排针条和 USB 区域，避免把立体件贴平到板上；真实 CAD 轮廓与孔洞继续裁切贴图。实物颜色和光照没有做色彩标定。照片未见装配螺丝/螺柱，不添加猜测紧固件。

[尺寸与照片检查](../evidence/m5stack-s016-dip/verification.json) 明确区分：

| 检查范围 | 结果 |
| --- | --- |
| 官方模块本体 | 17.6 × 19.1 × 3.4 mm；[官方图纸](../evidence/m5stack-s016-dip/base-module-drawing.pdf) 标注 17.6 × 19.06 × 3.36 mm |
| 原生本体 CAD | 17.60000 × 19.06253 × 3.36100 mm；相对产品参数各轴误差 < 0.1 mm |
| 含排针的可见装配模型 | 17.95000 × 19.06253 × 10.06000 mm；照片估算，不属于官方 3.4 mm 尺寸验收 |
| 正面轮廓 | IoU 0.97104，通过 |
| 背面轮廓 | IoU 0.98269，通过 |
| 侧面轮廓 | IoU 0.91459，仅定性 |
| 包装与斜视资料 | 来源参考，不计为通过项 |

[最终浏览器凭据](../evidence/m5stack-s016-dip/browser-review.json) 包含前后侧视、斜视、FPC 近景、尺寸边界及图片哈希。原色、金色、半透明紫色的每个视角逐像素相同；裸模块没有可改色外壳。运行 `python tools/rebuild-product-model.py build m5stack-s016-dip --scratch <fresh-scratch>`，依赖固定为 `tools/requirements-model.txt`；[离线重建](../evidence/m5stack-s016-dip/offline-rebuild.json) 验证最终 12 个模型输入/派生资产，模型 JSON 逐字节一致。

模型为 partial。未验证实机、电气连接、RF 传播、FPC 插合公差或 ESP32-C5 固件执行；运行与固件记录由独立适配项目维护。
