# 可选的 3D IMU 联动

工作台和 mini/iframe 的 3D 工具栏提供“IMU 联动”按钮，默认关闭。
关闭时拖动只改变观察视角；开启后，模型旋转通过原有运行时输入通道
改变虚拟传感器，加速度和角速度仍由客体固件读取。

目前覆盖 StopWatch、CoreMatrix、AtomS3、Atom Matrix、Atom Matrix v1.1、
AtomS3R、AtomS3R Chatbot、PaperS3、StampFly 和 StampFly v1.1。
这些板型与 `capabilities().imu.available` 使用同一份运行时注册表。
没有已实现的 host-fed IMU 时隐藏按钮，不按产品名称猜测硬件能力。

## 使用和生命周期

- 固件运行中且位于 3D 视图时可开启。首次开启立即按当前模型姿态发送
  静止样本；后续拖动至多每秒发送 30 次。慢请求只保留一个最新待发送样本。
- 松手、指针取消、失焦或切到后台时清零手势角速度。关闭按钮后保留当前
  加速度，随后拖动不再改变传感器；清零请求不覆写姿态。
- 暂停、切到 2D、切设备、重建运行时、切换外观预览、开始录制或回放时
  关闭联动。回到可操作状态后需要用户重新开启，不自动恢复手势控制。
- 飞行环境拥有 StampFly 的传感器输入，按钮禁用并说明原因。关闭飞行
  环境后也保持未开启；独立输入仍受运行时 `FLIGHT_OWNS_IMU` 检查。
- agent 的 `setImu` 保持原有能力，显式输入接管传感器并关闭工具栏联动。
  其指定的角速度不会被 UI 清零。设备更换后，旧手势和清理请求不会写入
  新设备。UI 开关不调用 `setImu({enabled:false})` 去禁用 agent 传感器。
- 开关仅属于当前页面，没有加入场景、分享链接、录制或持久化设置。
  新页面、分享恢复及 iframe 默认关闭。二维画面没有模型拖动输入。

联动关闭时，场景旋转不创建姿态节流/结束定时器，也不提交 IMU 消息。
这次改动没有加入 Rust、WASM 或模拟器每 tick 工作。

## 明确的虚拟轴

四元数 `[x,y,z,w]` 表示模型局部坐标到世界坐标的旋转。局部 `+X` 向右、
`+Y` 向上、`+Z` 朝模型正面外侧；有屏幕的模型通常对应屏幕外侧。
世界重力指向 `-Y`，静止加速度为 `inverse(q) × [0,1,0]`，单位 g。
单位四元数得到 `[0,1,0]g`，绕 `+Z` 旋转 90° 得到 `[1,0,0]g`。
固件自己的轴交换仍由固件执行；例如 CoreMatrix M5Unified 读数会转换
为 `[rawY,-rawX,rawZ]`。这里不额外补偿该变换。

这是一致的软件轴约定，并非实物 PCB 安装标定。能力中明确保留
`physicalMountingVerified:false`、`hardwareValidated:false`，同时声明
`sceneOrientation:true`、`sceneMapping:'virtual-model-xyz'` 和
`sceneLinkDefault:false`。未校准安装方向不妨碍用户选择虚拟联动。
没有模拟平移、自由落体、碰撞、传感器噪声或手势的精确 guest 时间轨迹。

## 验证

`tools/platform-scene-imu-test.mjs` 检查默认关闭、慢请求合并、关闭时尾部
清零、会话更换、agent 接管、暂停/预览/录制/2D/飞行互斥，并遍历上述
十种板型的运行时能力和虚拟轴输入。原有 IMU、飞行、平台、录制边界
仍使用原来的运行时接口。

`tools/fixtures/scene-imu-browser-check.js` 通过任务自有的 Playwright
无头会话运行官方 CoreMatrix UIFlow 2.5.4 原始镜像，经 ROM 启动，
用 USB raw REPL 直接读取 BMI270 的 12 个加速度/角速度数据字节。
运行时使用已有生产 WASM，没有另编模拟器或替换固件。
镜像 SHA-256 为
`8d5b8d4e22ebc293937086b2f9f218aa5a5ebda27b5ccb32b3f8f580ee02939b`。

结果保存在[浏览器收据](evidence/scene-imu/browser.json)：关闭时拖动前后
字节完全相同；开启后加速度读回与姿态投影相差不超过一个传感器量化
单位；松手后六个角速度字节全零；关闭再拖动字节保持不变。浏览器也
检查暂停、2D、重建、agent 接管、飞行占用、mini/分享 iframe 默认关闭，
并检查 390px 窄屏工具栏没有横向溢出。

```sh
node --test tools/platform-scene-imu-test.mjs tools/platform-imu-test.mjs \
  tools/platform-motor-test.mjs tools/platform-flight-runtime-test.mjs \
  tools/platform-config-test.mjs tools/platform-runtime-test.mjs \
  tools/platform-input-test.mjs tools/platform-view-test.mjs tools/platform-i18n-test.mjs
```

浏览器脚本要求页面查询参数 `firmware` 指向调用方提供的原始镜像 URL，
并固定校验上述 hash。通过 Playwright skill 的 wrapper `run-code` 执行，
结束后关闭该任务会话。截图留在 ignored `output/playwright/`；收据只保存
公开输入 hash、数值和检查结果，不保存本机地址、用户路径或原始日志。
本轮没有实机验证，也不把一个固件的读回扩展为所有固件完全兼容。
