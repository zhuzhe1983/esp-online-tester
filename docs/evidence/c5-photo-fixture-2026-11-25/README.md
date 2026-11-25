# Photo 测试卡：生成与独立复核

本包记录确定性 JPEG 与 FAT32 测试卡的生成及独立只读回验。**不声明原厂 guest 已读取文件、完成解码或显示图片。** 原卡、固件、SDK和生产代码未改；本次整理只读核验既有物料，没有重生成64MiB IMG。运行与 kernel/MMIO 结果属于另一份证据。

## 输入与产物

生成器要求调用者提供原64MiB prepared FAT32 IMG：67108864B，SHA256 `a0e026d9aeafab624884455d25147647980bd6a83011618da04140659c28bfc6`。该输入及二进制产物没有随本文提供或上传；没有虚构下载入口。原输入生成后、只读复核后 hash 均不变。

| 文件 | 字节 | SHA256 |
| --- | ---: | --- |
| lcd273-photo-fixture.img | 67108864 | `f29e47c7fca119caf71353f311cfb7bcbb4ccff2c26f6afcf9c80c9805dba80f` |
| test.jpg | 5200 | `6af0bed529e583dc696a07c3d01088667efff5c4bb0d99932e7d71f42c6b53b6` |
| test-decoded.rgba | 409600 | `5208e0dd41bec6703a81d000fe3b9860fc468ce13a72965799dafe17ac8b2929` |
| test-decoded.png | 891 | `efb3e33351348de1f441dd0c83f382bdd8c996f7a44c66e9c040ca328321b406` |

图像为320×320、32px三色棋格，palette=(224,48,48)/(48,208,80)/(48,96,224)，规则 `palette[((x//32)+(y//32))%3]`。记录环境为 Python3.9.6、Pillow11.3.0、JPEG API6.2/libjpeg-turbo3.1.1；quality90、baseline8-bit、4:4:4、nonprogressive、optimize=false，无EXIF/ICC/comment。两次编码字节一致。RGBA来自5200B JPEG的实际解码，row-major RGBA8888、stride1280、alpha255；没有用源RGB代替解码结果。不同codec版本不保证产生相同字节。

## FAT32 与静态 Photo 合同

分区起LBA2048、129024sectors；512B sector/cluster，reserved32，两份FAT各993sectors，root cluster2，data起LBA4066。原root仅有volume label，无文件/子目录。

新增 `/photo/test.jpg`：root slot1的PHOTO目录在cluster3，NTcase0x08；其内`.`指3、`..`指0，slot2的`TEST    JPG`为archive attr0x20、NTcase0x18，文件簇链 **4→5→…→14→EOC**。无LFN。两份FAT一致；free127005→126993、next_free15，主/备FSInfo同步更新。固定FAT时间2026-11-25 00:00:00。只17sectors/4657B改变：2049、2055、2080、3073、4066..4078；MBR、boot、原volume entry及其他字节保留。

固定厂家源码 [app_photo.c](https://github.com/waveshareteam/ESP32-C5-LCD-2.73/blob/418de818f4f40c5a04c4292dbe293bfaa1351068/example/ESP-IDF-V553/01_factory/components/app_photo/app_photo.c) 扫描 `/sdcard/photo` 的DT_REG，以strcasecmp接受.jpg/.jpeg；NTcase保留小写短名，筛选本身也不区分大小写。[photo_ui.c](https://github.com/waveshareteam/ESP32-C5-LCD-2.73/blob/418de818f4f40c5a04c4292dbe293bfaa1351068/example/ESP-IDF-V553/01_factory/components/app_photo/ui/photo_ui.c) 通过POSIX fopen读JPEG，解为RGB565_LE，再给LVGL内存descriptor，没有LVGL盘符路径。320×320输入scale=256、居中、不放大。两份原文已核Git blob/size/SHA，详见 [result.json](result.json)。这些是静态合同，不是执行证明。

## 复现与回验

两脚本均由调用者传入路径，不依赖私有目录。生成器原子创建fresh输出目录，已存在即拒绝，不复用/清理目录；该负例已实际执行，原卡和测试卡hash不变。可用记录版本的Python/Pillow执行：

```sh
"$PYTHON" -B generate-fixture.py \
  --source "$SOURCE_IMG" --output-directory "$FRESH_OUTPUT_DIR"
"$PYTHON" -B verify-fixture.py \
  --source "$SOURCE_IMG" --card "$CARD_IMG" \
  --jpeg "$JPEG" --rgba "$RGBA" --report "$FRESH_REPORT_JSON"
```

独立 verifier 不调用生成器函数；重新解析MBR/BPB、FAT mirrors、目录/簇链，逐变更字节核对全IMG允许范围，从卡取出JPEG并实际解码，与RGBA逐字比较。本目录中的 [readonly-verification.json](readonly-verification.json) 为对既有输入的再次只读回验，全部通过，没有重新生成IMG。

原脚本本已参数化且不含个人路径，审查后按原字节复制；原报告则仅提炼必要事实。原报告/脚本、公开README/result/脚本的hash映射见 [provenance.json](provenance.json)。原ignored物料没有覆盖。这里不收录私人绝对路径、进程/会话标识、原traceback或协作过程叙事。

Host RGBA是资产参考。目标JPEG实现、RGB565量化、ILI转换及标题/按钮覆盖可能改变像素，不能要求guest整帧SHA等于此RGBA，也不能用host预览替代实际文件消费或出图验收。
