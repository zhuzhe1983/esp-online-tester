# CoreS3 K128 前部与复用装配

本次在原厂 CoreS3 CAD 上补充摄像头、双麦克风和 LTR553 的可见窗口，修正前玻璃轮廓及 LCD 中心。窗口的平面坐标和尺寸来自官方 DXF；薄层厚度、镜片颜色与反射为展示估算。没有构造隐藏的传感器封装或 PCB 元件，也没有改变摄像头、麦克风或 LTR 的模拟行为。

## 来源与尺寸

固定的 [M5Stack CAD](https://github.com/m5stack/M5_Hardware/blob/e29c139cbc7c80b5ccc5557e5ab8025a7a19a109/Products/K128_CoreS3/Structures/CoreS3.stl) 仍是头壳来源，保留原始 STL 和 MIT 许可文本。原厂 [CoreS3 产品页](https://docs.m5stack.com/en/core/CoreS3) 标明 54×54×15.5 mm 主机模块，以及 GC0308、双麦克风与 LTR553。[官方 DXF](https://m5stack.oss-cn-shenzhen.aliyuncs.com/resource/docs/products/core/CoreS3/cores3.dxf) 保存在模型目录，SHA-256 为 `c97a042b573bdc2e0f7c09897420b9352ccc941a7705ce18caf59356fb17fc42`。DXF 与照片未找到独立授权声明，不继承 STL 的 MIT 声明。

下表使用模型坐标：X 向右、Y 向上、Z 朝屏幕外。DXF 的 54 mm 尺寸线确定平面原点，坐标由实体直接读取，没有以照片反推比例。

| 项目 | 尺寸/中心，mm | 依据 |
| --- | --- | --- |
| 头壳 | 54×54×15.5；后接合面 Z=0.25，前面 Z=15.75 | 厂家 STL component 0、产品页 |
| 前玻璃 | 50.8×50.8，R3.4 | DXF TMP_DWG 轮廓实体 71E/75B/724/75F |
| LCD 光学窗口 | 41.6×31.4，中心 (0,0) | DXF 734/756/759 |
| 实时 LCD 平面 | 40.64×30.48，中心 (0,0,15.85) | 2 英寸 4:3 标称面积；Z 为估算 |
| 摄像头环/窗口/内镜片 | 中心 (0,-21)，半径 1.9/1.5/1.2 | DXF 10B8/10BA/10BB |
| 左/右麦克风孔 | (-14,-20.25)/(14,-20.25)，半径 0.5 | DXF 109A/10BC |
| 两个 LTR 窗口 | (6.58,-21)/(8.98,-21)，半径 0.5 | DXF 10BD/10BE |

前玻璃 Z=15.8，即比厂家壳体前面多 0.05 mm 的显示偏置；这不是实测玻璃堆叠厚度。新增窗口最前面 Z=15.844，且全部在 LCD 下方。前玻璃由旧的 52 mm/R3 改为 DXF 的 50.8 mm/R3.4，LCD 中心由旧 Y=3 改为 Y=0，纹理方向保持不变。

## StackChan 等宿主如何复用

[front-assembly.json](../../web/platform/assets/m5stack-k128/front-assembly.json) 固定完整几何文件的 hash，但只选择 `shell-front`、`sensor-window-support`、`front-glass` 三个网格，外加七个窗口和 `live-display`。保留厂家两个独立组件，不把深色传感器支撑片当作可改色外壳。标准 DIN 底座、四只安装脚、卡扣、旧主板占位及 USB/Grove 估算连接器全部排除。

宿主将 `rearMatingPlane` 对齐自己的 CAD 接合面，以刚体变换移动选定网格、窗口、LCD 及整套语义 ID。不能把玻璃贴到宿主的后接合面而省略 15.5 mm 主机厚度。`sourceCad.matrixRows` 保留原 STL 到 CoreS3 模型的变换；从原 STL 重新导入时，使用 `hostGlobal⁻¹ × desiredWorld × sourceCad.matrixRows` 作为局部变换。没有照片贴图层，因此没有额外的 `-photo-surface` 子网格需要补入舵机组。

## 验证与边界

2026-11-01 UTC，Headless Chromium 156 使用实际 `createManufacturerModel` 加载器核对原色正面、侧面、背面，金色正面及紫透正面/背斜视。镜片、麦克风孔、LTR 窗口、前玻璃和深色支撑片保持独立材质；紫透背斜视可看到原有 `UNKNOWN` 主板占位。重新载入原模型后再拍摄原色侧后视，避免只复位颜色而残留透明材质状态。

浏览器重新抓取的几何 SHA 为 `7343b46f3eb7f0a1b45d1049596c5ed33bb779e9dbe2fa9c48a980590eb7585c`，与 metadata 一致。LCD 的九个边角/中心采样全部首先命中实时屏幕。Node 测试另外检查五个光学窗口的最近命中、两种改色下的材料隔离及复用清单不含 DIN 部件。没有把产品照片上的画面贴到屏幕；本次是几何与命中验证，不是固件执行或实机测试。

官方斜正面照片 `product-photo-86528c8744` 用玻璃四边的手工地标拟合透视；相机、双麦、LTR 与 LCD 中心相对 DXF 的误差为 0.81–1.38 mm。拟合参数、像素地标和逐点误差保存在[紧凑回执](m5stack-k128-front-verification.json)。圆角边缘、反光、斜拍透视和玻璃厚度限制了校准精度，因此只确认可见布局，没有宣称完成制造精度的 1:1 照片验收。侧面连接器、背部标识和内部主板仍沿用已有简化模型，不属于本次精确还原范围。

来源时间、版本 hash、重建结果、浏览器参数与截图 hash 也记录在回执。原始截图留在忽略的 `output/playwright/cores3-front/`，不提交；没有上传外部证据副本。

## 复现

安装 `tools/requirements-model.txt` 后，在仓库根目录执行，scratch 必须是新目录：

```sh
python tools/rebuild-cores3-front.py --check
python tools/rebuild-cores3-front.py --write --scratch /tmp/cores3-front-rebuild
python tools/rebuild-product-model.py build m5stack-k128 --scratch /tmp/cores3-front-independent
node --test tools/platform-cores3-front-test.mjs tools/platform-manufacturer-model-test.mjs tools/platform-servo-view-test.mjs
```

专属重建工具校验 DXF 原始 hash，并从实体生成 plan、模型及复用清单。通用离线重建已验证六个输出文件精确一致；它不消耗新增 DXF，DXF 的校验由专属工具承担。浏览器使用 `web/platform/review-lane-02.html?id=m5stack-k128`，镜头与材料参数见回执；按仓库 Playwright 生命周期要求关闭自有会话。
