# PaperDIY Complete Kit K159-B：并行电子纸适配记录

K159-B 已启用 `paperdiy` 部分板级模型。未修改的官方 User Demo v1.0 经 ROM 启动后，能显示出厂图和主界面，A/B 按键能改变固件绘制的边框；真实页面 worker、浏览器 3D 点按均已检查。当前支持黑白电子纸、M5PM1 基础电源寄存器、A/B 按键、SPI TF 的 FAT32 读写和 Wi-Fi 虚拟热点扫描，不代表整机所有功能已经兼容。硬件目录最后改动时间 2026-10-19 是队列排序依据，不是产品上市日期。

## 固定来源

[来源收据](evidence/paperdiy/sources.json) 记录实际访问的厂家产品页、出厂固件分享、固定版本下载和源码，以及 ESP-IDF 5.5.1 LCD_CAM / i80 驱动头文件。厂家 M5PaperDIY-UserDemo 提交 `60a26f0ebccc656027bde413a0e9356aae5bb6e7` 使用 IDF 5.5.1，固定 M5GFX `02107b828acb3eb782fccd747863638d64c876d7`、M5Unified `2fe93c0c64f12f916e73a0605c86bf1241c06a6b`。

官方 Burner 分享 UVVZ26 的项目 2110309776206622722、版本 2110309776210817025 明确是 PaperDIY User Demo v1.0，发布时间 2026-10-14T18:00:35+08:00。下载镜像 1,404,800 字节，SHA-256 `d7de13e62342c1ab7115897a4a731abb054efda8ecb01d711aeb66c8bc6baa37`。这是完整合并镜像：0x0 引导程序、0x8000 分区表、0x10000 应用；不能当作 app-only 从文件起点直启。首次错误使用 app-direct 的探测停在 Simcall / 0xffffffe0 写入，改为正确 ROM 引导后通过。网站只记录远程地址与摘要，不发布镜像副本。

官方 `AppShutdown::handleFactoryBoot()` 在空白 NVS 时显示出厂图、设置 `paperdiy/factory_done` 后发送关机命令。因此首次运行停在出厂图是原固件行为。保留设备存储并点击「重启」即可进入主测试界面；清空设备存储会再次经历出厂流程。自动探测确认首次新增一个持久化页，重启后不再执行首次出厂分支。主机重启不能等同于真实 M5PM1 电源键。

## 接线与实施边界

ESP32-S3-WROOM-1-N16R8，16 MB Flash、8 MB OPI PSRAM；ET047TC1 原生 960×540，产品竖屏 540×960。纸屏数据 D0..D7 为 G6/14/7/12/9/11/8/10，XCL G16、XLE G15、XSTL G13、CKV G18、SPV G17、BST_EN G46。原理图显示 XOE 接 EPD_PWR，G45 是 FPC EXT_G45；不能因为通用驱动配置 OE G45 就错误地用它阻止纸屏输出。M5PM1 的 PYG2 控制纸屏电源，I²C SDA41/SCL42，地址 0x6e。按键上方 A G4、下方 B G3，电源键由 M5PM1 处理，不能虚构为 GPIO。官方产品不含触摸屏。

屏幕使用 LCD_CAM i80 + GDMA trigger 5，不是 PaperMono 的 SSD1677 SPI。实现从客体 DMA 描述符读取并行数据，经 GPIO 矩阵检查物理路由，再由 SPV/CKV/XLE 和电源状态决定是否更新像素，没有读取客体 framebuffer 地址或识别固件画面。每行前 240 字节承载 960 个两位驱动命令，其后八字节为驱动填充；1/2 写入黑/白，0/3 保持。模型保留掉电后的纸屏图像。

