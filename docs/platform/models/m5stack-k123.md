# M5Stack Station-485 · K123

厂家 CAD 按毫米导入；状态为 partial。此型号尚无可运行的板级模拟器。

带 RS485、六组 Grove 接口、功率监测和小尺寸彩屏的 ESP32 工业控制主机；USB-A 仅提供电源输出。

装配：Source assembled cluster near X=0 retained, translated only. Print-layout duplicates to positive X excluded. DIN clip remains in original position.

核查结果：11 个保留组件，11 个排除组件；15 项几何检查，1 项未通过。失败值保存在 [verification.json](../evidence/m5stack-k123/verification.json)。

浏览器照片核对：实际浏览器四视图确认 DIN 卡扣、两侧支架和前面三个键的拓扑及相对位置；按照片将两侧支架由初次误选红色改为黑色，按钮为深灰。来源整体高度冲突保留。缺螺钉、背贴和连接器内芯；没有量化照片对齐通过。

限制：

- Specification height 88 mm differs from CAD overall 92 mm and marketing photo 92 mm; failure kept visible.
- Some unlabelled CAD inserts retain neutral source-component names; no unverified electrical role inferred.
- Display placement and interior board are estimates; RS485/Grove/electrical behavior unavailable.
- No complete board/peripheral emulator exists for this SKU; preview and documentation only.
- PCB placeholder has no components. Its dimensions/placement are an enclosure-space estimate, not a verified PCB layout.
- Manufacturer photo rights are separate from CAD MIT license. Materials are visual approximations, not measured reflectance.

来源与装配计划：[source-audit.json](../evidence/m5stack-k123/source-audit.json)。复现：`python tools/onboard-lane-03.py prepare --ids m5stack-k123`，然后 `python tools/onboard-lane-03.py build --ids m5stack-k123`；依赖 `tools/requirements-model.txt`。厂家 CAD 保留 MIT；官方照片未核实另有开放许可，不归入项目 MIT。
