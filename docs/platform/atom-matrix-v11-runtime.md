# Atom-Matrix v1.1 C008-B-V11 运行适配

`atom-matrix-v11` 按具体版次实现经典 ESP32-PICO-D4、4 MB Flash、G39 按键、G27×25 RGB，以及 SDA25/SCL21 上的 BMI270 `0x68`。原版 C008-B 的 MPU6886 板型保持独立。官方 UIFlow2 v2.5.3 原始完整镜像已从 ROM 启动至 MicroPython REPL，启动程序实际输出25颗蓝灯；目录保持 `partial/trial`，不是整机兼容认证。

## 固件来源与默认项

[官方 v1.1 产品页](https://docs.m5stack.com/en/core/Atom-Matrix_v1.1)明确链接共享 Atom-Matrix UIFlow1/2 教程。固定 UIFlow2 项目 `2099000000000000002`、设备 `2093000000000000009`、版本 `2098279897621549057`；原始镜像4,186,112字节，平台下载计算 SHA-256 `1591382c9b46f10549faa0053b4036aa55f5c8131867b2f039d2650982e7886d`。抓取时间、最终地址和摘要见 [sources.json](evidence/atom-matrix-v11/sources.json)。二进制只在验收时临时下载，不进入 Git 或站点。

本轮未找到明确适用于 v1.1 的出厂镜像，也未确认该具体版次的官方小智项目。UIFlow2 是默认的官方开发固件，角色仍为 `alternative`；旧 C008-B 出厂镜像没有套用到 BMI270 版。UIFlow1 已按产品页链接加入映射并允许试运行，本轮未执行。watchlist 保留项目/设备/版本身份，新版本不继承本次验证。

## 输入和硬件修复

- PICO-D4 的包型 eFuse 在该板型挂载时设置为5，寄存器为 `BLK0_RDATA3[11:9]`。IDF5.5.1 将字段命名为 `CHIP_PACKAGE`，IDF4.4.3 称 `CHIP_VER_PKG` 并定义 PICO-D4 值5；位定义一致。通用 D0WD 和其它板型保持原状，没有在逐指令或逐 tick 路径增加判断。固定 M5Unified `368ad608` 源码说明包型如何参与识别；它只作为机制证据，不能称为该固件的已核对构建源码。
- 修正共享 BMI270 的 `ACC_RANGE` 复位值：Bosch BST-BMI270-DS000-08 §5.2.42（p102）明确为 `0x02`（±8g），旧模型误用 `0x01`（±4g）。旧默认量程单测预期随此修正，并新增软复位、显式4g量程和1g计数验证，不伪造官方驱动返回值。
- 原始 UIFlow2 经 REPL 调用 `M5.begin/update`，`M5.BtnA` 实际返回松开0→按下1→松开0。3D盖板点击和键盘 A 通过同一路径。REPL表达式是明确发送给官方解释器的测试输入，不是固件补丁，也不冒充官方出厂交互流程。
- 以姿态输入把支撑加速度从 Y 轴转到 X 轴，`M5.Imu.getAccel()` 从 `(0,1,0)` 变为 `(1,0,0)`；官方 MicroPython 硬件 I²C 读取 `ACC_RANGE=2`、原始 X=4096，符合8g量程。修改前出现的 `(2,-1,0)` 及4g寄存器值保留为负面证据。

## 仍未覆盖的部分

BMI270 沿用现有数字轮询子集：标识、量程、软件复位、完整8192字节配置写入门槛和突发采样。配置里的特征代码不执行；FIFO、中断、ODR、模拟惯性/噪声均未实现。软件 `SoftI2C` 试读返回 ENODEV，不能声称软件位操作通路通过；实际验证的是硬件 I²C 和官方 M5 驱动。原始官方程序仍有 XTAL估计与 I²C未初始化提示，原文保留在临时日志，公开证据只摘取有关结果。

红外、模拟供电、真实 Wi-Fi/BLE、Grove外设和完整官方应用流程未验证。没有实机测试。官方 MicroPython 不含 `neopixel` 模块，本轮不拿另一个RGB程序替代官方验证；25蓝灯确为其启动代码的输出。

## 模型核验

保留厂家 CAD 的原始几何与装配变换。原图 x306…897 对应24mm外壳，首末灯 x388…781 对应约3.99mm间距；采用4mm间距、2mm发光区，估计误差±0.3mm。链序按该版官方 `Light-num.jpg` 从左到右、从上到下，IMU轴按官方图 X右/Y上/Z出正面。发光区域是扩散盖板上的光学近似，不是浮在盖板上的实体LED封装，也不复制照片的彩虹颜色。

正面同视角叠图、背面、侧面、原色/金色/紫透及盖板悬停已经检查。正面轮廓和灯位可对照；侧面连接器、孔洞背衬和配色仍是近似，内部 PCB 保持 `UNKNOWN`，不能当作完整内部CAD。厂家壳体13.8mm与文档13.9mm的差异保留，未缩放消除。Grove逻辑引脚有依据，但逐针物理方位尚未确认，连接线继续禁用。独立重建的7个资产及模型配置逐项一致，见 [重建收据](evidence/atom-matrix-v11/model-rebuild.json)。

## 复现

```sh
node tools/probe-atom-matrix-v11-uiflow.mjs \
  https://burner.m5stack.com/api/v1/firmwares/2099000000000000002/versions/2098279897621549057/download \
  1591382c9b46f10549faa0053b4036aa55f5c8131867b2f039d2650982e7886d
python tools/rebuild-product-model.py build m5stack-c008-b-v11 --scratch <independent-output>
```

[生产WASM收据](evidence/atom-matrix-v11/official-wasm.json)与[负面记录](evidence/atom-matrix-v11/negative.json)分别保留。鼠标/键盘释放测试尊重平台至少150ms宿主保持时间，最终读取前等待200ms宿主时间；不是周期精确的输入或物理去抖认证。原始串口包含设备标识，仅保留在忽略目录，公开收据仅取输入/传感器结果；不提交原始 API 开发者资料。

## 最终检查

28组检查全部通过：最新 stable、双 Clippy（warnings denied）、1439项 Rust（132组，含 ignored 并跳过 external_）、11个生产WASM场景、100项平台测试、真实录制回放、schema、111126个WASM JIT差分用例及24个GPIO波形用例。没有重新生成或修改 golden。完整命令与结果见[checks.json](evidence/atom-matrix-v11/checks.json)。

最终[浏览器收据](evidence/atom-matrix-v11/browser.json)来自原始官方镜像：鼠标和键盘分别读出0→1→0，旋转后的加速度与注入方向在一个8g量程计数（1/4096g）内一致。分享 mini 还原通过；2D模式明确说明该型号没有屏幕。最终资产hash和外观偏差见[model-validation.json](evidence/atom-matrix-v11/model-validation.json)。验收浏览器和独立服务均已关闭。

以固定 main `84e67571` 二进制为参考，三组 hello 各5对、每次30秒客体，median user time差异 S3 −4.0%、C3 0.0%、C6 0.0%；UART/指令/周期逐对相同。仅作跨版本生产回归护栏，百分秒计时量化和宿主噪声不足以归因性能改善；见[CPU收据](evidence/atom-matrix-v11/cpu.json)。未改S3核心，无Pocket要求。

阻断模型请求后会回到无屏幕2D视图，配置仍可用；通用“官方CAD”来源徽标在该回退状态仍显示，属已记录的展示局限，不作为模型加载成功的证据。
