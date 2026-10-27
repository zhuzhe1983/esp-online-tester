# Unit Hub U006 的并联连接

U006 的[官方原理图](https://m5stack-doc.oss-cn-shenzhen.aliyuncs.com/791/UNIT_HUB.pdf)把 J1–J4 的四根线逐脚相连。它没有 MCU、I²C 地址或软件选择器。这一轮把这层拓扑加入现有 StopWatch 配件路径；GPIO、RMT 和 ADC 仍由真实客体固件操作。仅支持一个直接连接 StopWatch Port A 的 U006，不适用于 PaHub/PbHub。

## 场景与输入

```json
[
  {"id":"hub","catalogId":"m5stack-unit-hub-u006","parameters":{},"wiring":{"mode":"physical","interfaceId":"grove"}},
  {"id":"button-unit","catalogId":"m5stack-unit-button-u027","parameters":{},"wiring":{"mode":"physical","viaAccessoryId":"hub","portId":"port-1"}},
  {"id":"rgb-unit","catalogId":"m5stack-unit-rgb-u003","parameters":{},"wiring":{"mode":"physical","viaAccessoryId":"hub","portId":"port-3"}}
]
```

把这组实例设为 scene.accessories。`port-1` 至 `port-3` 是平台从正面看左到右的出口名称，不声称是 PCB 上的 J1–J3 丝印。`interfaceId` 与 `viaAccessoryId/portId` 二选一。场景和分享保留拓扑；网页提供连接位置选择。添加 Hub 会将原来直连的一个受支持配件移到出口1；后来添加的配件使用空出口，信号争用仍拒绝。

U027 使用白线 G11，U003 使用黄线 G10，可以同时使用；U005 可以代替 U027，并以 ADC2_CH0 采样。U027+U005、两个 U003、同一个插口重复占用、嵌套 Hub、自环、孤立路径、未适配主机及未知出口都拒绝。三个插口并不意味着三套 GPIO；本轮最多连接两个占用不同信号线的已支持器件。没有新增任意 I²C 从设备模型。

`input({type:'accessory-connection',accessoryId:'hub',connected:false})` 断开上游。`observe().accessoryInputs` 中 `connected` 表示本段插接，子配件额外的 `effectiveConnected` 表示到主机的连续通路。下游仍插着时，前者保持 true、后者变为 false。它不代表 PMIC 已供电；实际供电由客体 M5PM1 寄存器决定。

Hub 断开把所有子设备输入放入一次 worker `text-batch`，在下一段客体执行前全部入队，返回一个客体时钟回执。按钮释放；RGB 断开熄灭，重新连接需要新 RMT 数据。Angle 保留机械位置，通路未接通时电压为零。单独拔出子配件不改变同伴。暂停时允许断开及按钮释放，恢复输入后生效。客体重启保留物理插接；平台 reset 恢复场景初态。录制保留 Hub 断开的语义，不展开成伪造用户点击。

## 运行验证

内部源码 fixture：`tools/fixtures/unit-hub-firmware`，ESP-IDF 5.5.1；仅 ignored output 中生成二进制，不加入网站固件库。

```sh
idf.py -C tools/fixtures/unit-hub-firmware -B "$PWD/output/unit-hub-build" \
  -D SDKCONFIG="$PWD/output/unit-hub-build/sdkconfig" set-target esp32s3 build
node tools/probe-unit-hub.mjs --build output/unit-hub-build
node --test tools/platform-*-test.mjs
python tools/validate-platform-catalog.py
```

实际 page worker/WASM 的10组检查涵盖 GPIO低/高、独立 RGB 数据、单子口拔插、上游批量断开、供电门控、暂停、两种重启、录制及 ADC 路径。Angle 满量程客体 raw=2786；拔出为0；断开时改为0.25，恢复后 raw=1087。浏览器使用真实鼠标命中 U027 CAD 按键和面板按键，固件分别读到0/1；mini 从分享场景恢复路径，再用本地 fixture 验证 Hub 拔出。未验证实物或任意第三方固件。

本轮没有改 Rust、总线、每 tick 路径或 WASM 二进制。使用 `53400cbf` 对应的相同 WASM hash，检查以新增 JS 路由/worker 分支和真实客体为准，没有重新跑 native CPU 时间对照。之前 U027、U003、U005 的性能增量及未解释项仍保留，不据此宣称整体性能无回退。

## 模型边界

厂家固定提交 `e29c139cbc7c80b5ccc5557e5ab8025a7a19a109` 的 STL 组件0/3已装配，组件1/2是平移打印副本。只做刚性居中；壳体24×32×8mm，整机官方尺寸24×32×10.8mm。三只立式插座中心、5mm安装孔与7.5mm间距对照官方尺寸图。背贴只裁纸面，未把灰色塑料贴成贴纸；内部用 UNKNOWN PCB 占位。

正面轮廓 IoU **0.9666**、背面 **0.9724** 通过0.95阈值；这是轮廓检查，不能证明完整1:1。斜视没有同视角标定，保留为定性检查。后盖螺钉与 M5 浮雕未完整补建，连接器隐藏深度、光照/表面细节和 PCB 布局仍近似。原色、金色和半透明紫色均查看过，插座和纸贴保持独立材质。

入口背贴可交叉核对镜像线序。三只出口的键槽可见，但缺少逐针方位/PCB资料；**出口不填 pinOrder、不显示连线**，UI 明确提示。不能用已经证明的并联网表反过来假装确定了三维针序。接线功能与外观精度分开报告。

公开重建不访问私有下载队列：

```sh
python tools/rebuild-unit-library.py m5stack-unit-hub-u006 --scratch output/u006-rebuild
```

来源、hash、抓取时间、模型比对、负面结果和运行收据见 `docs/platform/evidence/m5stack-unit-hub-u006/`。原理图属于电气依据，不能冒充 PCB 布局。

U136后续增加经过核验的I²C共用：ENV-IV0x44/0x76与DLight0x23可使用同一G10 SDA/G11 SCL。重复地址、不同针序或数字/模拟/RGB输出争用仍拒绝；未扩展为任意设备通用I²C。
