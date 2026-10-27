# AtomS3 C123 官方出厂固件验证

2026-10-27，官方 v0.1 出厂镜像在既有 `atoms3` GC9107 板级模型上通过 ROM 启动。
原生解释器和浏览器 WASM JIT 均显示真实 WiFi Scan 菜单，未修改固件、替代应用或
添加外设 ACK 桩。本轮核验的是 C123，不包含 AtomS3R 或其他屏幕批次。

浏览器中已核验：

- 点击 3D 屏幕，GPIO41 按钮输入让固件切到 I2C Scan。
- 键盘 A 让固件继续切到 UART MON；四次 agent 短按后到达 IMU TEST。
- 长按屏幕按钮进入 IMU 页面。`setImu` 从竖直改为绕虚拟 X 轴倾斜45°后，
  客体通过 MPU6886 I²C 读取数据，自己绘制的小球发生位移。
- 分享到 mini 后，仍先显示 trial 确认；点击继续运行，重新从 ROM 启动到同一菜单。

画面、实际输入回执、固件/ROM/WASM hash 及来源见
[verification.json](evidence/atoms3-official-firmware/verification.json)。
所有检查都是客体输出，不以输入已排队作为通过依据。

出厂固件仍是默认第一项，继续保留 trial 和 `simulationVerified:false`。
Wi-Fi 扫描、I²C 外接扫描、UART、PWM、ADC 和 IR 应用尚未做功能验收；进入菜单
不等于这些功能全部可用。IMU 输入是未与实物校准的虚拟轴，不自动跟随 3D 旋转。
背光 PWM、传感器 FIFO/中断等原有边界见 [板级说明](atoms3-runtime.md)。未实机验证。

## 复现

构建 WASM 并获取 ROM 后，在独立浏览器上下文打开平台：

```js
const {loadCatalog,createDefaultScene}=await import('./catalog.js');
const catalog=await loadCatalog();
const scene=createDefaultScene(catalog,'m5stack-c123');
scene.firmware=catalog.firmwares.find(f=>f.id==='m5stack-c123-official-reference').source;
await espTester.configure(scene);
await espTester.loadFirmware();
await espTester.start();
// 等真实 WiFi Scan 菜单出现；不要把下载/启动回执当作画面通过。
for(let i=0;i<6;i++) {
  await espTester.press('btnA',{durationMs:200});
  await new Promise(resolve=>setTimeout(resolve,400));
}
await espTester.press('btnA',{durationMs:1200});
await espTester.setImu({quaternion:[0,0,0,1],angularVelocityDps:[0,0,0]});
// 观察竖直画面，再设置倾角并等待固件滤波器收敛。
await espTester.setImu({quaternion:[0.3826834323650898,0,0,0.9238795325112867],angularVelocityDps:[0,0,0]});
await new Promise(resolve=>setTimeout(resolve,3000));
await espTester.pause();
await espTester.capture({format:'png'});
```

短按/长按使用宿主机时间，机器繁忙时需检查实际客体输入回执和页面，不能假设
时序完全一致。复现前不得自动清除用户 Flash 数据。本轮测试未导入外部文件或
改写用户程序，远程固件未加入 Git 或网站资产。

本轮只更新目录与证据。源码基线 `c195e1ae` 的 Rust/worker 与所用 `a9375446`
构建一致，复用相同 SHA 的原生程序与 WASM；没有 Rust、硬件时序或性能改动。
