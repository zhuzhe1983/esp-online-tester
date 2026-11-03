# M5Stack StampFly · K138

本模型采用 StampFly Ecosystem 的分件 STL、M5Stack 的 StampS3 外壳 CAD，以及 K138 官方照片。机架、PCB、电池、电池支架和四个电机保留源模型的尺寸与装配坐标；外观状态仍为 **partial**。本页记录外观证据，固件执行范围以产品目录为准。

Ecosystem 固定于 `ac0ed6f67d609a6f7e989f61be0a6d1e0e311906`，MIT 许可。它是官方文档链接的模拟器项目，并非已经核实的厂家生产 CAD。原始合并 STL 和分件网格均保留来源；M5Stack K138 目录里的电池保护件也另存为 `source-battery-protection.stl`。StampS3 外壳取自 M5_Hardware 的固定版本，18 × 24 × 4.7 mm，不再使用 Ecosystem 的粗略模块方块。

PCB 使用源模型轮廓和黑色基材，正反面外露部分分别投影官方照片，并按实际板面裁切。照片原图为 800 × 800，PCB 裁片为 260 × 372 像素；没有放大后冒称更高清素材。抬高的模块、接口、螺丝和按钮区域从板面照片中排除。载板背面采用四个安装孔中心透视校正，排除导线、插头和机架横梁。光流小板在独立高度贴图，13.2 × 21.6 × 0.8 mm 及其深度为照片估计（深度不确定性 ±1.5 mm），其未观察到的内侧标记 `UNKNOWN`。

四个螺旋桨从源模型的圆盘改为照片支持的四叶轮廓，采用浅橙色、38% 不透明度并区分左右旋轮廓；补齐可见蓝色减振圈、螺丝头及六角凹口，并用开口接口替换封闭方块。螺纹、桨距和隐藏连接件尺寸仍未核实。模块纸贴为 17.6 × 23.2 mm 的独立纸面，位于 18 × 24 mm 外壳内侧，上方留出 0.028 mm，覆盖源壳的开孔和凸字，不再穿孔或超出边缘。

源坐标仅做刚体变换 `X=-sourceX, Y=sourceZ, Z=sourceY`，比例为 1。实测外廓 81.842 × 81.842 × 31.528 mm，官方尺寸为 81.5 × 81.5 × 31 mm；三轴分别偏差 0.342、0.342、0.528 mm，均未通过原有 0.05 mm 容差。十一项源网格保真检查通过，失败的整机尺寸检查完整保留，没有放宽容差或缩放凑数。

官方正反侧照片均拆下了电池，因此照片对照显式隐藏 `battery`；默认模型及材质对照仍装有源电池。正反侧照片、叠图和不同材质见 [照片对照](../evidence/m5stack-k138/photo-comparison.webp)、[材质对照](../evidence/m5stack-k138/appearance-comparison.webp) 和 [斜视细节](../evidence/m5stack-k138/detail-comparison.webp)。金色及半透明紫色只改变机架，PCB 黑色、橙色模块、纸贴和螺丝保持独立。白壳与白背景无法可靠自动分割，照片检查仅作定性核验，不能证明整机 1:1。

来源、抓取时间和 SHA-256 见 [source-audit.json](../evidence/m5stack-k138/source-audit.json)；尺寸、浏览器相机和截图收据见 [verification.json](../evidence/m5stack-k138/verification.json)。此前的失败记录保留于 `history/2026-10-26/`，便于比较。

复现时使用 `tools/requirements-model.txt` 中的 Python 环境：

```sh
python tools/rebuild-product-model.py plan m5stack-k138
python tools/build-stampfly-model.py --scratch /tmp/stampfly-rebuild-review
python tools/test-product-model-exporter.py
node --test tools/platform-manufacturer-model-test.mjs tools/platform-model-test.mjs
```

重建默认离线，输出到新建的 scratch 目录，逐项核对原始文件和生成资源。浏览器使用 `web/platform/review-lane-02.html?id=m5stack-k138`，800 × 800 画布；各视角参数在核验收据中。照片对照前将 `laneReview.current.model.parts.battery.visible=false`。默认、金色和半透明紫色均检查正反面，另检查侧面、斜视、贴纸及 UNKNOWN 特写。

G0 点击区域沿用同一份 StampS3 CAD 已核验的机械盖片内侧：5.2 × 3.2 mm，中心为 `[2, 2, 15.8]`。它对应模块按键，位置与贴纸的 BTN0 文字中心略有区别；载板 S1 的可见顶面也绑定同一 `button`：原理图中 USER_A 经 P2.3 接 Stamp G0，按下接地，与 EN/RST 分开。运行时仍需目录声明 `button` 控件。

背面照片能显示实际可见布局，但仍未还原其电路细节、导线、光流镜头和连接器触点的准确三维结构；照片颜色未经反射率校准，四叶桨是平面视觉近似。模型证据不证明电机转速、飞行动力学或实物电气行为。
