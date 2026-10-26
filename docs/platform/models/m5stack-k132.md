# M5Stack Cardputer · K132

ESP32-S3便携键盘终端，56键与1.14英寸LCD；本型号硬件差异与固件出版链接独立记录。

厂家 CAD 以毫米和刚体变换导入；本产品外观状态仍为 **partial**，本次只核验外观，固件运行支持以产品库为准。

装配：Selected manufacturer components retain their native assembled relationship; one common rigid transform only. Omitted components are separately recorded. No nonuniform scale or geometry fitting.

已检查正面、背面和侧面，以及原色/金色/半透明紫色正背对照。纸贴使用源照片的显式透明遮罩，独立于可换色外壳；照片不覆盖屏幕。缺失的字样、端子细节和内部板图不因本次检查而成为已还原。

几何检查共 10 项，0 项未通过；数值和照片轮廓测试的失败均保留在 [verification.json](../evidence/m5stack-k132/verification.json)。三种材质对照见 [appearance-comparison.webp](../evidence/m5stack-k132/appearance-comparison.webp)。侧面及斜拍只作定性核查，白底白壳、标注图和附带物可能使自动轮廓分割无效，不能用该指标证明整机1:1。

来源、抓取时间、hash与装配计划见 [source-audit.json](../evidence/m5stack-k132/source-audit.json)。复现：`python tools/onboard-lane-02.py prepare --ids m5stack-k132`、`build --ids m5stack-k132`，然后使用 `review-lane-02.html?id=m5stack-k132` 截取对应正背侧视，运行 `verify --ids m5stack-k132`。依赖见 `tools/requirements-model.txt`。

限制：

- Manufacturer exterior geometry only; exact product board/CPU/peripheral execution is unavailable.
- Photograph-informed material colors are uncalibrated display approximations.
- PCB UNKNOWN is an estimated blank placeholder, not a populated or manufacturing-ready board.
- External connector/body inserts are photo-derived estimates where manufacturer CAD omits them.
- No physical hardware measurements or flashing performed.
- Only the photographed white keyboard-paper strips are retained; actual key geometry stays separate, grey case and black keycaps are excluded from the masks.

## 原厂56键几何分离

厂家STL的component11共有39180个三角形：56个键帽各250个三角形，其余25180个留在carrier。按实际顶面连通性和实测包围盒排序为4行14列；没有新增热点方盒，也没有缩放、裁切三角形或修改法线。分离前后位置与法线的三角形多重集合哈希完全一致，既有贴纸、端子、估算表面和纹理哈希保持不变。

- [分离和官方源码回执](../evidence/m5stack-k132/keyboard-partition.json)
- [真实生产拾取器的逐键结果](../evidence/m5stack-k132/keyboard-picking.json)
- [原色、金色和半透明对照](../evidence/m5stack-k132/appearance-review.webp)

原版官方键盘源码固定在 `a34d7ebcd508fb903a2b1123be8c8b54abd55d07` 与 `f0b37ae5ebe709925fa54cca643418d96210f216`，两份主键表56项一致。原版74HC138扫描：选择GPIO为8/9/11，selector = 3-row + (column为偶数 ? 4 : 0)，输入GPIO为[13,15,3,4,5,6,7][floor(column/2)]。它与Adv的TCA8418硬件不同。

默认电脑键码映射遵从主字符；Fn→AltRight、Opt→MetaLeft是平台约定，不冒充厂家USB HID码。官方二进制版本含 `f0b37ae-dirty`，本报告不宣称可由这些公共源码精确复现，也不把新旧Fn/HID处理差异归为已验证固件行为。

实际 `manufacturer-model.js` 和 `scene-interaction.js` 中的透明贴纸感知最近表面拾取器验证了56/56正面和斜视命中；背面56键全部被外壳遮挡。原色、金色、半透明三种材质分别通过。外观其余partial限制继续保留；固件启动及按键语义由板级验证单独提供。

重生成：获取已记录固定URL与hash的官方源文件后，运行 `python tools/split-cardputer-classic-keys.py --ids m5stack-k132`；通过受跟踪headless浏览器打开模型审阅页，再运行 `python tools/check-cardputer-classic-picking.py --session <task-session>`。临时源目录为 `output/source/cardputer-key-partition/m5stack-k132`。
