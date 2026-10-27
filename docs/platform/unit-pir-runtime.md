# Unit PIR U004 的移动感应输入

U004 使用 AS312，经 HT7533 由 Grove 5V 供电；传感器 OUT 串联 1kΩ 后接白线，黄线未连接。首轮支持 StopWatch Port A 白线 G11，可经一层 U006。背贴的白色 IN / 黄色 OUT 是主机 Port B 的方向标注；它不代表 U004 有两个输出。依据、原始地址、抓取时间和文件校验见[知识记录](../../web/platform/knowledge/m5stack-unit-pir-u004.json)。

## 已实现

```js
await espPlatform.input({type:'accessory-motion',accessoryId:'pir',motion:true});
await espPlatform.input({type:'accessory-motion',accessoryId:'pir',motion:false});
```

`motion:true` 表示视场内持续的红外变化，不是“有人静止站着”。`false` 停止变化，随后保持高电平 2 个客体秒；再次变化取消下降期限。初始化来自 `parameters.motion`，默认 `false`。面板、agent、mini 和录制使用同一 API。观察中的 `accessoryInputs[].motion` 是环境输入，不能代替固件 `gpio_get_level(11)` 的结果。

模型名 `pir-u004-m5-2s` 明确采用 M5 官方 2 秒可重复触发描述。两份 Senba AS312 原始手册的第三方镜像正文也写 2 秒，但典型参数表同时列出 2.3 秒 on-time / lock-time；没有资料证明其版次对应 U004。本实现不模拟该锁定窗口，不宣称实物时序校准。没有模拟预热、热源距离/速度、光学视角、菲涅尔光路、温度漂移或噪声。

输出通过现有 BoardEdge/deadline 和真实 GPIO 输入寄存器，不发送串口假读数。M5PM1 Grove 5V 关闭或拔线时释放外部驱动，由固件上下拉决定读数；掉电高阻是平台理想接口策略，未经 AS312 漏电/反向供电实测。恢复供电清除旧保持期限，按当前环境重新驱动。黄线 G10 保留原用途，不跟随移动输入。

U006 只做逐脚并联：允许与 G10 的 U003 共存，拒绝占用 G11 的 Button/Angle/ENV/DLight、第二个 PIR、重复插口和多层 Hub。Hub 上游断开同时撤掉叶子的物理连接，重连仍保留单个子口的拔出状态。GPIO 争用不通过“后写覆盖”偷偷解决。

暂停时拒绝改变环境，允许拔出配件；客体重启保留外部连接与环境，平台 reset 恢复 scene 的连接和初始环境。环境可在运行但拔线时改变；再次插入采用当前环境。录制允许结束时保持持续移动，因为它不是按住未释放的按键；回放按客体时间重新输入，仍不保证逐指令一致。

## 模型与来源

直接保留官方 U004 STL 的 0/3 装配壳体三角形，略去 1/2 打印排版副本。原始毫米坐标仅刚体平移；外壳 24×32×8 mm、整机含镜片 12.5 mm。镜片按官方尺寸图 Ø10.2 mm 和照片建立平滑球冠近似，没有伪造菲涅尔细面或隐藏 PCB。内部空板标 `UNKNOWN`；可见黑色 Grove 插座开顶，照片白贴独立于外壳材质。背面螺丝尚未还原。

复现已提交模型：

```sh
python tools/rebuild-unit-library.py m5stack-unit-pir-u004 --scratch output/pir-rebuild
python tools/verify-unit-pir-model.py --renders output/playwright
```

第二条需先用实际通用渲染器的 `review-lane-01.html?id=m5stack-unit-pir-u004` 拍摄固定 800×800 正/背/插口图和黑色 mask。镜片、孔位、材料、线序分别目视检查；轮廓 IoU 不作为内部结构或固件兼容证据。来源照片版权属于 M5Stack；没有另行核实开源照片许可。结构 STL 为 M5_Hardware MIT。

## 阶段状态

功能阶段已验证；性能对照尚未开始，不能声明无性能回退。工作台与 mini 自动启动均已保留完整 uart0 GPIO 高低变化证据。此阶段保留待审，不作为最终合入许可。

## 定向验收

[内部源码 fixture](../../tools/fixtures/unit-pir-firmware/README.md) 由 ESP-IDF 5.5.1 构建，运行于实际页面 worker/WASM，读取真实 GPIO 与 M5PM1 寄存器。它不进入线上固件库，不替代官方程序。U004 没有用户可刷写 MCU，官方 M5GO PIR 示例运行在外部主机；不虚构配件出厂固件。真实客体、浏览器、模型复建、CI 与 CPU 收据见 [runtime evidence](evidence/m5stack-unit-pir-u004/runtime/)。未实机验证，不代表任意主机/固件兼容。
