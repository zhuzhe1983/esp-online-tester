# PaperS3 官方 v0.5 TF 验收

固定官方 UserDemo v0.5（SHA-256 `d6733a0ca378f95335fa5fba4d4d992fb1dd97c17557b20e9aebfca08ba6d624`）在 `7f0498df` 生产 WASM 中经 ROM 启动。没有修改固件，未使用私人应用替代官方测试。复现命令：

```sh
node tools/probe-papers3-factory.mjs --url 'https://m5burner-cdn.m5stack.com/firmware/16287f6984f8ec7fe8058a6c3497b3b4.bin' --sha256 d6733a0ca378f95335fa5fba4d4d992fb1dd97c17557b20e9aebfca08ba6d624
```

[收据](factory-wasm.json) 保存产物和卡镜像摘要、实际输入时间、断言及串口白名单。[固定源码](sources.json) 的 `Hal::sdCardTest()` 在 FAT32 根目录写入 `TEST.TXT`、内容 `Hello, World!`。探针独立解析客体导出的 FAT32 镜像，核对文件名、13 字节内容和两份 FAT 的簇链。64 MiB 卡有 4 个变更页；复位和新 worker 重载后文件与整卡摘要一致。新 worker 启动前另行核对完整卡摘要，避免仅凭应用重新写入相同内容推断持久化。官方卡片区域实际显示 `Size: 0.1 GB`、`Type: SDHC/SDXC`、`Name: SIMSD`；容量是固件按一位小数格式化所得，不代表 100 MB 卡。

该运行同时复核黑白测试界面、GT911 两次触摸切换静音图标、BMI270 显式虚拟样本及新产生的扫描日志。没有新的 TF 实现改动；本次补齐既有 SPI SDHC 路径在官方出厂镜像上的结果证据。旧的私人固件、多簇写入和 IndexedDB/mini 页面证据见 [SPI TF 证据](../papers3-tf/README.md)。

范围是即时数字 SPI SDHC、官方 FAT32 读挂载及文件写入、内存存储的 reset/reload。未重测浏览器 IndexedDB，不验证 SD 供电/物理拔插/忙时序。RTC 仍未实现，官方界面此时显示无效时间；PMS150G 电源键、音频、灰阶光学和真实无线均未据此通过。原始 RGBA 画面只保存在本地临时目录，未入库；没有私人固件或文件数据。Node worker 在 `finally` 中释放，本次没有开启浏览器。

首轮完整通过保存在 [初始收据](initial-pass.json)。随后一次 150 ms 扫描短按未产生任务日志，见 [负结果](scan-short-tap-negative.json)；原因未定位，不能归结为已确认的刷新窗口。最终探针最多短按三次，每次输入及实际尝试次数均记录在 `scanInputs`，仍须观察新增任务日志。
