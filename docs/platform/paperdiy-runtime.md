# PaperDIY Complete Kit K159-B：并行电子纸适配记录

当前为研究 checkpoint，目录仍未启用；尚无官方固件执行通过的结论。硬件目录最后改动时间 2026-10-19 是队列排序依据，不是产品上市日期。

## 固定来源

[来源收据](evidence/paperdiy/sources.json) 记录实际访问的厂家产品页、出厂固件分享、固定版本下载和源码，以及 ESP-IDF 5.5.1 LCD_CAM / i80 驱动头文件。厂家 M5PaperDIY-UserDemo 提交 `60a26f0ebccc656027bde413a0e9356aae5bb6e7` 使用 IDF 5.5.1，固定 M5GFX `02107b828acb3eb782fccd747863638d64c876d7`、M5Unified `2fe93c0c64f12f916e73a0605c86bf1241c06a6b`。

官方 Burner 分享 UVVZ26 的项目 2110309776206622722、版本 2110309776210817025 明确是 PaperDIY User Demo v1.0，发布时间 2026-10-14T18:00:35+08:00。下载镜像 1,404,800 字节，SHA-256 `d7de13e62342c1ab7115897a4a731abb054efda8ecb01d711aeb66c8bc6baa37`，仅保留远程地址；镜像布局仍待核对，不能仅凭 .bin 后缀认定完整 merge 镜像。

## 接线与实施边界

ESP32-S3-WROOM-1-N16R8，16 MB Flash、8 MB OPI PSRAM；ET047TC1 原生 960×540，产品竖屏 540×960。纸屏数据 D0..D7 为 G6/14/7/12/9/11/8/10，XCL G16、XLE G15、XSTL G13、CKV G18、SPV G17、BST_EN G46。M5GFX 还配置 OE G45；这项来自驱动，尚须原理图交叉核验。M5PM1 的 PYG2 控制纸屏电源，I²C SDA41/SCL42，地址 0x6e。按键 A G4、B G3，电源键由 M5PM1 处理，不能虚构为 GPIO。官方产品不含触摸屏。

屏幕使用 LCD_CAM i80 + GDMA trigger 5，不是 PaperMono 的 SSD1677 SPI。计划通过真实 DMA 描述符、GPIO 矩阵和并行波形更新屏幕，不读取客体 framebuffer 内存伪造输出。先覆盖黑白终态；16 灰阶电压波形、光学响应和周期精确扫描不能据此宣称实现。

TF 卡是 SPI：MISO40、CS47、SCK39、MOSI38、检测48；不能直接套用已有 SDMMC 插槽。FPC 扩展、音频、电源睡眠、无线通信仍需独立实现或验收。现有厂家 CAD 与原模型核验偏差保留，本轮不宣称重新完成 1:1 模型验收。

## 共享实现检查

本轮读取开放 PR 列表，未见 LCD_CAM i80 / PaperDIY 重复实现。已有 SPI 服务和 SPI1 工作不复用或覆盖。实验记录未有相同并行接口实现；这是缺失器件行为，不是重试 CPU 加速实验。新增冷 MMIO 通路后仍须按 AGENTS 比较同指令 hello CPU 时间，保持 RGB / camera 和未启用路径回归。
