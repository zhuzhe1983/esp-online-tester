# Grove 插口、插头与线序

模型坐标使用毫米。每个插口有独立局部坐标：原点位于插口口沿，+Z 朝外，+X 沿四个引脚排列方向。`pinOrder` 必须按局部 -X → +X 声明；不能把照片中的“从左到右”直接用于旋转后的模型。

`grove-geometry.js` 提供中空插座与插头。引脚间距为 2 mm；插头前端进入插口 3 mm，止挡留在口沿外，四根线以 2 mm 间距从插头后部引出。3 mm 插入量、塑料壁厚及隐藏深度是照片辅助的可视化估算，尚无该批 M5Stack 连接器完整装配 CAD，不能作为生产公差或机械配合证明。四个针脚进入插头孔，不能以实心黑块封住插口。不同产品的插座塑料颜色按来源声明 `housingColor`，与针脚、内部阴影和可改色机壳保持材质隔离；不能把所有 Grove 口都设为白色。

线缆只为同时满足以下条件的两端生成：

- `allowConnections: true`；
- 明确的 `pinOrder`，包含且只包含一次 `gnd`、`vcc`、`signal1`、`signal2`；
- 非空 `sourceIds`，可追溯到该 SKU/版次的照片或图纸。

未核实的接口保留外观，但不生成“看似已插好”的线缆。`signal1` 对应黄线、`signal2` 对应白线，名称不意味着所有产品都使用同一 GPIO 或 UART 方向；物理线色与电气模拟分别验证。

| 产品 | 局部 -X → +X | 核验依据 |
| --- | --- | --- |
| StopWatch C152 | 白、黄、红、黑 | 官方背贴 `photo-07`，G11/G10/5V/GND |
| NanoH2 C149 | 白、黄、红、黑 | 官方背贴 `product-photo-ac6b7ae28d`，背视图左→右 G/5V/G2/G1，背视方向反转 X |
| GPS Unit U032-V11 | 黑、红、黄、白 | 官方背贴 TXD/RXD/5V/GND，背视方向反转 X；文档针脚表 |

实际旋转后的两端分别计算线序，不能为了让线缆平行而交换信号。线缆仍是视觉装配；当前 GPS NMEA 注入不代表物理 UART 路由、供电或时序已经模拟。

来源：

- [M5Stack Grove Cable A034 官方页](https://docs.m5stack.com/en/accessory/cable/Grove_Cable)与[官方产品照片](https://shop.m5stack.com/cdn/shop/products/1_86ab3aec-7779-4067-87b2-abb9a9cb947e_1200x1200.jpg?v=1666946172)：线色、插头外观。
- [Seeed Grove 官方机械图](https://files.seeedstudio.com/wiki/GroveSystem/res/Grove_Mechnical_Drawing.zip)：参考接口形态。水平 SMD 图为另一型号，未当成 M5Stack 实际连接器 CAD。
- [M5Stack A118](https://shop.m5stack.com/products/grove-female-header-hy2-0-4p-smd-with-locating-pins-20pcs)说明其端子造型与传统 M5Stack 连接器不同，因此未直接替代产品内的型号。

抓取及 hash 见 `evidence/grove-connectors/sources.json`。几何回归运行 `node tools/platform-scene-test.mjs`；它检查插入段、口沿止挡、中空通道、旋转后的两端线序和未知接口拒绝连线。真实页面截图另行核验外壳、口沿与插头之间是否存在间隙、穿模或错误遮挡。
