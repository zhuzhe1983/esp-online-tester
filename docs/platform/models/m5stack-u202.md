# Unit C6L U202 双目录入库

控制器 `m5stack-u202` 与配件 `m5stack-unit-c6l-u202` 共用厂家模型和知识记录。官方资料明确 ESP32-C6、16 MiB Flash、SX1262、64×48 SSD1306 OLED、WS2812C、蜂鸣器与 IO 扩展器按键；Grove 黑/红/黄/白分别为 GND/5V/G5/G4。官网称 PORT.A，背贴标 PORT.X，均保留。原理图、datasheet、尺寸图及实际来源抓取时间见 [知识记录](../../../web/platform/knowledge/m5stack-u202.json)。

厂家 STL 固定 M5_Hardware `e29c139cbc7c80b5ccc5557e5ab8025a7a19a109`。组件 0/3/5 已装配，保留原始三角形和尺寸；1/2/4 是分开放置的打印副本，未重复展示。仅做刚体居中，不缩放。独立尺寸图标注壳体24×56×8 mm，加两只 SMA 的外形高度62 mm；壳体三轴误差均小于0.05 mm。

正反面纸贴分别从官方照片裁切，独立薄面不覆盖灰色外壳、OLED、孔洞和端子。OLED为无内容屏幕，照片里的 Meshtastic Logo 不充当模拟输出。内部只放10×30×0.8 mm的保守 `UNKNOWN` PCB占位；尺寸与位置为腔体适配估算，不是真实PCB数据。

SMA螺纹、Grove/USB和纸贴安装位置为尺寸图/照片近似；未导入鞭状天线，未精确重建螺钉头和背面M5浮雕。外形对照不能证明这些细节1:1，不能用来制造或校验连接器公差。正反面固定相机叠图与误差见 [验证记录](../evidence/m5stack-u202/verification.json)，三种材质及近侧面单独检查。板级运行和外接主机协议尚未实现。

可离线复建，不依赖临时研究目录：

```sh
python tools/rebuild-product-model.py plan m5stack-u202
python tools/rebuild-product-model.py build m5stack-u202 --scratch output/c6l-rebuild
```

Meshtastic 的设备映射和默认恢复入口见 [固件说明](../meshtastic-watchlist.md)。
