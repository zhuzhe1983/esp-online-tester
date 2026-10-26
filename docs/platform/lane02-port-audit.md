# Lane02 端口复核

原27款与新增12款均保留 **partial** 状态。表中数量为已建模插口；没有插口模型不代表实物没有该接口。厂家板级CAD中的USB/BTB另列。每个口的坐标、朝外法向、口宽方向、颜色、来源和线序见 [结构化记录](evidence/lane02-ports.json)。

| 型号 | 插口模型 / Grove | 颜色与口位（面；口宽轴） | 连接线 | 缺项 / 否定旧草案 |
|---|---:|---|---|---|
| [m5stack-c153-lite](https://docs.m5stack.com/en/core/PaperMono-Lite) | 1 / 0 | usb-c metal -Y/+X | 禁用（未核实） | 外观插口估计；未实机/机械配合验证 |
| [m5stack-k124-b](https://docs.m5stack.com/en/core/Station-Bat) | 6 / 6 | grove-left-1 #e4e5dc -X/+Y；grove-left-2 #e4e5dc -X/+Y；grove-left-3 #e4e5dc -X/+Y；grove-right-1 #e4e5dc +X/+Y；grove-right-2 #e4e5dc +X/+Y；grove-right-3 #e4e5dc +X/+Y | grove-right-1:gnd,vcc,signal1,signal2；grove-right-2:gnd,vcc,signal1,signal2；grove-right-3:gnd,vcc,signal1,signal2 | 外观插口估计；未实机/机械配合验证 |
| [m5stack-c149](https://docs.m5stack.com/en/core/NanoH2) | 2 / 1 | grove #e4e5dc +Y/+X；usb-c metal -Y/+X | grove:signal2,signal1,vcc,gnd | Rejected original bottom-center cable position and inherited wire order. Explicit side mouth and back-view mirrored wire order retained. Close-up fit is visual only; hidden plug dimensions remain estimates. |
| [m5stack-s007-pin127](https://docs.m5stack.com/en/core/Stamp-S3_PIN1.27) | 0 / 0 | 厂家CAD：USB-C | 禁用（未核实） | Native STEP USB mechanical surfaces included. Header pitch is1.27mm; no2.54mm header geometry from sibling SKU is reused. |
| [m5stack-k049-b](https://docs.m5stack.com/en/core/Paper_v1.1) | 4 / 3 | usb-c metal -Y/+X；grove-a #e4e5dc -Y/+X；grove-b #e4e5dc -X/+Y；grove-c #e4e5dc -X/+Y | 禁用（未核实） | 外观插口估计；未实机/机械配合验证 |
| [m5stack-c139](https://docs.m5stack.com/en/core/PaperS3) | 1 / 0 | usb-c metal -Y/+X | 禁用（未核实） | 外观插口估计；未实机/机械配合验证 |
| [m5stack-s007-pin254-v033](https://docs.m5stack.com/en/core/Stamp-S3A_PIN2.54) | 1 / 0 | usb-c metal -Y/+X | 禁用（未核实） | S3A cover geometry is distinct from older StampS3 STEP. Do not infer old board/USB/headers as exact hardware revision. |
| [m5stack-k051](https://docs.m5stack.com/en/core/Stamp-Pico_Mate) | 0 / 0 | 未建插口模型 | 禁用（未核实） | 外观插口估计；未实机/机械配合验证 |
| [m5stack-s015-dip](https://docs.m5stack.com/en/core/Stamp-S3Bat_DIP) | 1 / 0 | usb-c metal -Y/+X | 禁用（未核实） | 外观插口估计；未实机/机械配合验证 |
| [m5stack-k131](https://docs.m5stack.com/en/core/Air_Quality) | 1 / 0 | usb-c metal -Y/+X | 禁用（未核实） | 外观插口估计；未实机/机械配合验证 |
| [m5stack-k130-v11](https://docs.m5stack.com/en/core/Dial_v1.1) | 3 / 2 | grove-a #9f2635 -Z/+Y；grove-b #242a27 -Z/+Y；usb-c metal -Y/+X | 禁用（未核实） | Rejected upside-down rear preview (+Y USB) and missing Grove pair. Same-source CAD and exact SKU rear image establish -Y USB, rear Ared/Bblack; oldS3 paper text was not reused for S3A. |
| [m5stack-k152](https://docs.m5stack.com/en/core/Cardputer_Mesh_Kit) | 2 / 1 | usb-c metal +X/+Y；grove #e4e5dc -X/+Y | 禁用（未核实） | USB/Grove local width corrected to worldY for mouths on ±X. |
| [m5stack-k132](https://docs.m5stack.com/en/core/Cardputer) | 2 / 1 | usb-c metal +X/+Y；grove #e4e5dc -X/+Y | 禁用（未核实） | USB/Grove local width corrected to worldY for mouths on ±X; keyboard photo includes only masked white paper, not keys/case. |
| [m5stack-c126-m12](https://docs.m5stack.com/en/core/AtomS3R-M12) | 2 / 1 | usb-c metal -Y/+X；grove #e4e5dc -Y/+X | grove:signal2,signal1,vcc,gnd | 外观插口估计；未实机/机械配合验证 |
| [m5stack-c126](https://docs.m5stack.com/en/core/AtomS3R) | 2 / 1 | usb-c metal -Y/+X；grove #e4e5dc -Y/+X | grove:signal2,signal1,vcc,gnd | 外观插口估计；未实机/机械配合验证 |
| [m5stack-c124](https://docs.m5stack.com/en/core/AtomS3-Lite) | 2 / 1 | usb-c metal -Y/+X；grove #e4e5dc -Y/+X | grove:signal2,signal1,vcc,gnd | 外观插口估计；未实机/机械配合验证 |
| [m5stack-c008-c](https://docs.m5stack.com/en/atom/Atom_Voice) | 2 / 1 | usb-c metal -Y/+X；grove #e4e5dc -Y/+X | grove:signal2,signal1,vcc,gnd | 外观插口估计；未实机/机械配合验证 |
| [m5stack-k034](https://docs.m5stack.com/en/core/Tough) | 0 / 0 | 未建插口模型 | 禁用（未核实） | Waterproof cable/gland/contact geometry is missing. Internal extension Grove ports are not exterior case sockets; no cable connection drawn. |
| [m5stack-k016-p](https://docs.m5stack.com/en/core/StickC-Plus) | 2 / 1 | usb-c metal -Y/+X；grove #e4e5dc -Y/+X | 禁用（未核实） | 外观插口估计；未实机/机械配合验证 |
| [m5stack-k010-aws](https://docs.m5stack.com/en/core/Core2_For_AWS) | 2 / 1 | usb-c metal -X/+Y；grove #9f2635 -X/+Y | 禁用（未核实） | 外观插口估计；未实机/机械配合验证 |
| [m5stack-k006-v27](https://docs.m5stack.com/en/core/M5GO_IoT_Kit_v2.7) | 2 / 1 | usb-c metal -X/+Y；grove-a #9f2635 -X/+Y | 禁用（未核实） | 外观插口估计；未实机/机械配合验证 |
| [m5stack-k128](https://docs.m5stack.com/en/core/CoreS3) | 2 / 1 | usb-c metal -X/+Y；grove-a #9f2635 -X/+Y | 禁用（未核实） | Rejected grove-a on -Y; exact side image and CAD cutout put it on -X next to USB. |
| [m5stack-k010-v13](https://docs.m5stack.com/en/core/Core2_v1.3) | 2 / 1 | usb-c metal -X/+Y；grove #9f2635 -X/+Y | 禁用（未核实） | 外观插口估计；未实机/机械配合验证 |
| [m5stack-c122-b](https://docs.m5stack.com/en/core/Stamp-C3U) | 1 / 0 | usb-c metal -Y/+X | 禁用（未核实） | 外观插口估计；未实机/机械配合验证 |
| [m5stack-k151](https://docs.m5stack.com/en/StackChan) | 2 / 1 | usb-c metal -X/+Y；grove-a #9f2635 -X/+Y | 禁用（未核实） | Rejected socket rotation with local width alongZ; exact side photo requires width alongY. |
| [m5stack-k138](https://docs.m5stack.com/en/app/StampFly) | 0 / 0 | 未建插口模型 | 禁用（未核实） | Whole drone source CAD is unavailable except battery-protection piece. Exposed USB and two Grove connectors are recorded in product metadata but not modeled as verified ports; no cable connection drawn. |
| [m5stack-k001-v27](https://docs.m5stack.com/en/core/Basic_v2.7) | 2 / 1 | usb-c metal -X/+Y；grove #9f2635 -X/+Y | 禁用（未核实） | 外观插口估计；未实机/机械配合验证 |
| [m5stack-k048](https://docs.m5stack.com/en/core/CoreInk) | 2 / 1 | usb-c metal -Y/+X；grove #e4e5dc -Y/+X | grove:signal2,signal1,vcc,gnd | 外观插口估计；未实机/机械配合验证 |
| [m5stack-k122](https://docs.m5stack.com/en/core/Stamp-C3U_Mate) | 1 / 0 | usb-c metal -Y/+X | 禁用（未核实） | 外观插口估计；未实机/机械配合验证 |
| [m5stack-k134](https://docs.m5stack.com/en/core/DinMeter) | 3 / 2 | usb-c metal -X/+Y；grove-a #e4e5dc -Z/+Y；grove-b #e4e5dc -Z/+Y | 禁用（未核实） | Rejected single mirrored Grove draft. Exact rear photograph has two white sockets and two-pin green power terminal. Back camera up=-Y preserves physicalX; widths are alongY, outward normal-Z. |
| [m5stack-k129](https://docs.m5stack.com/en/core/Capsule) | 2 / 1 | usb-c metal -Y/+X；grove #e4e5dc -Y/+X | 禁用（未核实） | 外观插口估计；未实机/机械配合验证 |
| [m5stack-k132-v11](https://docs.m5stack.com/en/core/Cardputer_v1.1) | 2 / 1 | usb-c metal +X/+Y；grove #e4e5dc -X/+Y | 禁用（未核实） | 外观插口估计；未实机/机械配合验证 |
| [m5stack-c126-echo](https://docs.m5stack.com/en/core/Atom_VoiceS3R) | 2 / 1 | usb-c metal -Y/+X；grove #e4e5dc -Y/+X | grove:signal2,signal1,vcc,gnd | 外观插口估计；未实机/机械配合验证 |
| [m5stack-c008-b](https://docs.m5stack.com/en/core/Atom-Matrix) | 2 / 1 | usb-c metal -Y/+X；grove #e4e5dc -Y/+X | 禁用（未核实） | 外观插口估计；未实机/机械配合验证 |
| [m5stack-k010](https://docs.m5stack.com/en/core/Core2) | 2 / 1 | usb-c metal -X/+Y；grove #9f2635 -X/+Y | 禁用（未核实） | 外观插口估计；未实机/机械配合验证 |
| [m5stack-k149](https://docs.m5stack.com/en/core/CoreS3_Thread_BR) | 4 / 3 | usb-c metal -X/+Y；grove #9f2635 -X/+Y；grove-b #242a27 +Y/+X；grove-c #167e9c +Y/+X | 禁用（未核实） | 外观插口估计；未实机/机械配合验证 |
| [m5stack-c148](https://docs.m5stack.com/en/core/PowerHub) | 4 / 2 | grove-a #9f2635 +Z/+Y；grove-c #167e9c +Z/+Y；usb-c-output metal +Z/+Y；usb-c-input metal -Y/+X | 禁用（未核实） | Rejected optional battery as recolorable shell. C148 without battery omits source component6; native assembly oriented to match exact front image. |
| [m5stack-s013](https://docs.m5stack.com/en/core/Stamp-P4) | 0 / 0 | 厂家CAD：USB-C, MIPI CSI BTB, SDIO BTB | 禁用（未核实） | Native STEP includes USB and BTB mechanical geometry; no fabricated PCB or shell. Copper stackup, exact user revision and electrical behavior remain unknown. |
| [m5stack-k001](https://docs.m5stack.com/en/core/Basic) | 2 / 1 | usb-c metal -X/+Y；grove #9f2635 -X/+Y | 禁用（未核实） | Matching bottom battery-base geometry and its expansion-pin detail are unavailable.；Source glass/LCD was separated3.294mm from frame. Explicit estimated rigid seating corrected; missing battery-base CAD remains a dimension/coverage failure. |

颜色的十六进制值是照片参考用色，未经光学标定。红/蓝/黑/白端子不会随外壳金色或半透明设置改变。口内阴影、针脚及插头独立；是否画线由逐口证据控制。

复现：`python tools/onboard-lane-02-port-audit.py`。截图和逐型号尺寸/失败记录分别位于 `docs/platform/evidence/<product-id>/`。
