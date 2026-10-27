# NanoC6 C125 适配检查点

固定官方产品页、v0.0.1 原理图、M5NanoC6 `7cf50969` 源码与出厂 v1.1 镜像，见 [sources.json](evidence/nanoc6/sources.json)。已核对 ESP32-C6FH4、4 MB Flash、G9 低有效按键（R2 10 kΩ 上拉）、G20 的单颗 WS2812、G19 高有效 RGB 电源开关、G7 高有效蓝灯；G3 红外和 Grove G1/G2 分开记录，不套用 NanoH2。

上游开放 PR [#262](https://github.com/mikroverk/esp32sim/pull/262) 已提供 C6 板型文件拆分，先复用其首个源码提交并保留作者，冲突合并保留本 fork 的 P4 和 StopWatch 路径；不导入后续测试二进制或 CI demo。本阶段还未启用 NanoC6 运行状态，下一步实现按键、RGB 电源门控和蓝灯，观察真实官方固件输出后再交付。
