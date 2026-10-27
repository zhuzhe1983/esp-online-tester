# 出厂固件来源证据

本目录服务于 `docs/platform/research/factory-firmware-audit-2026-10-27.json` 的 83 产品审计。

- `metadata-source-receipt.json`：官方 M5Burner 原始元信息响应的 URL、UTC 抓取时间、长度和 SHA-256。原响应含账号字段，未提交；复核工具只在内存比较发布者身份。
- `product-document-sources.json`：83 个官方型号页的来源回执与固件相关链接。页面原文保留在忽略的工作输出中，不重复提交。
- `image-source-index.json` 与各 firmwareId JSON：30 份完整镜像的实际 hash、布局、芯片、Flash 容量及映射依据。响应体只在内存读取，未执行，也未将 BIN 提交或放入站点。
- `crosspoint-app-source-receipt.json`：上游 1.6.5/2.0.0rc 的浏览器读取/hash 结果及发布元信息回执，明确为 app-only。
- `strict-check-negative.json`：StickT2 v1.0 的 bootloader 头部 SPI mode 与内嵌摘要不一致；已定位 ID、原始完整文件 hash 与一字节反事实校验。`targeted-digest-checks.json` 记录随后有界读取及检查覆盖，不扩展为全部镜像内部摘要通过。

UTC 抓取时间与厂商发布日期分别记录。`image-pinned` 只描述来源覆盖；`trial` 不承诺板级兼容性。新的检查脚本 `tools/audit-factory-firmware-sources.py` 只复核固定来源、段 checksum 与附加摘要，不执行固件；需显式传入证据目录与公开元信息 URL。

隐私复核：未提交作者账号、访问令牌、终端标识、用户路径或浏览器会话。原始响应 hash 保留，用于复核精简记录。
