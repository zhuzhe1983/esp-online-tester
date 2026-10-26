# M5Stack PaperDIY Complete Kit · K159-B

厂家 CAD 按毫米导入；状态为 partial。此型号尚无可运行的板级模拟器。

ESP32-S3、4.7 英寸墨水屏与可扩展 FPC 接口组成的轻薄电子纸套件；包含 3D 打印外壳和 140 mAh 电池。

装配：Rotate -90 degrees around Z and translate assembled source cluster to portrait orientation. Original millimetres and all component-relative positions preserved; no scale or deformation.

核查结果：6 个保留组件，5 个排除组件；10 项几何检查，1 项未通过。失败值保存在 [verification.json](../evidence/m5stack-k159-b/verification.json)。

浏览器照片核对：实际浏览器四视图与官方前/后/端口/侧面照片并列检查。修正了初次整体倒置；现用户键位于左侧、独立电源键位于下端，与前视照片方向一致。顶部内部卡扣/连接器尚未被真实电子组件覆盖，屏幕与内部尺寸为估计，未通过完整外观精度验收。

限制：

- Screen opening and PCB dimensions are explicitly estimated; no full 1:1 interior claim.
- Overall button protrusion differs from official body length; dimensional failure is retained.
- PaperDIY factory firmware is an upstream reference; M5PM1/e-paper/FPC behavior is not implemented.
- No complete board/peripheral emulator exists for this SKU; preview and documentation only.
- PCB placeholder has no components. Its dimensions/placement are an enclosure-space estimate, not a verified PCB layout.
- Manufacturer photo rights are separate from CAD MIT license. Materials are visual approximations, not measured reflectance.

来源与装配计划：[source-audit.json](../evidence/m5stack-k159-b/source-audit.json)。复现：`python tools/onboard-lane-03.py prepare --ids m5stack-k159-b`，然后 `python tools/onboard-lane-03.py build --ids m5stack-k159-b`；依赖 `tools/requirements-model.txt`。厂家 CAD 保留 MIT；官方照片未核实另有开放许可，不归入项目 MIT。
