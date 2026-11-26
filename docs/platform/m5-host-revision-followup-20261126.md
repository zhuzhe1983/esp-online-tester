# M5Stack 主控版次与首轮角色复核

六个原待核套装中，一项排针形态已收敛，一项确证为不同的旧主控配置，其余公开资料仍未锁定供货版次。此处把厂商直接陈述、照片/图纸对应推断和未知分别保留。

| SKU | 深查结果 | 可用父映射与阻断 |
| --- | --- | --- |
| A145-V033 | 精确套装高清图展示预焊密排针，17+11、1.27mm形态；同S3A 8MB/无PSRAM配置 | 对应S007-PIN127-V033为照片/官方图纸推断，Includes未写完整采购SKU。套装罩为黑色，单卖PIN127多为橙色；不继承壳颜色 |
| K023、K023-B | 官方各自文档与Includes均仅指定Basic，所链接原理图分别是PM/温湿度附件 | 没有供货主控芯片/Flash/显示版次契约；不能仅凭年份、封闭外壳或例程默认值选择4MB/16MB或K001版本 |
| K119 | 官方明确Basic + MLX90640 + SIM7080G基座，公开例程用通用M5Stack初始化 | 主板内存/屏幕版次未锁定；不把SIM模块规格或软件构建目标当作实际主控容量 |
| K017 | 精确旧商品与固定官方旧CarMain/Remote源码证实机器人/手柄两个M5Core ESP-NOW角色 | 存储和完整Core采购SKU未声明；不能用现代K017-C文档替换旧SKU身份 |
| K014-B | 精确商城表明确16MB Flash + 4MB PSRAM + ILI9341，IMU有MPU6886/SH200Q历史差异 | 不对应当前目录任一ILI9342C FIRE配置。单独保留旧配置，不能继承8MB PSRAM或现代FIRE运行结论 |

A145-V033的套装尺寸图仅标4.7mm模块本体，不包含完整针脚包络；这不否定预焊排针。精确套装[斜视原图](evidence/m5-host-revisions-20261126/A145-V033-gallery-2.webp)与[侧视原图](evidence/m5-host-revisions-20261126/A145-V033-gallery-4.webp)按实际下载SHA保留，照片版权归M5Stack，不随项目MIT许可重新授权。S3A裸模块、PIN1.27单卖品及套装图片均独立核对；没有把相同模块名当作相同外观。

K014-B商城是精确SKU证据；当前 [Bala文档](https://docs.m5stack.com/en/app/bala) 页面标SKU **K014**，只能作为家族佐证。当前目录K007、K007-V26、K007-V27都声明ILI9342C。K007/K007-V26的8MB或K007-V27的4MB模拟映射不能改写旧实物4MB的事实，也不能证明旧ILI9341已经被验证。

补查还发现首轮Base-MCU行掩盖的7个含主机组合。K137含AtomS3，K033-C含StickC Plus2，K038含StickC，K038-B含StickC Plus；配置均已有具名父主控。K014-C为Gray16MB/ILI9342C，未声明PSRAM；没有证据把它说成旧FIRE16/4/ILI9341。K014-E为FIRE16MB/8MB/ILI9342C，采购版次仍不锁定。K017-C当前文档记2019.7从K017更换LiDAR及增加钣金，不能据“未提主控改变”就证明两个版本主板相同。

固定官方源码检查：

- `m5stack/M5Stack@e4e05109297c193559562b62ee4b5f276bc19b01` 的PM25/PM25_SHT30/BALA例程。
- `m5stack/M5-ProductExampleCodes@1bc38736e61a362080d1dffc4020b3a2a1dff6ae` 的旧PM2.5例程。
- `m5stack/IoT_BASE_SIM7080@ce787e5f61929aaada2ce7ee8bb301ab08de7d68` 的README/MQTT例程。
- `m5stack/Applications-LidarBot@6d62ec165a90aa163f5ba87865a2d01e900ab6a9` 的README、旧CarMain/Remote及X2 Master/Remote源码。
- `m5stack/M5Bala@67e66e484fafc6ff26e4637ee817dfbb79c53f2b` 的README。

这些源码支持通用M5Stack接口或两端ESP-NOW角色，不锁定包内Flash/PSRAM/LCD采购版次。未下载EXE/BIN，也未执行或提取固件；没有将例程构建板型当成物理PCB证明。

全量324条复核还纠正U109-B整机被Enclosure标题误作纯壳，以及有源通信单元被Antenna标题误作机械件。K143是AX630C主平台并包含ESP32-C6网卡，明确记录协处理器而不混成ESP32主平台。其余原首轮条目的主描述、规格和Includes均已重新筛查；没有发现额外独立ESP32主控缺口。

完整[324条复核与事实](research/m5-prior-store-audit-20261126.json)、[实际源收据](research/m5-host-revision-sources-20261126.json)及[44个SKU映射](research/m5-host-kit-mapping-20261126.json)供复查。5个仍未锁定主控版次的SKU为K023、K023-B、K119、K017、K017-C；A072的host包含关系另有来源冲突。这些阻断保留，未声称厂商在所有渠道都从未锁定过版次，也未声称完成PCB/BOM或实机审计。
