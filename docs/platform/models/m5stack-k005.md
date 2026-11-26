# M5Stack Faces Kit：部分分件 3D 预览

K005 现在提供独立套件模型：Gray 上部、A009 长电池底座、默认装上的 35 键 QWERTY，以及并排散放的 20 键 Calculator、8 键 Gamepad 和独立充电座。不是将 Basic 外壳改标签当作整个套件。三面板共有 63 个稳定物理键 ID；它们是 ATmega/I²C 面板外观，尚未接入 ESP32 的按键输入路径。现有 Gray 主机的三个可操作按键、显示、SD 与固件身份不变。

原厂精确 K005 Structures 目录（M5_Hardware `18de3bb3087ea617c38642ba3eedd10dc9f17cbf`）只有 DWG/CDR，均按实际抓取 hash 留存，未声称成功转换完整装配。Gray 上部明确复用 Core1 家族 CAD 的六个已装配网格，保留原始 STL、原始全局/局部定位和额外刚性平移；不复制 Gray 的独立底盖或出厂固件身份。A009、三种面板和充电座用本型号官方照片近似，屏幕照片没有覆盖实时 framebuffer。

- A009 官方标注为 108×54×19 mm；单块 Face 面板为 58.2×54.2×10.4 mm。
- 套件页 58.2×54.2×18.7 mm 的作用域仍未解决，不能作为整套装配尺寸。模型中的 198.2×246.1 mm 是散件展示包络。
- 面板外壳、键帽厚度、底座厚度和 Gray 装配 Z 为照片估计；充电座隐藏机制、电池线、接触针、螺钉与主板布局未补猜。内部留 UNKNOWN。
- QWERTY 白纸条是四个独立遮罩薄面；键顶照片只投到各自键帽正面，灰塑料和开孔没有当作白贴纸。纸、键帽、板与连接器不随外壳金色/半透明紫色修改。

离线重建：`python tools/build-m5-missing-models.py --output /tmp/m5-faces m5stack-k005`。哈希、照片像素裁切、UV、63 键位置和扫描引脚事实见 [model-plan.json](../../../web/platform/assets/m5stack-k005/model-plan.json) 与 [panel-key-mapping.json](../../../web/platform/assets/m5stack-k005/panel-key-mapping.json)。[本次 Node 检查](../evidence/m5-faces-rear-20261126/loader-checks.json) 覆盖实际生产 loader、全部键顶最近射线、9 点屏幕射线、展开复位和材质隔离；未完成 WebGL 浏览器、同视角照片叠图或实机外观验收。

## Gray 主机模拟

工作台使用厂家列出的 16 MiB Gray 主机配置。ILI9342C 的屏幕输出、A/B/C 按键和 SPI TF 卡采用与经典 Basic 相同的板级路由。套件身份保留 K005，外部扩展仍以原始资料为准。 [Faces Kit](https://docs.m5stack.com/en/core/Faces_Kit) · [Gray 引脚表](https://docs.m5stack.com/en/core/Gray)

官方 Gray UIFlow2 v2.5.5 的 16 MiB 镜像可试运行。原始镜像在经典 ESP32 ROM 和 JIT 上进入 320×240 的 DEVELOP 页面，按住 GPIO39 对应的 A 键 300 毫秒后切换至 APP RUN。对照接入 GPIO34 与无输入的画面一致。浏览器 2D 面板的 A 键也能触发同样的画面变化。通过同一固件的 UART0 MicroPython REPL，用 slot 2 挂载默认 64 MiB FAT32 卡，写入后读回测试文件，刷新页面后再次读回。slot 3 在该次探测中返回 ESP_ERR_NOT_FOUND。 [运行记录与复现参数](../evidence/m5stack-k005/gray-host-runtime.json)

目录仍以原厂 EasyLoader 为首选参考。提取的原厂镜像在 4 MiB、16 MiB 两种虚拟 Flash 配置上均于 FreeRTOS 队列初始化处退出。UIFlow1 v1.15.5 则在屏幕绘制前触发除零重启。三款 I²C 0x08 键盘、底座、IMU、音频与电源状态未建模，未进行实机验证。 [固件来源与布局](../evidence/core-family/README.md)

## 套件散件与装配复核

2026-11-26 再按保留的官方 `kit-photo-5` 核对：照片中的 Gray+QWERTY 已装配，Gamepad、Calculator 与充电座独立陈列。当前模型明确是整套陈列，未把这些散件的间距当成主机缺件或凭空挪入主机。侧壁和已装配主机/键盘的高度区间相交；这只支持陈列解释，不证明工程装配、接触针或隐藏紧固件的完整性。

原模型与全部位置保持不变。独立离线重建的模型、几何和纹理逐字节一致；生产 loader 的 63 键、主机 3 键、屏幕与 UNKNOWN 射线、材质隔离和展开复位通过。原色、金色和半透明紫色的真正 CPU 正反/斜视及最终资产哈希见[复核回执](../evidence/m5-faces-layout-review-20261126/receipt.json)。未完成校准照片叠图或实物配合验证。
