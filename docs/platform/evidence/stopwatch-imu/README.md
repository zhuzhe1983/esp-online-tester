# StopWatch BMI270 与官方 V0.5 验收

固定固件 SHA-256：`6395799089341e0ae96072c2a3418d469d6797b2ab8ee0cab1c9ed24a42acbbe`，3,854,192 字节。官方源为 M5Burner CDN；探针下载到 RAM，不保存固件文件。来源、下载时间、字节数和 hash 见 [sources.json](sources.json)。

实际 WASM worker 与未经修改的官方固件已完成：I²C 初始化、官方触摸箭头打开 IMU、五次静止姿态、90 °/s 陀螺仪输入、禁用场景耦合。初始四元数单位值的 BMI270 读数为 `[0,1,0] g`，官方 HAL 交换 X/Y，所以画面显示 X=1.0。[运行回执](interpreter-pass.json)记录实际 guest 时间、输入回执、帧 hash、输出和异常状态。原始图由真实 framebuffer 导出：

| 姿态 | 官方固件标签 X/Y/Z | 画面 |
| --- | --- | --- |
| 正放 | 1.0 / 0.0 / 0.0 | [upright](imu-upright.png) |
| 绕模型 X 转 90° | 0.0 / 0.0 / -1.0 | [tilt-x](imu-tilt-x.png) |
| 绕模型 Z 转 90° | 0.0 / 1.0 / 0.0 | [tilt-z](imu-tilt-z.png) |
| 绕模型 X 转 180° | -1.0 / 0.0 / 0.0 | [inverted](imu-inverted.png) |
| Z 角速度 90 °/s | 保持静态重力值，圆球变大、外围点旋转 | [gyro](imu-gyro.png) |

图片已实际打开核对，未使用前端覆盖层或固件补丁。解释器探针总耗时 65.153 秒，最后检查约 40.78 guest 秒；总耗时含网络、初始化、交互和保存，**不是 CPU 基准或稳定实时倍率**。没有实物测试；坐标安装约定、主机输入采样、未实现 FIFO/ODR/特征引擎等限制见[功能合同](../../stopwatch-imu.md)。

保留的失败：

- [JIT / 10 秒 watchdog](jit-timeout-10s.json)：进入官方 IMU 页面并显示第一帧后，worker 不再确认时钟请求。
- [JIT / 60 秒 watchdog](jit-timeout-60s.json)：更长 watchdog 仍失败；增加等待没有解决。未宣称已查明 JIT 根因。
- [短按超时](short-press-overrun.json)：一次 80 ms 目标按键持续 0.564 guest 秒，超过固件 500 ms 长按分界。IMU 正向验收随后采用官方页面本身的触摸翻页箭头，不用短按时序来假定导航成功；真实串口必须包含 `[IMU] on open`。
- 第一次完整 Rust 测试使用相对 ROM 目录，workspace 子包无法找到 classic ROM；使用绝对目录后全部 1328 项通过。代码未因此改变。

StopWatch 的平台创建配置现在固定 `jit: false`，保留真实 WASM 解释执行；其他板未改。相关历史是 [EX194](../../../experiments.md#ex194)，本次为不同传感器的外部姿态功能扩展，不是新一轮性能优化。

复现（调用者明确提供远程目标）：

```sh
tools/wasm-build.sh
node tools/probe-official-stopwatch.mjs "$FIRMWARE_URL" "$FIRMWARE_SHA256" output/imu-check 8 imu
# 仅用于复现已知失败，显式覆盖 StopWatch 的默认解释器：
ESP32SIM_FORCE_WASM_JIT=1 ESP32SIM_PROBE_CLOCK_TIMEOUT_MS=60000 \
  node tools/probe-official-stopwatch.mjs "$FIRMWARE_URL" "$FIRMWARE_SHA256" output/imu-jit-check 8 imu
```

核验记录与文件 hash 见 [verification.json](verification.json)。两项 Clippy、workspace release tests（含 ignored、跳过 external）、真实 runtime/recording 测试及定向姿态测试通过。没有 bus/per-tick 路径改动，不新增 CPU 基准结论。来源原文只存在临时下载目录；提交保留 URL、固定版本和 hash。回执没有本机路径、进程清单、IP 地址或设备身份。
