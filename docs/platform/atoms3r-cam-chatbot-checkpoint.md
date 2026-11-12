# K147-CAM 未完成检查点

基线为 `68038e52`。当前目录仍为 unavailable，不算已交付适配；这是传感器配置阶段的代码。

已实际抓取 K147-CAM / C126-CAM 官方产品页、主板与摄像头子板原理图、GC0308 datasheet、固定 esp32-camera v2.1.4 驱动和 IDF v5.5.1 GPIO/LCD_CAM 头文件。来源时间、地址、字节和 hash 见 [sources.json](evidence/atoms3r-cam-chatbot/sources.json)。API 原始回复仅放忽略目录，公开收据不保留作者/账号字段；固件二进制不入库。

新增的未交付代码实现 GC0308 两页 SCCB 寄存器存储、只读 chip ID 0x9b、软件复位、G18 低有效供电/NACK；BMI270 独立 SDA45/SCL0，摄像头 SDA12/SCL9。板型 `atoms3r-cam-chatbot` 不带假显示屏或 G41 按钮，音频底座 ES8311 尚未挂载。`Gc0308State` 当前只有配置寄存器子集，不实现图像信号处理或采集。

两个定向测试通过：`cargo test --release -p esp-soc gc0308 --lib` 和 `cargo test --release -p esp32s3 atoms3r_cam --lib`；`cargo build --release -p esp32sim --bin esp32sim` 通过。全量 Rust、双 Clippy、WASM、浏览器和性能尚未进行，不可把此提交作为合并验收。

首次原生 UIFlow2 / 出厂镜像探测无效：资源复制到了 `web/wasm/fw/fw/`，指定的上一层 ROM 路径不存在，客体只在无 ROM 异常循环。该结果丢弃，未据此修改固件或芯片；接续时先修正资源位置/使用实际 ROM 路径并复核 hash。另一次构建曾误用包名 esp32s3 的不存在 bin，已改用 esp32sim 包并成功。

后续实现范围：先观察未经修改的 UIFlow2 与出厂完整镜像；按 GC0308 datasheet 与固定驱动实现真实输出格式、窗口和供电；S3 LCD_CAM 活跃路径需要显式校验新板声明的 DVP 各线和 XCLK，不能套用 OV5640。相关历史机制为 EX212 的 VSYNC/帧 DMA 和 EX222 的复位合同；保留旧 Waveshare 行为，验证错引脚/停时钟/断电/复位负例并做必要 CPU 回归。K147-CAM 无屏，不能借用 K147 GC9107。主机 UVC 出厂固件不自动认定为整套语音套件出厂固件。