i80 是有限数字事务模型：在 LCD_USER START 写入时完成已准备好的 GDMA 链，支持命令/数据、8/16 位和驱动使用的交换顺序、完成中断及无数据初始事务。需要先准备 GDMA 再 START；尚无反向启动顺序补触发、部分描述符续传、像素时钟/FIFO 延迟或 16 灰阶光学响应。描述符循环、越界、MMIO、错误长度及启用 owner 检查后的 CPU-owned 链不会输出部分纸屏画面；分配量另限 1 MiB。沿用共享 GDMA walker；5.5.1 的 owner、writeback、OUT 中断位与已使用的 5.5.5 一致。

TF 卡通过独立 SPI 路由接入：MISO40、CS47、SCK39、MOSI38、低有效检测48，不走 SDMMC 插槽或 FPC 的 CS21。原理图的 U5 由 M5PM1 DCDC3V3_EN_PP 控制，输出 3V3_L2 给 J2 卡槽；关闭该电源清除卡协议状态并保留非易失数据。默认卡使用共享的浏览器持久存储，按产品、板型和初始 Flash 摘要隔离。

固定 UserDemo 的首次扫描及 A 键重新扫描均找到模拟器的一个虚拟热点，原始屏幕显示 `esp32_online_tester`；这仅验证扫描流程，不是外网、射频或真实热点连接验收。FPC 扩展、灰阶光学响应和真实电源睡眠仍不完整。UIFlow2 保持试运行，未继承此 UserDemo 验收。

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

## 2026-11-13：TF 读写与下载入口修复

官方 v1.0 的历史 CDN 地址仍提供与原始固定摘要相同的 1,404,800 字节镜像，并允许浏览器 CORS 读取，目录已切换至该固定地址；仍核对 SHA-256，不依赖验证码下载接口返回的 JSON。官方公开版本列表当前仍为 v1.0。[来源与供电依据](evidence/paperdiy/tf-20261113/sources.json)保留下载与源代码的摘要。

[真实 WASM 收据](evidence/paperdiy/tf-20261113/factory-wasm.json)验证了：默认 64 MiB FAT32 挂载、B 键触发官方 1 MiB 写入和逐字节读回校验、A/B 按住/释放边框反馈、A 键重新扫描，以及保留 NVS 和卡镜像的重启/重新加载。测试独立解析导出的 FAT32，检查 2,048 个数据扇区均符合官方 `(i * 31 + 17) & 255` 模式，临时文件已删除、两份 FAT 都释放对应簇；重启和新 worker 恢复的整卡 SHA-256 完全相同。真实 framebuffer 的 WRITE/READ 栏显示数值而非 TEST FAILED。

官方测试程序会删除自己的临时文件；镜像中保留的扇区内容不是用户可见文件仍存在的声明。显示的 MB/s 由模拟客体时间算出，不代表真实卡速。此自动探测使用内存后端检验真实客体读写与 worker 生命周期，浏览器持久存储和导入/清空还由平台存储测试覆盖。SPI 为即时数字协议子集，无物理卡、模拟电压和断电写入保证。

本次重新升级 stable 至 Rust 1.100.0，双 Clippy、1,970 个完整 release 测试、111,126 个 WASM JIT 对照案例和 24 个 GPIO 波形案例通过；相关平台测试 24 项和目录 schema 通过，黄金文件未变。初次执行缺少 CI 要求下载的 P4 固件输入，补齐固定摘要文件后重跑通过；新版 Clippy 报出的既有循环警告在独立提交中做等价修复。[本轮检查及局限](evidence/paperdiy/tf-20261113/checks.json)与[复现说明](evidence/paperdiy/tf-20261113/README.md)保留这些结果。

对干净 main 的[三对交替 CPU 回归](evidence/paperdiy/tf-20261113/cpu.json)：S3 hello 0.22→0.22 秒；C3 2.41→2.47 秒（+2.49%）；C6 1.52→1.53 秒（+0.66%）；Pocket Tank 39.46→39.21 秒（−0.63%）。每次均运行 30 客体秒，24 次的对应目标 UART 和指令数完全相同。样本小、时间分辨率为 0.01 秒，不据此宣称加速或严格零开销。
