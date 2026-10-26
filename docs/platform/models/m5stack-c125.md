# M5Stack NanoC6 · C125

厂家 CAD 按毫米导入；状态为 partial。此型号尚无可运行的板级模拟器。

集成 ESP32-C6、RGB LED、红外发射、按键和 Grove 接口的微型开发板，支持 Wi-Fi 6、Thread 与 Zigbee。

装配：Select source components 0 and 2 in assembled positions; rotate +90 degrees around Z to Grove-up portrait orientation, preserving millimetres.

核查结果：2 个保留组件，2 个排除组件；5 项几何检查，0 项未通过。失败值保存在 [verification.json](../evidence/m5stack-c125/verification.json)。

浏览器照片核对：实际浏览器四视图确认 Grove 朝上方向，依据官方照片改为青色壳体；壳体尺寸检查通过。官方 CAD 未包含照片中白色 Grove 端子、USB 金属内芯和文字；当前 UNKNOWN PCB 通过开口可见，外观仍不完整，未标记 1:1 通过。

限制：

- Manufacturer CAD contains enclosure only; visible Grove/USB inserts and label details are not supplied as mechanical CAD.
- Exterior shell dimensions pass 0.1 mm tolerance; this does not certify absent connectors or populated board.
- No display exists on NanoC6; no fabricated screen or framebuffer is added.
- No complete board/peripheral emulator exists for this SKU; preview and documentation only.
- PCB placeholder has no components. Its dimensions/placement are an enclosure-space estimate, not a verified PCB layout.
- Manufacturer photo rights are separate from CAD MIT license. Materials are visual approximations, not measured reflectance.

来源与装配计划：[source-audit.json](../evidence/m5stack-c125/source-audit.json)。复现：`python tools/onboard-lane-03.py prepare --ids m5stack-c125`，然后 `python tools/onboard-lane-03.py build --ids m5stack-c125`；依赖 `tools/requirements-model.txt`。厂家 CAD 保留 MIT；官方照片未核实另有开放许可，不归入项目 MIT。
