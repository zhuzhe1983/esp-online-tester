# Unit ENV-IV U001-D

本项是 **U001-D ENV-IV：SHT40-AD1B + BMP280**，不对应 ENV-III（SHT30/QMP6988）或 ENV Pro。官方电路以 HY2.0-4P 的 5V 经 HT7533 供电；黄色 SDA、白色 SCL 都有 4.7kΩ 上拉。首轮只连接 StopWatch Port A：G10 SDA、G11 SCL，电源由客体 M5PM1 控制。也可通过一层 U006 逐脚并联；同时占用 G10 或 G11 的其他配件会拒绝，不能把分线器当作额外 GPIO。

环境输入经过真实硬件 I²C 控制器和 S3 GPIO matrix 路由，传感器按地址、命令、忙状态和 CRC 回应；前端不把数字写入固件变量，也不通过串口伪造结果。软件 GPIO I²C 尚未实现。

```js
const scene = (await espTester.observe()).scene;
scene.accessories = [{
  id: 'environment', catalogId: 'm5stack-unit-env4-u001d',
  parameters: {temperatureC: 25, humidityPercent: 50, pressureHpa: 1013.25},
  wiring: {mode: 'physical', interfaceId: 'grove'}
}];
await espTester.configure(scene);
// 加载并启动当前用户固件后：
await espTester.input({type: 'accessory-environment', accessoryId: 'environment',
  temperatureC: 22.5, humidityPercent: 61.2, pressureHpa: 990.1});
```

三项必须同时传入有限数值。统一范围为 −40…85°C、0…100%RH、300…1100hPa；温度取两颗芯片的交集，不改写 SHT40 硬件到125°C的规格。API、工作台面板、mini 与录制使用同一路径。环境在断线时仍可改变；只有重新供电后的有效测量才返回新样本。暂停时拒绝环境修改，允许拔下。客体重启保留连接和环境状态；平台 reset/重新加载恢复 scene 中的初始参数与接线。

| 器件 | 本轮协议范围 | 明确边界 |
|---|---|---|
| SHT40，0x44 | FD/F6/E0 三档测量；8.2/4.5/1.7ms 最大转换时间；CRC-8；首次 ACK read header 消耗测量；89虚拟串号；94与通用调用06复位 | 加热命令未实现并 NACK；虚拟串号不是用户实物身份；未模拟时钟拉伸、噪声或热响应 |
| BMP280，0x76 | ID58；0x88…0x9F trim；0xF4过采样与强制/普通/睡眠模式；0xF5 IIR、standby；0xF7…FC原始数据；0xE0 B6复位；读突发冻结 | trim采用公开 datasheet 测试向量，不是实物标定；测量时间取datasheet最大值；NVM忙位只建模复位窗口；SPI3W不接线；不模拟电气误差或实体噪声 |

BMP280 从理想环境反求20位原始量，再由客体执行补偿；因此固件读数有量化误差。内部 fixture 直接包含固定版本 M5Unit-ENV 的未修改 MIT 补偿头文件，以独立厂商计算核对模型，不把前端设定值当作客体证明。源码在 `tools/fixtures/unit-env4-firmware`；构建产物只保留 ignored output，不进入线上固件库。

## 外观与来源

没有找到 ENV-IV 独立厂家 CAD。当前外壳使用固定提交的 **ENV-III** 原始 STL 两个装配件，省略两个打印排版副本，只平移、不缩放。用 ENV-IV 官方24×32×8mm图纸、通风孔、安装孔、接口及照片逐项核对；这不是 ENV-IV 完整官方装配文件。

背贴取 ENV-IV 官方背面照片的纸面四角，透视校正后装到独立薄面，排除灰色壳体、孔洞和螺丝。原图、四角坐标、输出像素与 SHA-256 均保留。PCB 虽有官方照片，但缺少布局/板级 CAD；`pcb-main` 保留中性 `UNKNOWN` 简化占位，不补猜传感器和走线。后盖螺丝、内部板形及元件布局尚未还原；保留原 CAD 的 M5 字样。

Grove 背贴左→右为白/黄/红/黑，镜像到前视 X 得黑/红/黄/白；照片同时核对红色连接器及开口。接口锚点 y=−16mm，pin row z≈2.85mm 依据官方端视图比例估计。图上5.2mm是壳体接缝，不能当作针排高度；隐藏插合深度仍是估算。

公开重建：

```sh
python tools/rebuild-unit-library.py m5stack-unit-env4-u001d --scratch output/env4-rebuild
```

完整来源与实际检查收据位于 `docs/platform/evidence/m5stack-unit-env4-u001d/`。环境参数不是实测；本轮没有连接真实硬件，不代表任意固件或全部传感器功能兼容。

2026-10-28 在 `5a9b68e0` 基线重新移植与验证：保留当前 StopWatch 内部 G47/G48 的软件及硬件 I²C 共用器件，独立追加外部 G10/G11 的两个地址，不能把外部配件接进内部总线。冷输入方法追加在现有 Board 方法末尾；没有新增通用 Machine 状态字段或每 tick 遍历。

SHT40 复用已有 `Sht4x`，通过显式 D1 v2 配置采用厂家附带手册的 8.2/4.5/1.7ms 时序，支持复位中止旧测量及重新上电后的 1ms 窗口。Air Quality 使用的默认 v7.3 配置保持 8.3/4.5/1.6ms、串号等待和原有忙状态行为；边界回归分别覆盖两种配置。新抓取的厂家手册、当前 Sensirion v7.3 以及原理图有各自 URL、时间与 hash，不覆盖历史来源。

本次10项源几何/尺寸检查及正面轮廓 IoU0.9766（阈值0.95）通过，公开重建资产逐字节一致。背面和接口斜视仅完成定性对照，没有相机内参，不能标作完整1:1通过。11组真实 WASM 检查覆盖错误引脚与地址、CRC/忙状态、供电、拔插、重启、回放和 Hub；工作台与 mini 的实际客体补偿读数亦已核对。双 Clippy、1,568 项 Rust、104 项平台检查及其余 CI 组全部通过，没有修改 golden。当前收据位于 [current-20261028](evidence/m5stack-unit-env4-u001d/runtime/current-20261028/README.md)，旧收据保留原时间和版本。

本次性能对照采用独立编译的固定 `5a9b68e0`，每组一次预热、五对交替样本。S3 Hello 中位 CPU 0.26→0.26s，C3 2.49→2.59s（+4.016%，5/5 对更慢），C6 1.70→1.65s（−2.941%）。每对客体指令数和输出相同；C3 差异原因未确定，不能称为无回退。当前没有修改 S3 core/bus/tick 遍历，因此未重复 Pocket Tank；不是将旧测量当作本次结果。

2026-10-27 历史对照仍保留：S3/C3 Hello 中位 CPU 不变，C6 1.67→1.72s（+2.994%，4/5 对更慢），Pocket Tank 41.45→41.51s（+0.145%，2/5）。两轮基线不同，不能合并或互相抵消，亦没有无变化重跑挑选结果。
