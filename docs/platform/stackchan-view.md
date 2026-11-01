# StackChan 舵机、触摸板与 RGB 的三维显示

K151 的显示模型现在按板级反馈转动水平和俯仰关节。屏幕、三块顶部触摸区、RGB 光窗、连接器和 `UNKNOWN` 主板随头部一起运动；底座保持原位。模型本身不发出舵机指令，也不按目标角度播放预设动画。固件断开、切换或清空观测值时恢复默认姿态；断力后继续显示反馈位置。

`sceneView.updateServos()` 接收板级观测的 `[{id:'pan', positionDegrees}, {id:'tilt', positionDegrees}]`；`targetDegrees` 和 `enabled` 不触发显示层推算。RGB 继续使用共享的 `updateLeds()`，三块触摸区沿用按下、按住、释放及多指输入通路，ID 为 `touch-pad-front`、`touch-pad-middle`、`touch-pad-back`。屏幕仍是独立的坐标触摸面。

## 坐标与依据

原始几何、毫米尺度、静态装配变换和厂家许可全部保留。关节配置在 `assets/m5stack-k151/model-plan.json` 与 `model.json` 的 `presentation.servoJoints` 中，可以通过公开重建脚本恢复。

| 内容 | 当前依据与范围 |
| --- | --- |
| 水平关节 | 舵机外壳 CAD 的底部圆环中心，轴线 `(0, y, -8.0968)` mm；参考高度 `y=-31.6` mm |
| 俯仰关节 | 舵机外壳 CAD 侧面轴孔中心，轴线 `(x, 0, -8.0968)` mm |
| 转动方向与初始姿态 | 保留导入模型的中立姿态；正俯仰显示为抬头。实物零点、装配方向和机械极限没有实机校准 |
| 顶部触摸 | 官方顶部照片确认可触摸外壳位置；前、中、后三块命中区域为估算，隐藏电极轮廓未知，不绘制虚构的物理边界 |
| RGB | 两侧实际 CAD 长孔内各设置六个光学显示段，逐项显示板级十二颗 LED 的 RGB 值；链序与左右空间位置尚未校准，不能把显示段当作十二个独立灯孔 |

顶部照片使用既有来源 `product-photo-6ce7959e87`，本次重新取得的内容与原记录 SHA-256 一致，保留原始抓取记录。照片是参考资料，屏幕不贴宣传图。来源、变换和派生位置均可在模型元信息中查询。

## 验证

执行：

```sh
node --test tools/platform-servo-view-test.mjs tools/platform-manufacturer-model-test.mjs tools/platform-view-test.mjs
python3 tools/rebuild-product-model.py plan m5stack-k151
python tools/rebuild-product-model.py build m5stack-k151 --scratch <new-scratch-directory>
```

重建使用 `tools/requirements-model.txt` 中的固定依赖。本次七项 Node 测试通过；离线重建的十二个资产及模型契约一致。`modelJsonByteIdentical:false` 单独保留，未把元信息字节顺序与几何重建一致混为一谈。

测试直接加载实际 CAD，检查中立姿态不移位、两级转轴组合、屏幕/连接器/主板随头部移动、重复复位无漂移、停用时保持反馈位置、缺失值复位，以及三个顶部触摸区的遮挡和十二个光窗的射线可见性。无效轴线、重复零件和祖先循环被拒绝。RGB 与外壳材质独立。

[本次浏览器图与资产校验值](evidence/m5stack-k151/runtime-view-2026-11-01/receipt.json)记录原色侧视、俯仰 60°、顶部 RGB、金色与半透明紫色；后视可辨认 `UNKNOWN` 主板。浏览器图通过明确的测试状态驱动渲染器，**仅证明显示与材质行为，不是固件执行或实机验证证据**。可在 `review-lane-02.html?id=m5stack-k151` 使用 `laneReview.current.group.userData.servoJoints` 配合 `applyServoValues()`、`applyRgbLedValues()` 重现；相机和输入值见回执。

现有模型仍为 `partial`：原先的舵机上盖装配间隙、缺失的连杆/内部件和外观偏差没有在本次悄然修整，也没有通过缩放消除。关节预览不包含碰撞、扭矩、负载、光学扩散或机械安全保证，不能据此声称内部结构已按 1:1 还原。固件功能验收应另看板级运行报告。

原始浏览器截图按证据保留规则移至本地忽略目录 `output/playwright/stackchan-view/`，未发布公共下载地址。回执保留其 SHA-256、大小和输入状态；截图不包含在源代码交付中。
