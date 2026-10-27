# PaperDIY Complete Kit K159-B：并行电子纸适配记录

K159-B 已启用 `paperdiy` 部分板级模型。未修改的官方 User Demo v1.0 经 ROM 启动后，能显示出厂图和主界面，A/B 按键能改变固件绘制的边框；真实页面 worker、浏览器 3D 点按均已检查。支持范围为黑白电子纸、M5PM1 基础电源寄存器和两个按键，不代表整机所有功能已经兼容。硬件目录最后改动时间 2026-10-19 是队列排序依据，不是产品上市日期。

## 固定来源

[来源收据](evidence/paperdiy/sources.json) 记录实际访问的厂家产品页、出厂固件分享、固定版本下载和源码，以及 ESP-IDF 5.5.1 LCD_CAM / i80 驱动头文件。厂家 M5PaperDIY-UserDemo 提交 `60a26f0ebccc656027bde413a0e9356aae5bb6e7` 使用 IDF 5.5.1，固定 M5GFX `02107b828acb3eb782fccd747863638d64c876d7`、M5Unified `2fe93c0c64f12f916e73a0605c86bf1241c06a6b`。

官方 Burner 分享 UVVZ26 的项目 2110309776206622722、版本 2110309776210817025 明确是 PaperDIY User Demo v1.0，发布时间 2026-10-14T18:00:35+08:00。下载镜像 1,404,800 字节，SHA-256 `d7de13e62342c1ab7115897a4a731abb054efda8ecb01d711aeb66c8bc6baa37`。这是完整合并镜像：0x0 引导程序、0x8000 分区表、0x10000 应用；不能当作 app-only 从文件起点直启。首次错误使用 app-direct 的探测停在 Simcall / 0xffffffe0 写入，改为正确 ROM 引导后通过。网站只记录远程地址与摘要，不发布镜像副本。

官方 `AppShutdown::handleFactoryBoot()` 在空白 NVS 时显示出厂图、设置 `paperdiy/factory_done` 后发送关机命令。因此首次运行停在出厂图是原固件行为。保留设备存储并点击「重启」即可进入主测试界面；清空设备存储会再次经历出厂流程。自动探测确认首次新增一个持久化页，重启后不再执行首次出厂分支。主机重启不能等同于真实 M5PM1 电源键。

## 接线与实施边界

ESP32-S3-WROOM-1-N16R8，16 MB Flash、8 MB OPI PSRAM；ET047TC1 原生 960×540，产品竖屏 540×960。纸屏数据 D0..D7 为 G6/14/7/12/9/11/8/10，XCL G16、XLE G15、XSTL G13、CKV G18、SPV G17、BST_EN G46。原理图显示 XOE 接 EPD_PWR，G45 是 FPC EXT_G45；不能因为通用驱动配置 OE G45 就错误地用它阻止纸屏输出。M5PM1 的 PYG2 控制纸屏电源，I²C SDA41/SCL42，地址 0x6e。按键上方 A G4、下方 B G3，电源键由 M5PM1 处理，不能虚构为 GPIO。官方产品不含触摸屏。

屏幕使用 LCD_CAM i80 + GDMA trigger 5，不是 PaperMono 的 SSD1677 SPI。实现从客体 DMA 描述符读取并行数据，经 GPIO 矩阵检查物理路由，再由 SPV/CKV/XLE 和电源状态决定是否更新像素，没有读取客体 framebuffer 地址或识别固件画面。每行前 240 字节承载 960 个两位驱动命令，其后八字节为驱动填充；1/2 写入黑/白，0/3 保持。模型保留掉电后的纸屏图像。

i80 是有限数字事务模型：在 LCD_USER START 写入时完成已准备好的 GDMA 链，支持命令/数据、8/16 位和驱动使用的交换顺序、完成中断及无数据初始事务。需要先准备 GDMA 再 START；尚无反向启动顺序补触发、部分描述符续传、像素时钟/FIFO 延迟或 16 灰阶光学响应。描述符循环、越界、MMIO、错误长度及启用 owner 检查后的 CPU-owned 链不会输出部分纸屏画面；分配量另限 1 MiB。沿用共享 GDMA walker；5.5.1 的 owner、writeback、OUT 中断位与已使用的 5.5.5 一致。

TF 卡是 SPI：MISO40、CS47、SCK39、MOSI38、检测48；当前报告无卡，不能直接套用已有 SDMMC 插槽。FPC 扩展、真实电源睡眠、无线通信仍需独立实现或验收。Wi-Fi 射频校准日志报错、无网络结果是已知边界，不代表能联网。UIFlow2 可自行尝试，但本轮未验证。

厂家 CAD 原有上/下按键语义颠倒，本轮按照官方照片修正为上 A / 下 B，并绑定真实 GPIO 输入。非触摸屏的屏幕区域现在可拖动旋转，避免误报不支持触摸。金色和紫色半透明外壳、绿色按钮提示及 2D/3D 画面检查通过；七个厂家模型/照片/几何产物重建字节一致。屏幕开口上方可见内部凸台、开口间隙和已有估算部件属于保留偏差，仍需未来照片叠图修正；本轮不宣称新的 1:1 外观验收或 PCB 数据完整。

## 共享实现检查

本轮读取开放 PR 列表，未见 LCD_CAM i80 / PaperDIY 重复实现。已有 SPI 服务和 SPI1 工作不复用或覆盖。实验记录未有相同并行接口实现；这是缺失器件行为，不是重试 CPU 加速实验。新增通路只在 LCD_USER MMIO 写入触发，没有增加 idle tick；Board 方法放在 trait 末尾，保持现有热路径 vtable 顺序。RGB / camera 路径不变。

## 验收与复现

[官方 WASM 收据](evidence/paperdiy/factory-wasm.json)、[检查与浏览器收据](evidence/paperdiy/checks.json)、[模型重建收据](evidence/paperdiy/model-rebuild.json) 保存输入、产物摘要和范围。测试使用真实未修改的官方镜像；浏览器从官方 API 跳转 CDN 加载，没有拦截为 fixture。A/B 各按住至少 0.9 客体秒后，官方边框像素变黑；释放并再运行至少 0.9 客体秒后变白。主机鼠标输入不是周期精确注入，也未使用真机验证。

```sh
tools/wasm-build.sh
node tools/probe-paperdiy-factory.mjs --url '<官方固定版本下载地址>' --sha256 d7de13e62342c1ab7115897a4a731abb054efda8ecb01d711aeb66c8bc6baa37
```

Rust stable 1.99.1 的双 Clippy、1433 个 Rust 测试、11 个 WASM 场景、111126 个 JIT 对照案例、100 个平台测试及 schema 检查通过，黄金文件没有变化。录制回放的 18 个真实 WASM 输入和生命周期检查通过，但该次最终帧为 `different`；本轮不以此宣称回放逐帧一致。详细命令和其他 CI 检查保留在收据中。未启用板型的三组 hello CPU 对照及指令/UART 摘要见 [CPU 收据](evidence/paperdiy/cpu.json)。

五对 CPU 中位数：S3 hello 0.24→0.24 秒；C3 2.29→2.36 秒（+3.06%）；C6 1.51→1.53 秒（+1.32%）。每组十次的 UART 和指令数完全相同。这是单机粗粒度对照，没有置信区间；保留小幅变慢结果，不声称严格零开销。S3 执行核心未改，此次未重复 pocket-tank。
