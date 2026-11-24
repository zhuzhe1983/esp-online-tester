# 最终发布前目录与来源审计

`publication-catalog-audit.json.gz` 是本次轻量审计的完整结果；解压字节未改动，`passed: true`。
它只证明当前目录、范围和导出过滤一致，不把尚未执行的第030批发布、
全链补丁重放或 Sites 下载回读写成已完成。

## 核对结果

| 核对项 | 实际结果 |
| --- | --- |
| 官方分类快照 | 2026-11-23 16:35:55 UTC；180 个 listing |
| 纳入主控或含主控套件 | 149 个系列；149 个唯一 fragment |
| 排除的分类 listing | 31 个非本范围商品 |
| 纳入系列内原始 SKU 行 | 384 个唯一 SKU |
| 附件包排除 | SKU33355，ESP32-XX-Basic-Kit-Acce，不含主控 |
| 最终主控 SKU / 精确配置 | 383 / 407 |
| 同 SKU 的额外配置 | 24 个 SKU 各有两套明确修订/控制器配置 |
| 最后第030批 | 4 系列、16 SKU、16 配置，与前029批无重叠 |
| 每系列 SKU 对账 | 全部相等；无缺项、无越界项 |
| fragment 产品对象与中央目录 | 407 / 407 逐对象完全相等 |
| 原029目录保护 | 660 产品、584 固件、416 watch、668 knowledge-index 对象全部保留 |
| 最终中央目录 | 676 产品、609 固件、441 watch、684 knowledge-index |
| 当前目录与最终507项验证 | 四文件 SHA 完全相同 |
| 导出器过滤与序列化 | 四文件逐字节相同；无被丢弃的固件、watch target 或 knowledge 条目 |

原029的保护检查同时使用保存的 canonical guards 和本工作区真实
`61447a576c36f40adcd40810fd9180f137937bdc` Git 对象。只从当前目录移除
第030批拥有的条目，并恢复 knowledge index 的原更新时间字段，得到的
完整 JSON 与原029对象相等；原391个 Waveshare 产品及269个其他产品均保留。

最终四目录 freeze 仍为：

```text
959710f0e2ae5666d534d20db52d4ceed89b283d854ed196d3f1b815abefb206
```

导出模拟从 `tools/export-waveshare-increment.py` 提取实际19条目录过滤
AST 语句和实际 JSON 序列化表达式，在内存中用391+16个允许的 profile 执行。
它没有调用 exporter 的 Git 写对象、建树、提交、写 state 或输出补丁步骤。
“源引用已纳入 development commit”的检查必须由正式 exporter 在提交后完成；
本审计验证的是当前工作区四目录字节与测试输入一致。

## 第030批与可引用的一手链接

