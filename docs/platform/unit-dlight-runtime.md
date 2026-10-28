# Unit DLight U136

本项对应 **U136、BH1750FVI-TR**，不对应 U134 HAT。官方电路将 ADDR 接地，地址固定为0x23；HY2.0-4P 的5V经HT7533降至3.3V，黄色 SDA、白色 SCL 有4.7kΩ上拉。首轮核验 StopWatch Port A 的G10 SDA/G11 SCL，供电由客体 M5PM1 控制。Grove 外部软件 GPIO I²C 尚未实现（主机内部 G47/G48 总线另有实现）。

```js
const scene = (await espTester.observe()).scene;
scene.accessories = [{
  id: 'light', catalogId: 'm5stack-unit-dlight-u136',
  parameters: {lux: 1000},
  wiring: {mode: 'physical', interfaceId: 'grove'}
}];
await espTester.configure(scene);
// 加载并启动用户固件后：
await espTester.input({type: 'accessory-illuminance', accessoryId: 'light', lux: 4321.5});
```

`lux` 必须是0…100000的有限数值；0表示黑暗，高照度范围需要客体调节MTreg。默认MT69、H1模式的16位结果在约54612lx饱和，不能把API范围当作所有模式的测量范围。输入参数只是虚拟环境，必须检查客体I²C读出的两个原始字节或实际固件日志。工作台数值控件、agent、mini与录制使用相同路径。

| 本轮协议 | 行为与边界 |
|---|---|
| H1/H2/L；连续及单次 | 0x10/11/13、0x20/21/23；用客体周期积分曝光，转换结束前保留旧值；H模式180ms、L模式24ms最大时间按MT/69缩放 |
| MTreg | 高/低位命令、31…254有效测量范围；灵敏度按MT/69，H2再乘2；转换结果饱和至65535 |
| PowerOn/Down、Reset | 上电默认MT69、无测量；Reset只在PowerOn时清数据；单次自动PowerDown仍可读最后结果；真正断电/断线NACK |
| 物理路由 | 硬件I²C0/1及GPIO matrix；必须正确G10/G11、0x23和Grove电源；错误地址0x5c、错误引脚不响应 |

L模式将原始结果量化为4个计数一档。datasheet只给典型4lx分辨率，未规定内部低位算法，因此这是明确的确定性近似，不是实物逐位证明。未实现光谱、角度响应、外壳窗口透过率、噪声、器件偏差、模拟供电过渡和时钟拉伸。未知命令防御性NACK，不据此宣称芯片真实ACK行为。

断线期间允许改变照度；暂停时拒绝照度修改、允许断开。客体重启保留当前环境和连接；平台reset/重新加载恢复scene初始值。经一层U006时，DLight0x23可与ENV-IV0x44/0x76共用同一对SDA/SCL；重复地址、重复插口、数字/模拟/RGB信号争用或多层Hub仍拒绝。断开DLight不会影响ENV；断开Hub同时中断子设备。Hub输出口的视觉针1仍未确认，输出电缆继续隐藏，不伪造已核实线序。

## 源码与真实客体验证

内部fixture直接包含固定提交 `m5stack/M5-DLight@4dd8d2bd862abce5e995a2e7efea2a691baa8c75` 的未改动MIT驱动；最小Arduino/Wire适配层调用真实ESP-IDF I²C控制器。驱动的`getLUX()`只计算raw/1.2，没有补偿H2或MTreg。fixture保留该厂商值，同时打印独立按datasheet换算的读数；不会修改驱动来让结果符合模型。

13组实际WASM测试覆盖厂商驱动、忙窗口、全部测量模式、MTreg、断电、断线、错误路由、记录回放、重启、默认饱和与100klx范围、Hub多地址共存。浏览器工作台设置4321.5lx，客体返回raw5186，即4321.6667lx；mini从120lx切到777lx，返回raw932，即776.6667lx。差值来自整数转换，不是前端伪造输出。BIN只放ignored output，不加入线上固件库。

## 模型与来源

使用官方U136 STL的前后两片已装配壳体，省略两个打印排版副本；只作刚体坐标变换、不缩放。24×32×8mm、Ø5安装孔与16mm孔距按官方尺寸核对。背贴仅截取白纸四角，排除灰色外壳；正面感光口保留真实CAD开口，并只补公开照片能看到的BH1750封装顶面。缺少PCB布局/CAD，内部保留中性`UNKNOWN`空板，形状和位置为估算，未补猜隐藏元件和走线。

照片确认红色Grove开顶结构，使用显式`housingVariant:'open-roof'`，旧产品的默认闭顶几何和材质不变。前面左到右为黑/红/黄/白，背贴镜像为白/黄/红/黑。接口y=−16mm，pin row z≈0.85mm依据端视图估计；图中5.2mm是接缝位置，不是针排高度。隐藏插入深度仍为估算。前后外轮廓IoU均≥0.95；斜视仅定性检查，不冒充有相机内参的1:1验证。原始CAD、照片、抓取时间、最终地址、MIME、哈希及许可证均在资产/证据目录。

```sh
python tools/rebuild-unit-library.py m5stack-unit-dlight-u136 --scratch output/dlight-rebuild
```

公开重建与已提交资产逐字节比较。验证入口`tools/verify-unit-dlight-model.py`，证据位于`docs/platform/evidence/m5stack-unit-dlight-u136/`。没有连接真实硬件，不代表全部固件或BH1750功能兼容。

本轮5对同工作量CPU对照中，S3/C6 Hello中位值不变，C3为−0.826%，Pocket Tank为−2.113%（2/5对较慢）；指令数与控制台输出哈希一致。样本区间重叠并有主机负载漂移，不据此宣称加速。此前U027、U003与ENV-IV的成本记录保留，不能用本次增量对照抵消。全部样本见runtime/cpu.json。

## 当前主线复核（2026-10-28）

在主线 `5a9b68e0` 上接入 ENV-IV 基础 `95fc5da2` 与通用 SHT4x 复用修复 `da61bbc1`，保留 StopWatch 内部 G47/G48 的共享硬件/GPIO I²C 路径；Grove G10/G11 的 U136 仍是单独的物理端点，不影响内部 PMIC、触摸、IMU 与 RTC。官方页面、原理图、datasheet、固定 CAD/驱动和照片共15个来源重新抓取，hash 均与历史资料相同，原始抓取时间保留。

新编译 ESP-IDF5.5.1 fixture 的15组真实 worker/WASM 检查已通过；补测了连续 H2、连续 L 和连接状态下客体重启后的环境保留。工作台与 mini 均重新检查真实原始字节；重建11个模型资产逐字节一致。当前收据与截图位于 [refresh-20261028](evidence/m5stack-unit-dlight-u136/refresh-20261028/)，历史 runtime 目录继续保留2026-10-27结果。

最终共享代码通过双 Clippy、1,572 项 Rust 测试（含 ignored/golden）、107 项平台检查、11 个生产 WASM 场景及其余 CI 检查；golden 未改动。最终 WASM 的工作台与 mini 再次读取到 raw5186 和 raw932，原始协议检查仍为15组全部通过。

当前固定主线的五对 CPU 对照：S3 Hello 中位值不变，C3 +0.397%（4/5 对较慢），C6 −1.212%（0/5 对较慢）；全部指令数与 UART 输出逐对相同。这是 ENV-IV 与 DLight 整体栈的闲置回归检查，不能把差值归因于 DLight，也不宣称零回退或加速。本次未修改 S3 CPU、通用总线或 tick 遍历，未重跑 Pocket Tank；历史结果继续保留。
