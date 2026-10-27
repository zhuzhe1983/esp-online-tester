# Atom-Matrix C008-B 研究检查点

原版 Atom-Matrix 采用 ESP32-PICO-D4、4 MB Flash；5×5 WS2812C-2020 在 G27，按键 G39，MPU6886 位于 SDA25/SCL21、地址 0x68。官网 IMU 轴图以 USB 朝下时 X 右、Y 上、Z 出正面；不能套用新版 v1.1 的传感器或其他 Atom 引脚。

官方 Factory Test v1.0 完整镜像 SHA-256 `24892fcf5d5f8c97ee30a5ffb4fec166dca1c254cc6f54861af28abc8732ba2f`，本轮已下载复核，未提交二进制。官方 UIFlow1/2 固定条目已有目录映射，继续保留出厂第一项。来源时间、地址、原始 hash 与 M5Atom 固定源码见 [sources.json](evidence/atom-matrix/sources.json)。

拟复用经典 ESP32 GPIO/RMT 和现有 MPU6886 轮询寄存器子集；必须在真实官方镜像中观察输出、输入后才启用目录。FIFO、中断、物理无线、红外和外部 Grove 夹具不预先宣称兼容。
