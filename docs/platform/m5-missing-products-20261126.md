# M5Stack 缺失产品补齐：五个独立 SKU

本批增加 K159、K162、K016-P-SE、C156、A172 的独立产品、知识、模型和固件调查。五款的板级运行均为 `unavailable`，模型为 `partial`；没有把同外观或同芯片的现有运行器套给它们，也没有执行或镜像第三方固件。

| SKU | 本次模型来源与具体变化 | 保留的偏差 |
| --- | --- | --- |
| K159 PaperDIY Assembly Kit | 独立开放结构产品；官方比较表证实与 Complete Kit 共用主控/屏幕，复用已由 K159 实物照片约束的板级外观和 USB 金属件，移除打印外壳及外壳按键 | PCB 历史照片测量宽约63.55mm，与官方整机62.5mm不一致；未缩放掩盖。电池和屏幕排线尚未完整装配 |
| K162 ToughC5 | 固定 `M5_Hardware@18de3bb3087ea617c38642ba3eedd10dc9f17cbf` 的原厂 STL；仅保留原位0/2组件，排除远处重复打印件1/3 | 原厂壳体75.9302×58.0×41.9mm与官网75.9×58.0×42.5mm有厚度偏差；光学盖板/内部层估算 |
| K016-P-SE StickC-Plus SE | 独立 SE 资料证实48×24×13.5mm及透明橙壳，复用相同外形的 Plus 壳体 CAD，增加 SE 专属背贴 | 不含 IMU；内部 PCB 仍为 UNKNOWN，真实透明材料未校准 |
| C156 StackChan Core | 官方背照可确认 CoreS3 主机结构，仅保留 CoreS3 原厂上壳与光学支撑；去掉 DinBase、四脚和卡扣 | 后部真实主板、背贴、紧固件和扬声器未完整还原；不包含机器人身体，不继承普通 CoreS3 的固件身份 |
| A172 Stamp-AddOn C6 For P4 | 独立 ESP32-C6 SDIO 无线扩展模块。按自身照片/图纸做板形、通孔、屏蔽罩和连接器预览；板面与屏蔽罩各用对应的遮罩照片 | 板厚、元件高度和连接器隐藏深度估算；背面 UNKNOWN；未模拟 SDIO 或插接 |

每款知识 JSON 保留官网规格表、引脚表、独立来源回执和未知项。官方 Release History 提供五款发布日期，不以 CAD 最后提交日期冒充上市日。公开照片未套用 CAD 的 MIT 许可。

PaperDIY User Demo v1.0、ToughC5 User Demo v0.1、StickC-Plus SE Factory Test v0.2 以及三款的 UIFlow v2.5.5 已核对新版官方 M5Burner 的字符串项目/设备/公开版本身份。目录保留固定版本 URL，均为未验证 `trial`；发布方无 SHA-256 时不虚构摘要。出厂条目保持该产品第一项和默认项。C156 的官方说明要求刷 StackChan 用户固件；A172 为无线协处理模块，两者保留调查入口，不据同族名称套用出厂镜像。小智未找到本型号精确映射，已记录调查边界。

## 可复核命令

```sh
python tools/build-m5-missing-models.py --output /tmp/m5-missing-rebuilt
node tools/check-m5-missing-models.mjs
python tools/validate-platform-catalog.py
node tools/platform-knowledge-test.mjs
```

构建器默认离线，校验所有保留输入哈希。各 `assets/<id>/model-plan.json` 保留 CAD 组件、刚体变换、像素裁切和遮罩；`build-check.json` 明确原生网格与动态派生几何的测量范围。`docs/platform/evidence/m5-missing-products-20261126/loader-checks.json` 使用生产 Three.js 加载器、CPU 射线和材质预设检查，覆盖36条屏幕射线、可恢复分开查看、UNKNOWN 构造及纸贴/连接器/PCB不随外壳改色。Node 纹理和 Canvas 是替身，不能证明 WebGL 渲染、字样可读、照片叠图、输入行为或制造精度通过；这些项保持未测。
