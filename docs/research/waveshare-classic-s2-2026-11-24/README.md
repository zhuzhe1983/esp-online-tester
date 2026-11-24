# Waveshare classic ESP32 / S2 五家族

本批包含 5 个商品家族、15 个精确 SKU。每个家族的集成合同位于 `tools/waveshare-batches/classic/`，中央目录由集成者更新。

| 家族 | SKU 数 | 真实数字子集 | 机械来源 |
|---|---:|---|---|
| Servo Driver with ESP32 | 1 | SSD1306 128×32 I²C、两颗 GPIO23 RMT RGB、BOOT0 | 原厂 STEP 全部253个拓扑对象 |
| ESP32-S2 | 4 | 独立S2 SRAM、8MiB数据PSRAM MMU、LCD款 GPIO SPI ST7735、BOOT0 | 官方21×51 /21×54毫米图与照片；排针款独立 |
| ESP32-DEV-KIT-WROxx | 8 | 四种精确模组存储、GPIO27 RMT RGB、BOOT0 | 官方53.8×27.94毫米图与逐模组/排针照片 |
| General Driver for Robots | 1 | QMI8658C、SPI SD启动命令、电机 GPIO输出观察、BOOT0 | 原厂 STEP 全部638个拓扑对象 |
| e-Paper ESP32 Driver Board V3 | 1 | 无面板；精确SPI/GPIO连接器输出观察、BOOT0 | 官方Rev3主板及随板FFC转接板尺寸/照片 |

## 真实性边界

General/Servo 的实体 Flash 密度未核实，runtime 的4MiB仅为显式虚拟配置。S2 的数据PSRAM不代表ROM、USB、Flash映射、硬件SPI或IDF启动完成。General没有板载OLED；电子纸驱动板没有随板显示面板，其BUSY信号不被伪造。

源码、固件和模型分别验收。S2包内TinyUF2字符串明确指向Espressif Saola1R WROVER；通用MicroPython WROVER镜像也是偏移包，均不冒充精确Waveshare原厂。EPD官方说明其内置驱动开源，保留factory来源参考但没有编造完整恢复BIN。其余厂家示例保留项目参考。UIFlow2 2.5.5的58个资产、小智v2.6.0的200个资产及M5Burner55个设备分类均做了精确名称调查，未把同芯片或其他Waveshare目标自动映射给本批SKU。

## 验收与复现

`batch-manifest.json`记录提交依赖、五家族边界与数量。各家族`runtime-verification.json`记录真实客体测试、失败修正和范围；`firmware-investigation.json`记录来源、原包成员调查及目标身份。

模型可用已有`python tools/rebuild-product-model.py plan <product-id>`或直接根据资产内`model-plan.json`与hash匹配原件重建；`tools/build-waveshare-models.py`保留原坐标、比例及全部指定拓扑。`models/loader-checks.json`来自实际生产Three加载器，包括源hash、尺寸、按钮前后遮挡、屏幕射线与UNKNOWN标签。四视图是几何审查图，不是运行界面；来源叠图单独保留，未知高度、模块包络、光学AA与背面布局不称为精确还原。General官网JPG请求403，只与原厂2023尺寸图对照，不声称照片验收。

S2基准`esp32s2/examples/sram_benchmark.rs`只使用旧新两侧共有的SRAM指令循环，默认40,000,000条指令，校验值10,000,000。基线与候选使用同一源码、独立target目录和ABBA交替；本批没有编造新PSRAM的相对性能。