| 系列 | SKU 数 | 官方商品页 | 官方说明 |
| --- | --- | --- | --- |
| ESP32-S3-Nano / Nano-M | 2 | [商品与选项](https://www.waveshare.com/product/esp32-related/esp32-s3-nano.htm) | [Nano 文档](https://docs.waveshare.com/ESP32-S3-Nano) |
| ESP32-S3-Zero / -M / -N8R8 / -N8R8-M | 4 | [商品与选项](https://www.waveshare.com/product/esp32-related/esp32-s3-zero.htm) | [Zero 文档](https://docs.waveshare.com/ESP32-S3-Zero) |
| ESP32-S3-DEV-KIT：N8R8、N16R8、N32R16V、N16R8-U 及对应排针款 | 8 | [商品与选项](https://www.waveshare.com/product/esp32-related/esp32-s3-dev-kit-n8r8.htm) | [DEV-KIT 文档](https://docs.waveshare.com/ESP32-S3-DEV-KIT-N8R8) |
| ESP32-S3-Pico / Pico-M | 2 | [商品与选项](https://www.waveshare.com/product/esp32-related/esp32-s3-pico.htm) | [Pico 文档](https://docs.waveshare.com/ESP32-S3-Pico) |

范围以[官方 ESP32 分类](https://www.waveshare.com/product/esp32-related.htm)
的保留快照为准；[Basic Kit 商品页](https://www.waveshare.com/product/esp32-related/esp32-xx-basic-kit.htm)
是33355附件包排除和三个含板套件纳入的来源。JSON 中还列出四系列的官方
Resources-And-Documents、原理图、Zero STEP 与 DEV-KIT N8R8 CAD ZIP 的精确 URL，
并指向实际使用的仓库证据路径。它们是保留的一手来源链接；本审计没有重新抓取网页，
也没有把新的网页内容静默加入既定范围。

## 最终说明应保留的界限

“完成”可以描述149系列的目录覆盖和383个主控 SKU 的精确配置覆盖。
407 是 profile 数，不是407个独立商品 SKU，也不是407套独立机械模型：
当前引用259个不同模型 URL，同板套件、容量或明确修订可共享已注明范围的几何。

407个 runtime 与 model 的状态均为 `partial`。与 Waveshare 产品关联的246条
固件中180条为 `trial`、66条为 `reference`，`simulationVerified` 和
`hardwareValidated` 全部为 false；285条关联 watch 的同类标记也为 false。
实际执行过的原始镜像或数字诊断只证明对应型号、对应镜像、对应路径，
不能扩展成所有原厂应用和外设均已实现。第030批新增排针、容量、天线款
不继承基础型号的原 BIN 执行验收。

当前模型验收有生产 loader、CPU 几何/射线和官方图对照证据；没有本次
浏览器 WebGL 或物理硬件验收。照片重建中的厚度、高度、背面结构、USB接触结构、
排针长度与插合公差保持估计或 UNKNOWN，不作为制造/实物装配数据。

发布日期没有被证实：全部407条 chronology.date 为 null。保留的官方
Position 只作为明确注明的优先参考，实际交付还按共同实现工作分组；
前7批均为初始带屏优先项，但第008批后已有无屏设备，后续批次仍继续交付屏幕设备。
因此不能声称严格按发布日期排序或全程先交付完全部带屏型号。初始99个
displayPriorityHint 是排期线索，也不等于99个最终确认的板载屏幕系列；例如
Thermal Camera 的最终知识明确没有板载显示屏。

## 保留元数据中的历史差异

- 初始 inventory 的 `boardVariantValidationComplete: false`、初始 census 的
  `skuExpansionComplete: false`，以及 Thermal Camera / DEV-KIT-NX 的早期404
  记录属于当时调查阶段。最终 fragments 已引用对应实际 wiki / N8 文档来源；
  新的覆盖审计与早期快照不互相替写。
- inventory 中旧 `documentationCacheFiles` 的绝对路径是历史工作缓存指针，
  不是当前保留来源文件，也不是本次审计所需输入。
- 第030批 fragment 的 integrationNotes 中“all Rust/Cargo sources remain unchanged”
  只适用于目录恢复操作。最终交付另外修正了两处 classic CLI 源码/测试，
  由 `classic-cli-correction.json`、最终855 freeze 与 WASM 同字节证明单独记录。
- 老 batch-04 文档正文中的四基础模型未变和36张审图记录是历史修订。该文档
  开头已明确当前15个重建模型应使用 `waveshare-batch030-models-20261124/`
  中绑定新 hash 的证据；只有已发布基础 Zero 模型保持原字节。
- `manufacturer-cad` 是加载类型。按真实 provenance 统计407个 profile：
  195个 official-cad、22个 official-cad-partial、3个
  official-cad-with-dimensioned-optical-reference、187个 dimension-photo-reconstruction。
  不能把类型字段解释为全部来自原生厂家 CAD。
- Nano 历史 LED 封装脚号与 ROHM 资料的冲突、Pico 的 S3R2/2MiB 与官方 Octal-PSRAM
  文字冲突仍有明示。Pico 的4MiB MicroPython软件布局不改变物理16MiB Flash。
- DEV-KIT 公布25.4×63.3mm，原生CAD边界为25.42068576×63.31069634mm。
  几何没有被缩放以掩盖差异；这不是实物公差验收。

## 复核方式

在包含原029 Git 对象的本发布工作区执行：

```sh
python3 docs/platform/evidence/waveshare-final-20261124/audit-publication-catalog.py
```

它使用同目录 `publication-audit-inputs.json` 保存的前029发布范围和030选择，
不依赖旧临时作者目录；没有构建或重复平台测试，实际本次运行小于1秒。
仅按 `git apply` 应用补丁而未保留原029 Git对象的用户，可使用已附的 canonical
guards 和全链重放工具核对；该脚本的原始 Git 对象对照需要相应对象在本地。
