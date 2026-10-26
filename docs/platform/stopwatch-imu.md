# StopWatch 的 3D 姿态与 BMI270

StopWatch 的姿态输入经过真实 BMI270 I²C 寄存器模型，官方 V0.5 固件通过其原有驱动读取加速度和角速度。前端没有重画 IMU 数字、圆球或轨迹，也没有修改固件字节。

StopWatch 目前默认使用 WASM 内的真实 CPU 解释执行：同一固件的 JIT 路径进入 IMU 页面后出现两次不响应，分别触发 10 秒和 60 秒 watchdog。解释执行完成了所有姿态和角速度检查；其他板仍保留原来的 JIT 默认设置。此取舍会降低 StopWatch 速度，JIT 原因尚未定位。[正负结果与实测时间](evidence/stopwatch-imu/README.md)均保留。

## 坐标与范围

- 四元数顺序是 `[x, y, z, w]`，描述设备局部坐标到场景世界坐标的旋转。虚拟传感器局部轴约定：X 指向屏幕右侧，Y 指向顶部，Z 从屏幕向外。公开资料尚不足以证明这与实物 PCB 安装方向一致，因此这是明确的虚拟坐标约定，未实机验证。
- 世界重力指向 `-Y`。静止加速度计读到反向支撑力，计算为 `inverse(q) × [0, 1, 0]`，单位为 g，模长为 1。没有模拟平移、自由落体、震动或传感器噪声。
- M5Stack 官方 V0.5 `hal_imu.cpp` 把传感器 X 读入显示 Y，把传感器 Y 读入显示 X，Z 不变。这一交换由原固件完成，模拟器不另外交换读数。设备正对观察者、四元数为单位值时，官方标签应为 `X: 1.0, Y: 0.0, Z: 0.0`。
- 连续拖动根据相邻四元数的最短旋转和主机手势时间计算角速度，单位为 °/s，上限 ±2000。大于 250 ms 的采样间隔视为新手势，避免把暂停累积成速度；停止拖动、失焦、暂停时回到零角速度。此输入不是按 guest 时间回放的物理运动轨迹。
- 连续场景拖动至多每秒发送 30 次姿态，结束时立即发送最终位置与零角速度。注入本身不执行指令、不增加模拟时钟。

## 公共 API

```js
await espTester.setImu({quaternion: [0, 0, 0, 1]});
await espTester.setImu({
  quaternion: [Math.SQRT1_2, 0, 0, Math.SQRT1_2],
  angularVelocityDps: [0, 0, 0],
});
await espTester.setImu({enabled: false}); // 停止 3D 场景耦合，保留姿态，角速度清零
await espTester.setImu({enabled: true});
const state = (await espTester.observe()).imu;
```

`source` 可为 `agent`（默认）或 `scene`。禁用时忽略 `scene` 输入；agent 仍可手工设定姿态。agent 默认设置静止姿态，角速度为零；需要转速时显式提供 `angularVelocityDps`。配置后即可设定，加载时应用到新传感器；未运行时角速度为零。换设备重置 IMU 状态，销毁时终止 worker 和姿态定时器。`setImu` 不写场景分享配置；当前操作录制不记录 IMU，automation 中的传感器操作会停止录制或接管回放。

返回值和 `type: "imu"` 事件的 `imu` 字段含 `enabled`、`source`、`quaternion`、`accelG`、`gyroDps`、`delivery`、`guestTime`。`register-state` 仅证明 worker 已把值写入传感器共享状态；固件读取和画面更新需要另行观察。`capabilities().imu` 描述支持和限制。iframe 使用同名 API，不涉及硬件刷写。

## 已实现的芯片子集

I²C0 地址 `0x68`、CHIP_ID `0x24`、软复位、8192 字节配置传输及不完整传输错误、加速度/角速度使能、量程、16 位有符号小端数据、读事务快照和数据就绪状态。加速度支持 ±2/4/8/16 g；陀螺仪支持 ±2000/1000/500/250/125 °/s，超范围饱和。官方驱动设置 ±4 g、±2000 °/s、100 Hz 配置。

模型没有执行 Bosch 配置里的运动特征程序，不实现 FIFO、硬件中断引脚、ODR/滤波时序、传感器时间戳、自检、步数、辅助磁力计或功耗/唤醒。数据就绪由新 host sample 或使能触发，不能把 30 Hz 主机输入称作精确 100 Hz 传感器采样。BMI270 子集只在主机输入和 I²C 读写时工作，没有增加 bus/per-tick 工作。

## 来源与复核

- [Bosch BMI270 数据手册 BST-BMI270-DS000-08](https://www.bosch-sensortec.com/media/boschsensortec/downloads/datasheets/bst-bmi270-ds000.pdf)：4.4 初始化、4.6 数据、5.2 寄存器、8.2 加速度和角速度符号。
- [M5Stack 官方 V0.5 HAL](https://github.com/m5stack/M5StopWatch-UserDemo/blob/6b4aa125288b6fe9dca661f10159f6e1e5ee785c/main/hal/hal_imu.cpp)及 [IMU 页面](https://github.com/m5stack/M5StopWatch-UserDemo/blob/6b4aa125288b6fe9dca661f10159f6e1e5ee785c/main/apps/app_imu/app_imu.cpp)。
- [V0.5 依赖声明](https://github.com/m5stack/M5StopWatch-UserDemo/blob/6b4aa125288b6fe9dca661f10159f6e1e5ee785c/repos.json)固定 BMI270_BMM150_Sensor 0.1.2；其[驱动](https://github.com/lbuque/BMI270_BMM150_Sensor/blob/ae8bc480bca638d3458e65d5e9a4fff4270c1430/src/bmi270_bmm150.c)和 [Bosch 寄存器定义](https://github.com/boschsensortec/BMI270_SensorAPI/blob/41129fcfe39c583ee5462d79195741945d51c1fe/bmi2_defs.h)用于核对初始化、量程换算和轮询数据就绪。
- 相关历史实验是 [EX194](../experiments.md#ex194) 的 QMI8658 固定脚本；本次实现 BMI270 与外部姿态输入，保留该脚本路径，不修改 guest 时钟或运行节奏。

测试包括芯片配置/量程/饱和/快照、四元数重力投影/短弧旋转、WASM 输入不改变 cycles/insns、真实 worker 生命周期及固定官方固件画面。远程官方固件仍不镜像到服务器或仓库；用户浏览器允许按校验后的 URL 与 hash 缓存。
