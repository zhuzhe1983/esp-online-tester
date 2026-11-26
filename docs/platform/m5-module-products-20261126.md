# M5Stack 含 ESP32 的 Unit / Module：五个独立 SKU

本批为 U215、U213、M141、U017、U109 建立独立产品、资料、固件调查和预览。五款板级状态均为 `unavailable`；Unit/Module 内含 ESP32 不等于其客体总线、无线、相机或外部主机连接已模拟。

| SKU | 来源与模型范围 | 独立官网尺寸 X×Y×Z（mm） | 当前完整构造范围（mm） |
| --- | --- | --- | --- |
| U215 CardKB2 | 自身照片与尺寸图；42个开关、独立印刷面、USB/Grove外观；真实4×11行列与扫描编号 | 84.7×54.3×6.15 | 84.5×54×5.365，厚度未通过官方外形对照 |
| U213 PoE-P4 | 固定原厂 STL 原位组件0/2，去掉重复排版1/3；独立背贴及 UNKNOWN 主板 | 24×64×20.2 | 24×64×20.225，额外0.025mm为背贴显示偏移；不代表内部装配完成 |
| M141 Gateway H2 | 本型号照片约束的中空框、PCB占位、排针、H2模块与拨码近似 | 54×54×13.1 | 54×54×13，照片几何不作制造公差验收 |
| U017 M5Camera | 有限官方图片近似含LEGO安装支架的外观，区分主壳、镜头与支架 | 40×48×11 | 40×48×11.06，仍需更多视角与实测 |
| U109 Unit CAM | 裸板、WROOM32E金属罩、FPC、镜头和Grove分层；无虚构白色外壳 | 20×45×12 | 20×43.2×9.145，长/厚度未通过官方外形对照 |

以上范围来自生产 Three.js 加载器中的完整对象包围盒，包含 `derivedParts`。没有通过缩放原厂 CAD 或拉伸照片掩盖偏差。未还原的零件、隐藏高度和连接器插合分别写入模型及知识的限制，不声称1:1完成。

CardKB2 的尺寸图区分84×54×1mm PCB、78×48mm安装孔距与6.15mm整机厚度。保留厂家固件 `M5Unit-CardKB2-UserDemo@282e4bc283635b501596221aec4d7fad2f19a361`、许可和哈希：ROW为 GPIO0/1/2/3，COM为4/5/6/7/8/29/10/11/22/23/24；(0,10)/(3,10)没有开关。42个稳定 ID、扫描编号、正面文字和实际位置保留在 `assets/m5stack-u215/keyboard-mapping.json`。按键射线命中通过不表示矩阵已接到 ESP32-C61；没有以 UART ASCII 注入替代按键。

U213 的官方 User Demo v1.0.0 与 UIFlow v2.5.5 已分别核对新版 M5Burner 的官方项目、精确设备和公开版本身份，固定远程版本 URL，保持未验证 `trial`、无虚构发布方 hash、无独立板型。没有下载或执行 BIN。其他 SKU 保存精确官方原厂/内部源码入口与未完成项；M141 的通用 Thread/Zigbee SDK 不冒充出厂固件，CardKB2 页面所写 UIFlow2 `coming soon` 不当作已发布镜像。小智没有证实精确匹配。

接口按资料记录信号，U017 GPIO13/4 与 U109 GPIO16/17 分开。没有来源证实的 Grove 线色、pin1方位和插入深度保持未知，关闭可视化连线；RJ45、SDIO、M-Bus、MIPI和无线传输没有以外观接好冒充真实协议。

```sh
python tools/build-m5-missing-models.py --output /tmp/m5-modules \
  m5stack-u215 m5stack-u213 m5stack-m141 m5stack-u017 m5stack-u109
node tools/check-m5-missing-models.mjs \
  m5stack-u215 m5stack-u213 m5stack-m141 m5stack-u017 m5stack-u109
python tools/validate-platform-catalog.py
node tools/platform-knowledge-test.mjs
node tools/platform-watchlist-test.mjs
```

构建默认离线，原图、CAD、源码、裁切和遮罩可复现；每个模型与输入均记录SHA-256。实际生产加载器检查包含42条独立按键最近射线、5个UNKNOWN在Inspect后的无遮挡几何射线、逐模型原色/金色/紫透材质隔离及精确复位。Node的纹理解码与Canvas使用替身；WebGL、屏幕可读性、照片叠图、客体固件和实机仍未验收。
