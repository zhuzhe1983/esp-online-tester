# Stamp 设备选择归并

设备选择按主 PCB 区分。同一主板的排针、连接器装配或套装附件差异不占用顶层设备入口；产品记录仍按具体 SKU 保留。`pickerVisible: false` 只影响普通下拉框、全局设备搜索及计数。完整 catalog、API、知识资料、模型、固件与分享链接仍使用原 ID，不重定向到普通版。

## 已隐藏的 10 个条目

官方产品描述、套装清单与比较表核对日期：2026-11-04。下列“保留入口”是选择器归并关系，不是 ID 别名或固件兼容声明。

| 隐藏的产品 ID | 保留入口 | 官方证据与装配差异 |
| --- | --- | --- |
| `m5stack-s007-pin127` | `m5stack-s007` | [S3 PIN1.27](https://docs.m5stack.com/en/core/Stamp-S3_PIN1.27)：S3 的 1.27 mm 排针装配。 |
| `m5stack-s007-pin254` | `m5stack-s007` | [S3 PIN2.54](https://docs.m5stack.com/en/core/Stamp-S3_PIN2.54)：S3 的 2.54 mm 排针装配。 |
| `m5stack-s007-pin127-v033` | `m5stack-s007-v033` | [S3A PIN1.27](https://docs.m5stack.com/en/core/Stamp-S3A_PIN127)：S3A 的 1.27 mm 排针装配。 |
| `m5stack-s007-pin254-v033` | `m5stack-s007-v033` | [S3A PIN2.54](https://docs.m5stack.com/en/core/Stamp-S3A_PIN2.54)：S3A 的 2.54 mm 排针装配。 |
| `m5stack-s015-dip` | `m5stack-s015` | [S3Bat DIP](https://docs.m5stack.com/en/core/Stamp-S3Bat_DIP)：比较表仅列排针是否焊接，DIP 预焊 2.54 mm 排针。 |
| `m5stack-s016-dip` | `m5stack-s016` | [C5 DIP](https://docs.m5stack.com/en/core/Stamp-C5_DIP)：DIP 默认焊接后部 FPC 连接器，另随附两条 2.54-7P 排针；普通 C5 不焊 FPC。并非声称两款已装接口完全相同。 |
| `m5stack-k051-b` | `m5stack-c050-b` | [Pico DIY Kit](https://docs.m5stack.com/en/core/Stamp-Pico_DIY_Kit)：1 个 Stamp-Pico 加排针/排母、Grove 插座、贴纸、扳手与外置 ESP32 下载器。 |
| `m5stack-k051` | `m5stack-c050-b` | [Pico Mate](https://docs.m5stack.com/en/core/Stamp-Pico_Mate)：1 个 Stamp-Pico 加排针/排母、Grove 插座、贴纸和扳手，无 DIY Kit 的外置下载器。 |
| `m5stack-k056` | `m5stack-c056-b` | [C3 Mate](https://docs.m5stack.com/en/core/Stamp-C3_Mate)：1 个 Stamp-C3 加排针/排母、两个 Grove 插座、贴纸和扳手。 |
| `m5stack-k122` | `m5stack-c122-b` | [C3U Mate](https://docs.m5stack.com/en/core/Stamp-C3U_Mate)：1 个 Stamp-C3U 加排针/排母、两个 Grove 插座、贴纸和扳手。 |

[Pico](https://docs.m5stack.com/en/core/Stamp-Pico)、[C3](https://docs.m5stack.com/en/core/Stamp-C3) 与 [C3U](https://docs.m5stack.com/en/core/Stamp-C3U) 的普通 SKU 均为五件包装；现有模型展示其中一个模块，包装数量不会生成五个设备入口。套装中的外置下载器和散件也不会自动变成已连接的电路。

## 主板交叉核对

Pico 普通款、Mate 与 DIY Kit 的官方页面引用同一 [Pico 原理图](https://static-cdn.m5stack.com/resource/docs/products/core/stamp_pico/stamp_pico_sch_01.webp)，同为 PICO-D4、G27 RGB 与 G39 用户键。C3 普通款与 Mate 引用同一 [C3 原理图](https://static-cdn.m5stack.com/resource/docs/products/core/stamp_c3/stamp_c3_sch_01.webp)，包含 CH9102F 与 G3 用户键；C3U 普通款与 Mate 引用同一 [C3U 原理图](https://static-cdn.m5stack.com/resource/docs/products/core/stamp_c3u/stamp_c3u_sch_01.webp)，USB D−/D+ 直接连接 G18/G19，用户键为 G9。此次核对读图，不仅比较产品名称。

C3 与 C3U 因上述板级差异保持两个入口。S3 与 S3A、StampFly 原版与 v1.1 继续保持独立入口；本次不改变任何运行支持结论。

原理图下载记录（均为 `image/webp`，保留文件校验值，不另行镜像图片）：

| 原理图 | 实际抓取 UTC | 字节数 | SHA-256 |
| --- | --- | ---: | --- |
| Pico | 2026-11-04T03:21:32Z | 115084 | `81479376963d1b7ac69231f718bbe2287d47d6a373437cba92c073bfa11365c5` |
| C3 | 2026-11-04T03:21:33Z | 248312 | `873538e7cbc79bb65b630146e6d4ee81384269df9f2005cd0267017a96a9c982` |
| C3U | 2026-11-04T03:21:34Z | 272246 | `eafaa3fac84eb1b3d7575a74e0e222e3b4d255255e1460c27ed9f8baf8a11957` |

## 旧链接与 API

显式打开隐藏款的 scene 时，选择器临时加入该型号并保持选中，名称、模型和 `productId` 均不替换。切换同系列/全部系列不会把它切成普通版；主动选择其他型号后临时选项消失。隐藏条目不计入普通设备总数，也不重新出现在搜索中。API 的 `loadCatalog()` 返回全部记录，`createDefaultScene(catalog, id)`、scene 编解码与 `espTester.configure(scene)` 不采用 UI 过滤结果。


## 验证记录

[浏览器与静态检查记录](../evidence/stamp-picker-2026-11-04.json)保留完整命令与结果。Headless Chromium 实测全部 10 个旧 ID 的 API 配置、模型加载、分享还原和系列筛选；普通 Stamp 列表 11 款、全局搜索 206 款，完整目录 216 款。也验证主动切到普通款后临时选项消失，以及其他厂商的选择与搜索。此次不新增固件运行支持结论。

浏览器检查脚本为 `tools/fixtures/product-picker-browser-check.cjs`，通过仓库规定的 Playwright wrapper 执行，结束后关闭并核验任务浏览器会话。


## 衍生模型菜单

以上 10 个条目通过 `visualVariantOf` 显式关联表中的父产品。选择父产品后，3D 顶部“外壳”旁的“衍生模型”可查看其排针、连接器装配或套装外观。预览可旋转、改色、分开查看；渲染器使用 `previewOnly`，不显示运行 RGB、电机、飞行动态与外接配件，也不发送模型按键或触摸输入。设备说明、接口、固件和配件配置仍属于左侧选中的父产品。

菜单状态仅存在于当前页面，分享与 API 观察的 `scene.productId` 仍是父产品。切换设备、切到 2D、加载固件、启动或重启会退出预览；运行和加载时菜单禁用。暂停后恢复可用，但需主动再次选择衍生外观，不恢复先前预览。人类按钮和 API `start()` / `reset()` 都由同一运行状态事件恢复本体，再接入真实客体输出。

旧分享/API 如直接指定 `m5stack-s015-dip`，仍按这个精确 SKU 配置并显示自身模型；不会自动改成 `m5stack-s015`，该旧入口也不提供父产品的衍生菜单。新菜单只提供外观选择，不扩充或改变板级支持范围。
