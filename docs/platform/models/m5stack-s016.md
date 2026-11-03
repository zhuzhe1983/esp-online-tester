# Stamp-C5（S016）模型与来源

保留官方 KiCad footprint 内嵌的 `Stamp_C5_PCB_main_V0.3_20260127_0940.step`，固定 M5_Hardware 提交 `e29c139cbc7c80b5ccc5557e5ab8025a7a19a109`。STEP SHA-256：`69f7f228edbcb001c3111e3f8f36f557222916b48473a9523fcee178d83b51d1`。原 footprint、内嵌 STEP 校验及每张照片来源见 [知识记录](../../../web/platform/knowledge/m5stack-s016.json)。

全部 301 个原厂组件、206,389 个三角面均保留。按逐面坐标和法向哈希多重集检查，材料分组前后完全一致；PCB 背面与 IPEX 绝缘体、金属触点、焊脚分为独立材质，最终 304 个原生几何组。真实板级 CAD 没有被空白板或 UNKNOWN 替换。

原厂模型缺少照片中的 RF 屏蔽罩。[模型计划](../../../web/platform/assets/m5stack-s016/model-plan.json) 按正面板宽和侧面高度建立带 IPEX 缺口的金属盖与空心边框，原始元件仍在罩内。平面占地约 12.8 × 11.2 mm，盖顶模型 Z 为 1.595 mm，盖厚 0.15 mm、壁厚 0.12 mm 均是照片估算；隐藏卡扣、折弯、制造公差未知。没有用实心方块填充 PCB 空间。罩体原图标识 alpha 区域完全落在金属盖内，距离承载面 0.001 mm，不是悬空纸贴。

裸露红板与背面丝印取自该 SKU 官方高清照片，限定在原始 CAD 平面、通孔和轮廓内。USB、IPEX、LED 区域分别避让；USB 顶面采用实物照片，CAD 元件库的通用厂商雕字保留几何但与金属同色，不冒充产品印刷。照片暗槽不证明连接器内部深度；原生 USB 开放曲面保持不变。照片光照和材质参数未经色彩标定。

官方对比表及背面照片确认 S016 没有默认焊接 FPC 座，也不附排针。官网侧照出现 FPC 座，与上述来源不一致；保留冲突，只作为定性参考。S016-DIP 另设 SKU。未在照片中发现装配螺丝或螺柱，不按孔数补猜。

[最终验证](../evidence/m5stack-s016/verification.json)：尺寸 17.600 × 19.06253 × 3.36100 mm，与官方 17.6 × 19.1 × 3.4 mm 各轴误差 < 0.1 mm；正面 IoU 0.98482、背面 0.97237 通过。侧面 IoU 0.81507，仅定性。官方拆罩斜视仅留作内部参考，未作为完成罩体后的轮廓通过项。

[浏览器凭据](../evidence/m5stack-s016/browser-review.json) 对应最终模型和图片哈希。原色、金色、半透明紫色的每个视角逐像素相同：裸模块没有可改色外壳，PCB、金属罩、照片和触点保持独立材质。复现使用 `tools/requirements-model.txt` 和 `python tools/rebuild-product-model.py build m5stack-s016 --scratch <fresh-scratch>`；[离线重建](../evidence/m5stack-s016/offline-rebuild.json) 对应最终文件。

模型状态 partial；以上为机械和表面资料验收，不代表 ESP32-C5 固件运行或实机验证。
