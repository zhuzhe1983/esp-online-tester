# K159 / C156 的有限共享主机接入

K159 Assembly Kit 已接入 `paperdiy`，C156 StackChan Core 已接入 `cores3`。两款仍为 `partial`，各自保留产品、固件、默认选择和存储身份，没有增加 `firmwareTargetId`。本次复用的是已经实现的实际主机数字通路，没有创建空板型或绑定整机动画。未改 Rust，也未另建 WASM。

| 产品 | 可复用的依据 | 实施范围与差异 |
| --- | --- | --- |
| K159 / K159-B | 两个精确官方页指向相同的 `K159-B_PaperDIY_sche.pdf`，字节摘要相同；同为 16MB Flash / 8MB PSRAM、ET047TC1、M5PM1、G4/G3 按键、独立 SPI TF | 开放套件用现有 `paperdiy` 的黑白数字显示、电源寄存器与 TF 子集。外壳及装配状态不同，不能沿用 Complete Kit 外壳键。 |
| C156 / CoreS3 V1.0 主机 | C156 官方原理图直接指向 `Sch_M5_CoreS3_v1.0.pdf`，与 K128 保存的摘要相同；固定 M5Unified / M5GFX 的 CoreS3/StackChan 主机总线设置吻合 | 复用 ILI9342C、FT6336U、AW9523B、AXP2101、RTC/虚拟 IMU 与 SPI TF 子集。独立替换主控没有底座的头顶触摸板、RGB 或舵机，因此使用 `cores3`。 |

C156 官方记录 2026-08-07 开始使用 ILI9342E。固定 M5GFX 存在 C/E 不同初始化与读回探测，不能将相同屏幕引脚当作 E 批次通过；本次保持既有 CoreS3 的 ILI9342C 范围。官方 StackChan App 还要求原装主控和专用用户固件，不能将 CoreS3 的产品或出厂身份赋给 C156。

[来源、原理图摘要和逐项接线对照](evidence/m5-shared-host-runtime-20261126/sources.json)固定了 M5Unified `2fe93c0c`、M5GFX `02107b82` / `7cb26b44`、PaperDIY UserDemo `60a26f0e` 的 HAL，以及 StackChan BSP `e2f3b02d`。PaperDIY 的通用驱动 OE G45 与原理图实际 XOE 电源网区别沿用既有实现，不修改为猜测的 GPIO 电源门。

K159 既有 PCB 照片模型已包含侧向 A/B 轻触开关；本次只在其朝外的实体 actuator 面增加输入命中面，并让其随原 PCB 查看分组移动。几何二进制、照片和材质不变。Node 实际加载检查两处最近射线命中、屏幕可见性、分组复位与非外壳材质隔离；没有新增照片叠图或 GPU 外观验收。

[真实 WASM 回执](evidence/m5-shared-host-runtime-20261126/wasm.json)使用新编写的原创 SRAM 诊断程序，经公开 runtime、生产 Worker 和已有 WASM 执行：K159 的两只真实 GPIO 按键分别让客体通过 LCD_CAM/GDMA 扫描 540 行并改变黑白像素；C156 的 FT6336U 接触状态由客体 I²C 读取后，经物理 SPI 路由改变显示像素。两款同时执行未编程电源、错误时钟引脚负例，均不能产生测试像素；C156 未出现机器人舵机能力或状态。程序不读主机 framebuffer 地址、不使用 ROM stub，也没有用诊断程序冒充出厂固件。

```sh
ESP32SIM_WASM=<built-esp32sim.wasm> ESP32SIM_ROM_DIR=<official-rom-folder> \
  node --test tools/platform-m5-shared-host-runtime-test.mjs
node tools/check-m5-missing-models.mjs m5stack-k159 m5stack-c156
python tools/build-m5-missing-models.py --output <separate-directory> m5stack-k159
```

检查结果另见[加载回执](evidence/m5-shared-host-runtime-20261126/loader.json)和[检查记录](evidence/m5-shared-host-runtime-20261126/checks.json)。原厂固件、UIFlow、小智、无线、音频、光学灰阶、模拟电源和实机表现没有继承本次诊断验证。默认 TF 卡保持 64MiB FAT32，按产品/固件隔离，已有私有数据不被配置覆盖。
