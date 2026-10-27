# NanoC6 C125：官方出厂固件与基础板级适配

原版 NanoC6 的官方出厂 v1.1 已从完整镜像经 C6 ROM 启动。新增 `nanoc6` 板型，启用目录中的运行入口；默认仍是官方出厂固件，官方 UIFlow2 条目仍可试运行但未作兼容验收。固件仅保留远程地址，运行时下载至调用方内存/浏览器缓存，不复制到网站。

## 真实执行与边界

固定镜像 SHA-256 为 `6ad2a3d3714b29ab9456f07d937e99065d2cb453afca2646d66325d6fc9c7178`（928432 字节）。它实际打印 `nanoc6_factorytest`、应用版本 `aa0ba2b-dirty` 和 IDF `v5.2-166-ge0944287f3-dirty`。产品页的公开 Arduino 示例用于引脚依据，不声称它是该镜像的完整构建源码。

- G9 低有效按键通过真实 GPIO 输入。官方固件在按下再释放后切换 `example: status: 1/0`；独立 WASM 探针、工作台 3D 鼠标与键盘 A 均观察到新输出，不以输入入队代表成功。
- G20 的 RMT GRB 数据驱动单颗 WS2812；G19 经 GPIO 输出路由控制 AW35122 的电源。禁用输出、错误 IO_MUX、断电、复位会熄灭，恢复供电必须等新的 RGB 帧。测试包含错误数据脚与错误电源路由。
- G7 高有效蓝灯独立于 RGB。官方固件的红绿蓝循环和蓝灯亮灭均来自客体输出，前后灯窗只是呈现这些状态。
- 原生探针运行 30 客体秒；工作台和 mini 均执行真实官方镜像。NanoC6 没有屏幕，未添加虚构 framebuffer。

官方镜像仍报告 `esp_timer: Task is already initialized` 和 `SH1107: OLED configuration failed. code: 0xFFFFFFFF`。未连接出厂测试用的外部显示器，不伪造成功应答。红外光学输出、Wi-Fi/BLE/802.15.4 完整网络、外部 Grove 器件和模拟电气未验证；全部结果均未实机比对，因此固件仍为可试运行的 partial 状态。

## 来源、共享路径与模型

[固定来源清单](evidence/nanoc6/sources.json)包括官方产品页、v0.0.1 原理图、M5NanoC6 `7cf509694316c9acb41e98658fb0ddb8ab6db790` 示例、镜像与对应 IDF 头文件的 URL、UTC 抓取时间和 hash。G9 的 R2 10 kΩ 上拉、G7 LED 极性及 G19 电源开关由原理图核对；厂商照片确认前 RGB 槽和后 USB 端中央蓝灯位置。IDF e0944287f3 与 v5.5.1 的测试所用 GPIO/IO_MUX 字段一致。

先检查并复用上游开放 PR [#262](https://github.com/mikroverk/esp32sim/pull/262) 的首个源码提交 `14c1f0435a4eb4915a7d7aac6da14bd5a2a7a517`，本仓库 `e5ac3605` 保留原作者。只引入 C6 板型拆分及其源码测试，未导入后续公共固件二进制和 CI demo。C6 总线沿用已有 `uses_gpio_routes` 合同，在 GPIO/IO_MUX 写入和板级挂载时通知需要静态路由的设备；没有增加逐指令工作。默认合同与原 `uses_gpio_waveform` 一致。

厂家 STL 壳体和三角形保持不变：12.000 × 23.4698 × 9.500 mm，对官方 12 × 23.5 × 9.5 mm 的误差小于 0.1 mm。独立重建复核 8 个资产及 model.json。G9 的点击面贴合盖板可见按钮；RGB 位于 CAD 狭缝内，蓝灯位于背面下端中央槽内。内部 PCB 继续是带 `UNKNOWN` 的空白占位；灯的隐藏深度和光学面积仅是照片推算，不冒充元件布局。

Grove 去掉旧的实心白色派生块，复用 `4670491d` 的显式 `open-roof` 通用插座选项并将开口朝前，与官方正面照片的可见针道对应。旧产品不传该选项时的几何、材质、索引、变换保持逐项相同。NanoC6 插口隐藏深度、逐针方位与精确插合尚未核实，因此 `allowConnections:false` 保持；无连接动画不妨碍已验证的按键/灯光。外壳原色、金色、紫透及前后视图已查看，插座不随外壳改色，紫透可识别 UNKNOWN。背面纸贴尚未补齐，材质与完整照片 1:1 验收仍为 partial。

## 复现与检查

```sh
cargo run --release -p esp32c6 --example nanoc6_probe -- <temporary-official-bin> web/wasm/fw/esp32c6_rev0_rom.elf
node tools/probe-nanoc6-official.mjs <official-https-url> <sha256>
uv run --python 3.11 --with-requirements tools/requirements-model.txt python tools/rebuild-product-model.py build m5stack-c125 --scratch <new-scratch>
node tools/check-dlight-connector-compat.mjs
```

[检查收据](evidence/nanoc6/checks.json)记录 stable Rust 1.99.1、双 Clippy、1438 Rust/132 组、11 WASM、100 平台文件、真实录制回放、111126 JIT/24 GPIO、CI 时序/历史检查。golden 未变。[官方 WASM](evidence/nanoc6/official-wasm.json)、[浏览器](evidence/nanoc6/browser.json)和[模型重建](evidence/nanoc6/model-rebuild.json)分别记录输入与输出，不互相替代。

保留的负向结果：最初把官方切换误判为按下触发，等待超时；改为观察完整按下/释放后获得状态切换。LED 订阅会先收到空数组的配置重置，探针必须允许该状态。模型点击元信息曾缺少 shape，实际渲染拒绝；修正后重新执行真实鼠标验收。新增 Grove 可选项使旧收据的 renderer hash 失配；通过默认几何兼容比较更新附加收据，原有照片时间和指标不改写。未将任何一次失败当成通过。

[五对 CPU 对照](evidence/nanoc6/cpu.json)使用独立保全的固定 main `84e67571`：S3 −4.00%（0.25→0.24 秒）、C3 −0.862%、C6 +0.633%；全部 UART、指令数和周期逐对一致。小幅差值受百分之一秒计时量化与宿主噪声影响，不作性能优化归因；未改 S3 核心，无须 Pocket 对照。最初脚本误比较带宿主吞吐量的整条 stop 行，已纠正为实际指令与周期字段，未将失败轮计入采样。浏览器与临时服务均已关闭。
