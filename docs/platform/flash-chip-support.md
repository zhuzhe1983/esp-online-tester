# 网页烧录的芯片支持范围

当前网页实机烧录流程开放 ESP32、ESP32-S3、ESP32-C3、ESP32-C6。P4 和 S31 在产品策略、镜像准备与安全状态检查三层均未开放；这与浏览器模拟器是否有对应板级模型是两件事。选择只有资料链接的固件时，还会先提示缺少可写入文件。

本地固定的 esptool-js 0.7.0 已包含 P4、S31 target 和 stub，见[官方 S31 target](https://github.com/espressif/esptool-js/blob/v0.7.0/src/targets/esp32s31.ts)及[stub 选择](https://github.com/espressif/esptool-js/blob/v0.7.0/src/stubFlasher.ts)。因此当前限制来自网站尚未完成接入，不能归因于芯片或底层库完全不能烧录。

基本流程仍是进入 ROM 下载模式、上传匹配的 stub、写入 Flash 并校验。P4、S31 的 bootloader 位于 `0x2000`；保留前方填充的完整 merged 镜像仍可从 `0` 写入。P4 旧版与 revision ≥ 3 使用不同 stub。依据：[esptool 5.6.0 P4](https://github.com/espressif/esptool/blob/v5.6.0/esptool/targets/esp32p4.py)、[S31](https://github.com/espressif/esptool/blob/v5.6.0/esptool/targets/esp32s31.py)。

开放网页入口前需要补齐并验证：

- S31 镜像 chip ID 为 32，不能落入 S3 的默认值 9；merged、build/segments、manifest 各路径都要统一芯片识别和 `0x2000` 启动地址。
- `flash-security.js` 接入相应芯片的安全状态查询，拒绝安全启动、Flash 加密或受保护的下载状态；未知状态仍拒绝写入。
- 用实际连接芯片的 revision 检查镜像范围；虚拟板型的 revision 限制不能代替实机检查。
- P4+C6 双芯片产品需要明确当前刷写的 MCU，不能把复合产品名称直接当作烧录芯片；Flash 容量也须有依据。

2026-11-05 检查只确认了本地库中的 P4 rev100/rev300、S31 stub 可以加载，并核对网站代码路径。没有选择串口或实机刷写，也没有扩大当前网页的芯片支持白名单。
