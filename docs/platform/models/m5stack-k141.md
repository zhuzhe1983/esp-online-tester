# StamPLC K141 模型修正与核验

StamPLC 现在保留原厂全部已选装配几何，修正 Stamp 小板纸贴和四组继电器标签，补齐照片确认的四颗后盖螺丝头、四个 M3 铜套可见边缘，并将 A/B/C 按键与真实 RGB 导光件接入已有运行时。主板和 Stamp 小板的未知布局仍明确保留；整机模型状态为 `partial`。

原厂 [StamPLC STL](https://github.com/m5stack/M5_Hardware/blob/e29c139cbc7c80b5ccc5557e5ab8025a7a19a109/Products/K141_StamPLC/Structures/StamPLC.stl) SHA-256 为 `91745b5fc0d2a4bb7ac866271b03bcbd5324fbde2b7e51c220cb66fdfe4470d6`。厂家当前公开结构目录仍只有该外壳文件，未找到完整 K141 板级 CAD。所选装配包含 **22496 个原生三角形**，没有缩放、删除或补猜原生零件。A/B/C 刻字底面和原生盲孔底面只重新分组着色；排序后的完整位置／法线记录哈希完全一致，见 [native-face-partitions.json](../evidence/m5stack-k141/native-face-partitions.json)。原始装配之外的排版副本仍按原记录排除。

Stamp 纸贴的错误来自坐标配准：它使用了包含背面 DIN 卡扣的 80 mm 整机高度，而前壳实际承载范围为 Y=−37…40 mm，共 77 mm。原贴图包含抗锯齿边缘时，向下超出原生 Stamp 载体约 **1.076 mm**；旧记录保留在 [previous-stamp-paper-overhang.json](../evidence/m5stack-k141/previous-stamp-paper-overhang.json)。新配准只改变照片平面，原生 24 × 18 mm 载体保持不动。最终 37849 个可见纸贴像素全部位于 X=9.8…33.8、Y=4…22 mm 的载体内；像素中心范围为 X=10.485…33.254、Y=4.666…21.253 mm。原图确认纸贴本来跨过载体内的浅槽，模型没有用虚构几何填槽。

底部四组继电器文字原来被整体投影截断。现在每组独立映射到已有的 15.8 × 4 mm 标签区域，位于 Z=10.2 mm 凹面上方 0.015 mm；避开端子孔和凸起隔筋。四组逐像素垂直射线检查全部落在原生承载面，没有高出台阶交叠或无支撑像素，见 [relay-paper-support.json](../evidence/m5stack-k141/relay-paper-support.json)。源图裁切、透明遮罩和公共重建计划均可复现；0.015 mm 是渲染分离量，并非测得的纸厚。

[官方功能标注图](../../../web/platform/assets/m5stack-k141/reference-product-photo-e38c32e489.webp)与[背面照片](../../../web/platform/assets/m5stack-k141/reference-product-photo-78481c3a59.webp)共同确认四个 M3 铜套孔、四颗后盖螺丝和前面的 RGB 导光件。

| 外观部件 | 模型依据与边界 |
| --- | --- |
| 四颗后盖螺丝头 | 原生沉孔中心为 X=±32、Y=25／−1 mm；保留原生承座。3.4 mm 头径、1.5 mm 高度和六角凹口是约 ±0.2 mm 的照片估算，未编造隐藏螺杆或螺纹。 |
| 四个 M3 铜套可见边缘 | 中心为 X=±24、Y=30／−26 mm；原生 1.5 mm 半径盲孔保持不变。只增加照片可见的 4 mm 外径铜色环面；0.05 mm 显示厚度不代表整个铜套深度。 |
| RGB 导光件 | 官方 LED 箭头指向顶部窄槽，正好对应原生独立组件 0。该组件原名 `upper-latch` 现改为 `front-rgb-light-pipe`，绑定 LED 0，并退出外壳改色分组。说明纸上的 LED 图标没有被当成灯孔。 |
| Stamp 小板与 USB-C | 小板只保留 24 × 18 × 1 mm 包络和照片确认的黑色板边，安装间隙属于估算；`UNKNOWN` 放在背面。USB-C 使用朝产品 +X 的中空外观口，位置约 ±0.5 mm。PIN1.27 排针装配、铜箔与内部器件未知。 |

内部 Stamp RGB 被不透明纸贴遮挡，没有猜测可见灯孔；它的真实运行时数值仍可读取。主板没有完整 CAD，继续保留中性 UNKNOWN 占位。通过公开的半透明外壳设置与真实旋转，在完整装配中同时核对主板和小板的文字可读性，见[公开查看回执](../evidence/m5stack-k141/public-unknown-review.json)。旧的隐藏其他零件截图只用于零件近景，不能单独证明普通用户可见。金色和半透明紫色仅改变外壳，不改变纸贴、铜套、螺丝、PCB、键帽或 RGB 导光件。

原生 CAD 尺寸 **72 × 80 × 33.4 mm** 通过官方尺寸目标。浏览器把新增可见零件计入后为 **72 × 80 × 33.415 mm**，多出的 0.015 mm 来自铜色环面的渲染偏移，仍在 0.1 mm 检查容差内。正面轮廓 IoU **0.935465**、背面 **0.922230**，均未达到 0.95；厂家 CAD 的卡扣／外壳轮廓与照片投影差异保留，没有为提高分数缩放模型。侧面 **0.892094** 仅作定性比较。标注图是来源证据，不是轮廓验收视图。完整原图、渲染、叠图和失败结果见 [verification.json](../evidence/m5stack-k141/verification.json)；不能据此称整机已达到 1:1 精度。

真实工作台运行独立构建的 ESP-IDF 5.5.1 测试固件。A/B/C 三个原生键面的鼠标按下／释放，以及 KeyA／KeyB／KeyC，各自触发新的 PI4IO 状态和中断读回；客体通过实际 SPI 把 240 × 135 个像素全部改为对应红、绿、蓝，RGB 输出同时匹配，释放后恢复四象限图。每次核验同时要求新增 UART0 行、对应 RGB 值和完整 RGBA 哈希，见 [workbench-input.json](../evidence/m5stack-k141/workbench-input.json)。不是仅以热点命中或前端颜色变化作为证据。

另外加载精确的[官方出厂镜像](https://m5burner-cdn.m5stack.com/firmware/714fd486771a7a4aec72e54cb6031570.bin)，SHA-256 `abd1a4345353ce0c166fbed6b89c3feb7d65ca2a5f37a05a34d1271405aabf2e`。真实 3D 按键完成 Dashboard → A 返回 Launcher → B 选择 → C 打开 LogMonitor 1；B 单独没有新串口行，其选择效果由随后打开的应用确认。截图在两个动画帧和 100 ms 渲染等待后取得，早期抢先截图的陈旧画面单独保留。见 [factory-browser.json](../evidence/m5stack-k141/factory-browser.json)。`simulationVerified` 仍为 false；传感器、RTC、SD、工业负载和外部总线等限制见 [StamPLC runtime](../stamplc-runtime.md)，未实机验证。

公开入口离线重建 **13 个生成资产和完整模型配置**，全部一致：

```sh
python tools/rebuild-product-model.py build m5stack-k141 --scratch <fresh-scratch-directory>
```

[重建回执](../evidence/m5stack-k141/rebuild.json)保留依赖、工具、输入和最终资产哈希。独立 Stamp-S3A 尺寸图作为额外来源保留；来源抓取时间未冒充刷新时间。平台配置、知识、视图测试及知识 JSON schema 通过。任务浏览器已关闭并确认退出；CAD 与照片分别保留各自许可和版权说明。
