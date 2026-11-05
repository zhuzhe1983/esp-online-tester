# 设备接口与配件筛选

选择设备时，配件区会切换到该设备已记录的外部接口。优先选中有已收录配件的接口，再按配件品牌筛选。同一设备内切换品牌、添加配件或编辑场景时，保留有效的当前选择；切换设备后重新选择。

接口清单来自产品的 `interfaces`，只采用 `external: true` 且记录了实体类型的条目。内部总线、仅有 GPIO/I²C 等信号映射的条目，以及形态未确认的泛称 USB 不会列入。没有已核实接口时显示“外部接口资料待核对”，不会默认成 Grove；“所有接口（仅浏览）”与全局配件搜索仍可浏览完整目录及本地 DIY 配件。

此筛选只比较物理连接器类型，不保证引脚、电压或协议兼容，也不增加模拟器支持。加入后的接线设置和已有运行支持检查保持独立。

2026-11-05 核对了全部 217 个产品条目，为 41 个产品补入 69 项记录，其中 67 项为追加、2 项为保留引脚的类型修正。TriviaPOD 另行作为独立产品补入 USB-C 充电口。目前 121 个 LILYGO 条目只有商品身份资料，外部接口仍待核实；其他设备已记录的接口也不代表穷尽所有插口。

- StopWatch：Grove、USB-C、后部 2.54 mm 排针；未推断后部各硬件版本的引脚映射。
- TriviaPOD：仅确认 USB-C 充电，不继承 StopWatch 的 Grove、后部总线或固件支持。
- Stamp-C5：裸 FPC 焊盘；C5 DIP：已安装的 FPC 插座，两者分别记录。
- Stamp-Pico / Mate / DIY Kit：主板按焊盘记录；套件散装 Grove 插座、排针和独立下载器不算主板已安装接口。
- Station-485：六个 Grove 分别记录，A1/A2 共用信号；USB-A 为电源输出；内部预留口不列为外部接口。
- Metalio：`microSD` 为实体类型，`SDMMC` 保留为协议，原 GPIO 映射不变。
- Grove 与 Qwiic/JST-SH、FPC 焊盘与插座、不同排针及 BTB 不因相似名称或间距而自动兼容。

可核对[全部产品接口清单](evidence/device-interfaces-20261105/inventory.json)、[逐项变更与事实引用](evidence/device-interfaces-20261105/applied-changes.json)、[本次官方来源抓取收据](evidence/device-interfaces-20261105/fresh-source-receipts.json)和[类型归一规则](evidence/device-interfaces-20261105/normalization-rules.json)。原始网页未提交，保留公开 URL、抓取时间、字节数与 SHA-256；事实和既有来源位于各产品知识文件。
