# 虚拟 TF 卡设置

所有在产品目录 `interfaces` 中明确声明 TF/microSD 卡槽的设备，默认准备一张 64 MiB FAT32 空卡。已有的卡数据优先保留，不因切换设置页、更新软件或新增默认规则而清空。

「设备存储」使用可滚动的独立对话框，桌面和手机均可打开、关闭或按 Esc 退出。这里可以上传完整 IMG/BIN/IMA 磁盘镜像、导出当前镜像，或选择 FAT32/exFAT 与 64/128 MiB 后创建空卡。创建空卡会明确确认替换；上传镜像保留输入字节，不重新格式化。导入范围为 1–128 MiB、512 KiB 的整数倍，以限制浏览器和 WASM 内存占用。

卡槽、准备的镜像和固件读写是三个不同状态：

- `slot:true` 表示目录中有卡槽；选择设备后就可配置卡，不要求先下载固件。
- `present:true,mounted:false` 表示本地已准备卡。未实现该板型 SD 传输时，界面注明“尚未接入固件”，不发送无效的 worker 挂卡指令。
- 具备已实现 SD 传输的板型载入固件后，自动将卡挂入 guest，状态为 `supported:true,mounted:true`。固件的驱动、供电和文件系统支持仍决定能否读取。

FAT32 保持默认，尤其是只支持 FAT32 的官方固件。选择 exFAT 会生成真正的 exFAT 文件系统，但不代表当前固件已支持它。导入后的格式名称仅根据引导扇区签名识别，不能代替文件系统完整性检查。

## 数据保留和隔离

未载入固件时，设置保存为当前产品的“准备卡”；新固件首次建立存储作用域时可使用它。已有固件的卡镜像、写入页以及用户明确清空后的记录都优先保留。准备卡只为从未使用的固件作用域初始化一次，采用 epoch/revision 条件写入，其他页面同时写卡时拒绝覆盖。

载入固件后，卡数据继续按产品、板型、原始完整 Flash 内容摘要隔离。PaperMono-Lite 保留历史 `papermono` 存储别名。创建空卡与镜像导入仅替换当前卡；Flash 中的 NVS 等内容保留。未实现 SD 传输的板型可以保存、导出准备卡，不能声称已向固件提供存储。

卡镜像只保存在当前浏览器，不进入服务器、Git、场景 JSON 或分享链接。浏览器 IndexedDB 不可用时明确显示仅本页内存模式。客体写入按约 1.5 秒增量保存，关闭页面时尽力收尾，不保证断电或浏览器崩溃不丢最后一批数据。

## 格式实现和验证

`web/platform/sd-card-image.js` 使用确定性的空卡格式：512 字节扇区、1 MiB 起始分区。FAT32 保持原有默认字节与两份 FAT、备份引导扇区及 FSInfo；64/128 MiB 两档均满足 FAT32 的簇数要求。

exFAT 依据 [Microsoft exFAT 文件系统规范](https://learn.microsoft.com/en-us/windows/win32/fileio/exfat-specification) 的第 3–7 节：主/备引导区及校验和、单份 FAT、4 KiB 簇、分配位图、空根目录和卷标。Unicode 大写映射采用第 7.2.5.1 节推荐的完整压缩数据表，5836 字节，规范校验和 `E619D30D`；数据由该规范表 25 提取，归因保留在 `exfat-upcase.js`。它不是依赖当前浏览器 Unicode 版本临时生成的映射。

验证包括独立 macOS `fsck_exfat`/`fsck_msdos` 检查、任务自有镜像挂载后的中文文件名和不区分大小写的写读、浏览器设置导入/格式化/刷新恢复，以及已有卡优先、跨产品隔离、并发写入拒绝的测试。检查记录见 [收据](evidence/tf-card-settings/verification.json)。这些证明镜像格式和平台存储行为，不代表每款固件或实物 TF 卡已通过测试。

```sh
node --test tools/platform-sd-image-test.mjs tools/platform-sd-card-test.mjs tools/platform-device-storage-test.mjs
```
