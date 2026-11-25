# C5 原厂浏览器证据草稿（2026-11-25）

这是两次既有运行的证据整理，没有新开浏览器、重建或执行固件。仅适用于显式 `waveshare-lcd273-idf553-offline-v1` 离线驱动辅助模式；原始 FactoryOnly 与 ROM 未改。strict、原生 Wi-Fi、AP/关联/IP/RF、硅片精度及其它参考固件资格均不继承。

| 证据 | 固定 WASM | 结果 |
|---|---|---|
| `before-cache-fix.json` | 8,896,114 B / `9e0f3c286fe90894a717786867667d0bc19bd3484cd36d4887409cdc7fd2b0c4` | 菜单、五次按键、a0卡与F29卡正常重载保留有证据；F29 Photo在PC4201ed26遇到ModelLimit，未显示JPEG。 |
| `photo-fixed.json` | 8,902,103 B / `dee308ca758e02e5574d1888b3b68d7f7b7071da6d0f193347a4cf8628ceb28f` | F29的真实JPEG棋格、Back及Sensor页面入口有证据；Sensor数值仍为负例。 |

共用原厂镜像为3,346,992 B / `cf10a4358d03898458c16390f3f16ceb0306befe2260160de0919d755b920942`；从固定Waveshare catalog远程加载，HTTP200/CORS允许，无本地固件替换。ROM为472,300 B / `49693d745eb4b4e6e6c77be4f5213765d545d8c7a1d9742a8a619bf596c4e27a`，路径 `web/wasm/fw/esp32c5_rev100_rom.elf`。两次实际appDirect=false、custom stubs=[]，同时确实启用了驱动API辅助钩子，不能称pure-ROM/no-HLE。nativeWifi/associated/hasIp均false。

旧版初始、尚未启用profile的实际capabilities标签为 `interpreter-slots-including-wfi-idle`；该旧标签没有说明驱动辅助slots。启用后的driver snapshots另有virtualSlots。固定版在ready和Photo均实际捕获 `interpreter-slots-including-wfi-idle-and-driver-compat`。新标签没有倒填旧run，所有insns均不能当作物理CPU指令或性能测量。

源身份也分开：旧服务初始commit为6e4581a17f41eebb07cbcdec4f24023e97ce2886，末次HEAD为06a8bad6e92ff0a5c5c999566ae3995d094dfeb1；列出的实际服务JS/CSS/worker仍与初始源相同。固定版源为c8232bfcc3464bd62bdafbc93c18d096c169d6f5。每次WASM/ROM与服务文件的实测哈希都留在各自JSON；HEAD变化不算另一次guest运行。

两次输入都使用同电平重复请求读取真实guestTime：低电平目标≥0.12guest秒，释放目标≥0.2guest秒，实际首次UP−首次DOWN均<0.4秒。回执只证明排队；实际黄框变化、App入口/返回标记与frame内容才证明本场景消费。没有host LVGL读取泵、焦点、应用函数调用或合成画面。

旧版五次路径为NEXT→Sensor、NEXT→Photo、BOOT→无目录提示、BOOT→返回、PREV→Sensor。原卡a0e026d9…根目录离线分析为空，但 `No /sdcard/photo folder` 也可能来自其它opendir失败，不能证明文件消费。旧F29路径出现App2clicked后ModelLimit；773269652是模拟器报告的计数，没有浏览器first-fault/raw指令ABI。随后释放失败的NOT_RUNNING是二次错误，不能替换初始停止原因。

固定版两NEXT/BOOT显示真实320×320三色32px棋格及左/Back/右控件：PNG SHA为a16be56df3b9a43b1013d41ca6ad4110e892c72f1c8d3b00921d8c839b983a0f，RGBA为668d3017ab8ba7806c87b1f6e5e1ddb5e42446566a09c7a205bfe816f77e5f5a。独立浏览器RGBA反量化的204800 B RGB565_LE SHA为81c3850416e159a29fb669a0b58ac59490b5f81444de92f4dd284c8253ae9b03，与独立native帧一致；native图未充当browser输出。上方90个格中心的主色90/90匹配输入。棋格对转置对称，不能据此验证任意旋转或物理色准；颜色还经过RGB565/RGB666与亮度量化。没有可见文件名/标题或解码成功UART，不补写这些输出。

固定版真实NEXT选Back，再BOOT返回；首次返回帧是部分发布画面，随后PREV帧才完整显示四图标菜单与Sensor黄框。BOOT进入Sensor布局后，仅保持既有释放电平推进0.651153604guest秒，实际SHTC3为-45.00°C/0.00%RH，QMI含AccelZ0.04、Temp0.10°C；仅页面与timer更新有证据，数值正确性不合格。后续源诊断不属于本草稿。

存储分开记录：旧a0原卡与F29独立卡、固定版F29均在各自同origin/同既有ephemeral context普通reload至ready后导出相同全卡SHA；不再次导入、不start时guestUART为空、stats为空、hooks0。a0等同默认空卡，独特数据保留由不同于默认a0的F29补充。F29为64MiB /f29e47c7fca119caf71353f311cfb7bcbb4ccff2c26f6afcf9c80c9805dba80f，含5200B JPEG /6af0bed529e583dc696a07c3d01088667efff5c4bb0d99932e7d71f42c6b53b6。SD54/20/0和host mounted/restored不是独立的guest消费证明。不声称crash-safe、跨profile、跨独立context或固定版reload后重开Photo。

保留的观察负例包括：旧返回半屏、reset灰屏/半屏、包含Weather滚动字的过严ROI门槛超时、combined serial包含离线host notice、ModelLimit后NOT_RUNNING；固定版返回半屏、Sensor异常数值、CLI inspection文本格式和旧network命令不兼容。没有把观察器超时当runtime故障，也没有删去真实故障。

复现命令使用调用者提供的 `$BASE_URL`、`$SERVE_ROOT`、`$PORT`、`$BIND`、`$TASK_SESSION` 和匹配哈希的输入，见各JSON。用 `~/.codex/skills/playwright/scripts/playwright_cli.sh`，headless启动；只关闭任务自有session/server。旧2个、新1个owned contexts及其进程/监听均已关闭，实际标识未保留。

此目录只有摘要与哈希。BIN、PNG、UART、IMG、完整事件流和原报告均未复制，尚无这些私有原件的公开下载链接；上游原厂URL是唯一列出的公开固件来源。`curation.json` 映射两轮原MD/JSON/raw manifest与草稿文件哈希，保留168/109份原始artifact身份。它自身用排除selfIntegrity字段的规范JSON摘要避免循环；完整文件SHA在交付回执中另列。
