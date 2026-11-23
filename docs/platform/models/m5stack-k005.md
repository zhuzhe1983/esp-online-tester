# M5Stack Faces Kit：资料入库，模型未完成

精确SKU目录只有DWG与CDR，已实际抓取并保留SHA-256/UTC。尚未验证完整装配3D或官方尺寸的作用域，不把相近Core模型或通用方盒当作Faces Kit。

- [知识记录](../../../web/platform/knowledge/m5stack-k005.json)
- [来源与未通过记录](../evidence/m5stack-k005/verification.json)

官方表格列58.2×54.2×18.7mm，但套件照片包含主机、键盘与底座；这个值尚不能作为整套包络。资料内Flash旧4MB/当前16MB的差异保留，用户硬件版本未知。

## Gray 主机模拟

工作台使用厂家列出的 16 MiB Gray 主机配置。ILI9342C 的屏幕输出、A/B/C 按键和 SPI TF 卡采用与经典 Basic 相同的板级路由。套件身份保留 K005，外部扩展仍以原始资料为准。 [Faces Kit](https://docs.m5stack.com/en/core/Faces_Kit) · [Gray 引脚表](https://docs.m5stack.com/en/core/Gray)

官方 Gray UIFlow2 v2.5.5 的 16 MiB 镜像可试运行。原始镜像在经典 ESP32 ROM 和 JIT 上进入 320×240 的 DEVELOP 页面，按住 GPIO39 对应的 A 键 300 毫秒后切换至 APP RUN。对照接入 GPIO34 与无输入的画面一致。浏览器默认准备 64 MiB FAT32 卡，这次菜单操作没有访问卡。 [运行记录与复现参数](../evidence/m5stack-k005/gray-host-runtime.json)

目录仍以原厂 EasyLoader 为首选参考。提取的原厂镜像在 4 MiB、16 MiB 两种虚拟 Flash 配置上均于 FreeRTOS 队列初始化处退出。UIFlow1 v1.15.5 则在屏幕绘制前触发除零重启。三款 I²C 0x08 键盘、底座、IMU、音频与电源状态未建模，未进行实机验证。 [固件来源与布局](../evidence/core-family/README.md)
