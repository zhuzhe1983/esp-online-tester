# CoreMatrix C158 模型

模型使用 M5Stack 官方 C158 外壳 STL，固定于 `M5_Hardware` 提交 `ec351c516644b44defc145728cc029ced77381ff`。全部 17,152 个原始三角形各保留一次；外壳、四个柔性按键及原有凹刻字仅按材质与输入语义分组。原始坐标只平移 `[-27, -27.871290683746338, +5.8] mm`，没有缩放或镜像。正面为 +Z，上侧三个键为 +Y，USB-C / Grove 在 −X 侧。

官方资料：[产品文档](https://docs.m5stack.com/zh_CN/core/CoreMatrix)、[原厂结构文件](https://github.com/m5stack/M5_Hardware/tree/ec351c516644b44defc145728cc029ced77381ff/Products/C158_CoreMatrix/Structures)、[尺寸图](https://m5stack-doc.oss-cn-shenzhen.aliyuncs.com/1274/C158_CoreMatrix_model_size.pdf)。逐源的原始地址、最终地址、实际下载时间、媒体类型、字节数和 SHA-256 在 [来源回执](../evidence/m5stack-c158/source-audit.json) 与 [知识记录](../../../web/platform/knowledge/m5stack-c158.json) 中。

模型保留独立的前片、256 个白色 LED 光学窗口、USB-C 空腔与双面触点、红色 Grove、两个后向 MX1.25 插座、microSD 插口、30 孔 M-Bus、四个带凹六角孔的螺丝头、背贴和侧面品牌贴纸。原厂只提供外壳 CAD，这些补充件均标明照片或图纸依据及未测量的内部深度。USB-C 的口沿使用原厂外壳孔位定位，插入空间为空腔，内部舌片与背壁保留深度。

背贴从官方正背面照片逐点描出真实纸边，并避开螺丝、USB.PORT、电池座和 Grove。纸贴有独立薄片承载面；原图内的连接器、螺丝和外壳没有烘焙到纸贴上。侧面贴纸只保留 CoreMatrix 白纸区域，M5、USB、电源、按键字符均保留原始凹刻网格。纸贴、LED、按钮、接口和螺丝不跟随外壳改色。

点阵直接使用 `ledIndex = y × 16 + x`：从正面看，索引 0 位于左上，15 右上，240 左下，255 右下。LED 中心的照片估算间距为 X 方向 3.00 mm、Y 方向 2.93 mm，窗口为 0.95 mm 方形。前片 51.23 mm 边长与 R3.62 圆角来自尺寸图；LED 的位置、大小、扩散和亮度没有光学校准。模型没有静态点亮纹理，也没有遮盖实际输出的屏幕照片。二维 framebuffer 与真实固件协议由板级运行时另行验证。

四个交互面对应 `button-a`、`button-b`、`button-c`、`power`。A/B/C 在 `[−13/0/+13, +27, −0.1] mm` 附近；电源在 `[−27, +13.5, −0.7] mm`。Grove 正视左到右为 G1/SCL、G0/SDA、5V、GND，与该型号的正侧面丝印、背贴和引脚表交叉核对。插口外法向 −X，口沿中心 `[-26.72, -14, -1.75] mm`；隐藏的插合深度采用已有连接器可视模型的估算，不声明制造配合精度。

内部没有公开 PCB、Gerber 或完整板级 CAD。主板使用照片可确认的蓝色和明确的 UNKNOWN 标识，板形、厚度、位置仅受外壳空间约束。公开“分开查看”可以将它移到侧面并读到 UNKNOWN；该查看方式不是实际拆机顺序，也不证明隐藏器件已还原。

| 验证项 | 结果 |
| --- | --- |
| 原厂三角形覆盖 | 17,152 / 17,152，无遗漏或重复 |
| 完整装配包围盒 | 54.000000 × 54.000002 × 11.600000 mm |
| 原厂尺寸图 | 54 × 54 × 11.6 mm，0.05 mm 数值容差内 |
| 正面 / 背面轮廓 IoU | 0.9906 / 0.9977 |
| USB侧 / 品牌侧轮廓 IoU | 0.9770 / 0.9811 |
| 按键侧 / SD侧轮廓 IoU | 0.9848 / 0.9705 |
| 四个按键命中 | 各自可见面命中；正面不能穿透操作侧键 |
| LED 索引 | 256 个独立窗口，四角独立值可见 |
| 改色、透明、展开 | 纸贴和接口材质独立；UNKNOWN 可读；复位无位移累计 |
| 离线重建 | 17 个资产一致，模型元信息逐字节一致 |

照片轮廓分数只比较外边界，不能证明光照、纸贴每个字、内部器件或固件兼容。照片中的灯光有曝光和扩散，当前材质没有复刻这些光学量；前片厚度、纸片支撑、螺丝隐藏部分和连接器内部结构仍有估算。没有实物测量，因此不声明整个产品各角度 100% 还原。

结果绑定的模型 SHA-256 为 `d7e6188512eda34b68979ff5c706182a40d657a85a147ee6ce76ef56ea6687d2`。完整数值与图像见 [验证报告](../evidence/m5stack-c158/verification.json)、[六视图及材质检查](../evidence/m5stack-c158/material-review.webp)、[浏览器回执](../evidence/m5stack-c158/browser/geometry.json)。图中的点亮值是明确标记的几何检查输入，不是官方固件运行证据。

复现使用仓库通用入口，无需历史任务文件：

```sh
python tools/rebuild-product-model.py plan m5stack-c158
python tools/rebuild-product-model.py build m5stack-c158 --scratch /tmp/corematrix-rebuild
```

浏览器检查通过本地静态服务器打开 `tools/fixtures/corematrix-model-review.html`，使用仓库要求的 Playwright wrapper，以 headless 会话执行 `tools/fixtures/corematrix-model-browser-check.js`；将其返回 JSON 保存到 `docs/platform/evidence/m5stack-c158/browser/geometry.json` 后运行：

```sh
python tools/verify-corematrix-model.py
```

原厂 CAD 保留 M5_Hardware MIT 许可；官方产品照片及其裁切纹理仍属于 M5Stack，没有重新标为本项目 MIT。
