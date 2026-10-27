# M5Stack 出厂固件覆盖审计（2026-10-27）

检查当前 **83** 个产品 SKU/硬件版本。固定官方出厂镜像覆盖由 **6** 个增加到 **41** 个；另 **12** 个有出厂源码或恢复入口，**30** 个本轮未定位可信出厂镜像。新增 **30** 份镜像来源/布局回执，对应 **35** 个型号；当前目录共 **106** 个固件条目。

本轮未执行任何固件，也未刷写硬件。`trial` 表示允许有板级模型的设备尝试；找到镜像不等于模拟器已实现或兼容性已验证。所有 BIN 只在核对时进入内存，未写入服务器目录或 Git；浏览器本地缓存仍允许。

## 调查与判定

- 抓取全部 83 个官方产品页，保留 URL、UTC 时间、响应长度与 SHA-256。官方 M5Burner 元信息快照包含 2651 条；保留整响应 hash 和选中条目的公开字段，不保留作者账号。
- 官方发布者身份按已独立核对的 StopWatch/Cardputer 条目比对，出厂身份另外核对官方产品说明或明确的 Factory/Default 名称。仅“由 M5Stack 发布”不足以把 UIFlow、小智或示例归为出厂。
- 30 份完整源均核对实际 SHA-256、ESP 镜像段、bootloader/app 芯片、分区范围及设备 Flash 容量。响应提供 `Access-Control-Allow-Origin: *`；本轮没有逐一进行浏览器或固件启动验收。
- 精确版本共用依据保留在对应官方产品页或元信息中。Core2 v1.3、AWS v1.3、Atom-Matrix v1.1 等缺构建适配证据的型号没有套用旧版镜像。
- 出厂条目置于每设备固件第一项，并设置 `defaultFirmwareId`；仅有恢复入口时显示 reference，明确没有启动源。Stamp-S3 的 LED 示例原被误标为出厂，已纠正为替代示例并取消虚假出厂默认。

## 完整逐型号结果

| 产品 / SKU | 调查状态 | 默认出厂条目 | 结论 |
|---|---|---|---|
| M5Stack Cardputer-Adv · K132-Adv / K132-Adv | image-pinned | m5stack-k132-adv-official | 固定官方出厂远程镜像；执行/板级适配状态见固件目录，来源与布局校验不等同运行验证。 |
| M5Stack PaperMono / C153 | image-pinned | m5stack-papermono-official | 固定官方出厂远程镜像；执行/板级适配状态见固件目录，来源与布局校验不等同运行验证。 |
| M5Stack PaperMono-Lite / C153-Lite | image-pinned | m5stack-papermono-official | 固定官方出厂远程镜像；执行/板级适配状态见固件目录，来源与布局校验不等同运行验证。 |
| M5Stack PaperDIY Complete Kit · K159-B / K159-B | reference-only | m5stack-k159-b-factory-reference | 官方 PaperDIY 源码和新 WebBurner 分享入口存在；本轮旧版 M5Burner API 中未定位其完整 BIN。 |
| M5Stack Tab5 · C145 / C145/K145 | image-pinned | m5stack-c145-official-reference | 固定官方出厂远程镜像；执行/板级适配状态见固件目录，来源与布局校验不等同运行验证。 |
| M5Stack Station-Bat / K124-B | reference-only | m5stack-k124-b-factory-reference | 官方提供 M5Staion.exe 恢复入口；API 只有官方 UIFlow_Station，另有非同一官方发布者的 M5Station-Factory，未将其冒充官方完整镜像。 |
| M5Stack Station-485 · K123 / K123 | reference-only | m5stack-k123-factory-reference | 官方提供 M5Staion.exe 恢复入口；API 只有官方 UIFlow_Station，另有非同一官方发布者的 M5Station-Factory，未将其冒充官方完整镜像。 |
| Arduino Nesso N1 · DK001 / DK001 | not-located | — | M5Burner 有社区上传的 Arduino Nesso N1 Factory code，但发布者不匹配已核对官方发布者；未找到 Arduino/M5Stack 官方页对此构建的确认。 |
| M5Stack NanoH2 / C149 | not-located | — | 官方 NanoH2 产品页未定位出厂完整镜像；未拿 NanoC6 镜像替代。 |
| M5Stack NanoC6 · C125 / C125 | image-pinned | m5stack-c125-official-factory | 固定官方出厂远程镜像；执行/板级适配状态见固件目录，来源与布局校验不等同运行验证。 |
| M5Stack Stamp-S3 PIN2.54 · S007-PIN254 / S007-PIN254 | not-located | — | 已查该 SKU 的官方资料；官方目录中 Arduino/LED 示例或 UIFlow 开发固件没有出厂身份依据。未定位此封装/排针版本的可信完整出厂镜像。 |
| M5Stack Stamp-S3 PIN1.27 / S007-PIN127 | not-located | — | 已查该 SKU 的官方资料；官方目录中 Arduino/LED 示例或 UIFlow 开发固件没有出厂身份依据。未定位此封装/排针版本的可信完整出厂镜像。 |
| M5Stack Xiaozhi Card Kit · K146 / K146 | reference-only | m5stack-k146-official-reference | 已找到 m5stack/xiaozhi-card 与专用恢复教程；本轮官方 M5Burner 集合未定位此 Kit 的完整分发，不套用 CoreS3/AtomS3R 的小智镜像。 |
| M5Stack StamPLC · K141 / K141 | image-pinned | m5stack-k141-official-reference | 固定官方出厂远程镜像；执行/板级适配状态见固件目录，来源与布局校验不等同运行验证。 |
| M5Stack Paper v1.1 / K049-B | image-pinned | m5stack-k049-b-factory-reference | 固定官方出厂远程镜像；执行/板级适配状态见固件目录，来源与布局校验不等同运行验证。 |
| M5Stack CoreInk / K048 | image-pinned | m5stack-k048-factory-reference | 固定官方出厂远程镜像；执行/板级适配状态见固件目录，来源与布局校验不等同运行验证。 |
| M5Stack StopWatch / C152 | image-pinned | m5stack-stopwatch-official | 固定官方出厂远程镜像；执行/板级适配状态见固件目录，来源与布局校验不等同运行验证。 |
| M5Stack PaperColor · C151 / C151 | image-pinned | m5stack-c151-official-reference | 固定官方出厂远程镜像；执行/板级适配状态见固件目录，来源与布局校验不等同运行验证。 |
| M5Stack PaperS3 / C139 | image-pinned | m5stack-c139-factory-reference | 固定官方出厂远程镜像；执行/板级适配状态见固件目录，来源与布局校验不等同运行验证。 |
| M5Stack Stamp-S3 · S007 / S007 | not-located | — | 官方 LED 示例原被误标为出厂固件，本轮纠正为替代示例；未找到出厂镜像。 |
| M5Stack Stamp-S3A · S007-V033 / S007-V033 | not-located | — | 已查该 SKU 的官方资料；官方目录中 Arduino/LED 示例或 UIFlow 开发固件没有出厂身份依据。未定位此封装/排针版本的可信完整出厂镜像。 |
| M5Stack Stamp-S3A PIN2.54 / S007-PIN254-V033 | not-located | — | 已查该 SKU 的官方资料；官方目录中 Arduino/LED 示例或 UIFlow 开发固件没有出厂身份依据。未定位此封装/排针版本的可信完整出厂镜像。 |
| M5Stack Stamp-C3U Mate / K122 | not-located | — | 已查该 SKU 的官方资料；官方目录中 Arduino/LED 示例或 UIFlow 开发固件没有出厂身份依据。未定位此封装/排针版本的可信完整出厂镜像。 |
| M5Stack Stamp-C3 Mate · K056 / K056 | not-located | — | 已查该 SKU 的官方资料；官方目录中 Arduino/LED 示例或 UIFlow 开发固件没有出厂身份依据。未定位此封装/排针版本的可信完整出厂镜像。 |
| M5Stack Stamp-Pico Mate / K051 | not-located | — | 已查该 SKU 的官方资料；官方目录中 Arduino/LED 示例或 UIFlow 开发固件没有出厂身份依据。未定位此封装/排针版本的可信完整出厂镜像。 |
| M5Stack Stamp-Pico DIY Kit · K051-B / K051-B | not-located | — | 已查该 SKU 的官方资料；官方目录中 Arduino/LED 示例或 UIFlow 开发固件没有出厂身份依据。未定位此封装/排针版本的可信完整出厂镜像。 |
| M5Stack Stamp-S3Bat · S015 / S015 | not-located | — | 已查该 SKU 的官方资料；官方目录中 Arduino/LED 示例或 UIFlow 开发固件没有出厂身份依据。未定位此封装/排针版本的可信完整出厂镜像。 |
| M5Stack Stamp-S3Bat DIP / S015-DIP | not-located | — | 已查该 SKU 的官方资料；官方目录中 Arduino/LED 示例或 UIFlow 开发固件没有出厂身份依据。未定位此封装/排针版本的可信完整出厂镜像。 |
| M5Stack DinMeter / K134 | image-pinned | m5stack-k134-factory-reference | 固定官方出厂远程镜像；执行/板级适配状态见固件目录，来源与布局校验不等同运行验证。 |
| M5Stack DinMeter v1.1 · K134-V11 / K134-V11 | image-pinned | m5stack-k134-v11-official-reference | 固定官方出厂远程镜像；执行/板级适配状态见固件目录，来源与布局校验不等同运行验证。 |
| M5Stack Air Quality / K131 | image-pinned | m5stack-k131-factory-reference | 固定官方出厂远程镜像；执行/板级适配状态见固件目录，来源与布局校验不等同运行验证。 |
| M5Stack Air Quality v1.1 · K131-V11 / K131-V11 | image-pinned | m5stack-k131-v11-official-reference | 固定官方出厂远程镜像；执行/板级适配状态见固件目录，来源与布局校验不等同运行验证。 |
| M5Stack Dial · K130 / K130 | reference-only | m5stack-k130-official-reference | 官方 M5Dial-UserDemo 与 Factory Demo.exe 存在；本轮未定位同一官方发布者的完整 Dial BIN。 |
| M5Stack Dial v1.1 / K130-V11 | reference-only | m5stack-k130-v11-factory-reference | v1.1 官方页指向 M5Dial-UserDemo 与 Factory Demo.exe；本轮未定位可固定的完整 BIN。 |
| M5Stack Capsule / K129 | not-located | — | 官方 Capsule 资料和通用编程入口已查，未定位明确 Factory/Default 完整镜像；社区 Capsule 应用不等同出厂程序。 |
| M5Stack Capsule v1.1 · K129-V11 / K129-V11 | not-located | — | 官方 Capsule v1.1 资料已查，未定位此硬件版本出厂镜像；不凭相同芯片继承旧版应用。 |
| M5Stack Cardputer Mesh Kit / K152 | image-pinned | m5stack-k152-official-factory | 固定官方出厂远程镜像；执行/板级适配状态见固件目录，来源与布局校验不等同运行验证。 |
| M5Stack AtomS3R AI Chatbot Kit · K147 / K147 | not-located | — | 有官方 AtomS3R Xiaozhi 分发，但本轮未完成 K147 套件出厂/音频底座与唤醒词变体的对应核对。 |
| M5Stack AtomS3R-CAM AI Chatbot · K147-CAM / K147-CAM | not-located | — | 有官方 AtomS3R-CAM/M12 Xiaozhi 分发，但本轮未完成 K147-CAM 套件出厂配置对应核对。 |
| M5Stack Cardputer / K132 | image-pinned | m5stack-cardputer-official | 固定官方出厂远程镜像；执行/板级适配状态见固件目录，来源与布局校验不等同运行验证。 |
| M5Stack Cardputer v1.1 / K132-V11 | image-pinned | m5stack-cardputer-official | 固定官方出厂远程镜像；执行/板级适配状态见固件目录，来源与布局校验不等同运行验证。 |
| M5Stack AtomS3R-M12 Volcengine Kit · D062-M12 / D062-M12 | not-located | — | 找到官方 AtomS3R-M12 火山引擎语音助手 v1.0-beta，元信息明确仅供测试；未取得其为 D062-M12 出厂构建的依据。 |
| M5Stack AtomS3R-M12 / C126-M12 | image-pinned | m5stack-c126-m12-official-factory | 固定官方出厂远程镜像；执行/板级适配状态见固件目录，来源与布局校验不等同运行验证。 |
| M5Stack AtomS3R-Ext · C126-Ext / C126-Ext | not-located | — | 未定位 AtomS3R-Ext 独立出厂镜像；不沿用带屏 AtomS3R User Demo。 |
| M5Stack AtomS3U · K125 / K125 | not-located | — | 未定位 AtomS3U 独立出厂镜像；不沿用带屏 AtomS3 User Demo。 |
| M5Stack AtomS3R / C126 | image-pinned | m5stack-c126-factory-reference | 固定官方出厂远程镜像；执行/板级适配状态见固件目录，来源与布局校验不等同运行验证。 |
| M5Stack Atom VoiceS3R / C126-ECHO | not-located | — | 官方有 Atom EchoS3R EasyLoader，但不能仅凭 Xiaozhi 分发名判定当前 Atom VoiceS3R 出厂镜像。 |
| M5Stack AtomS3R-CAM · C126-CAM / C126-CAM | image-pinned | m5stack-c126-cam-official-factory | 固定官方出厂远程镜像；执行/板级适配状态见固件目录，来源与布局校验不等同运行验证。 |
| M5Stack AtomS3-Lite / C124 | reference-only | m5stack-c124-factory-reference | 官方仅定位 AtomS3-Lite-Factory-Firmware.exe 恢复入口；不能以带屏 AtomS3 UserDemo 替代。 |
| M5Stack AtomS3 · C123 / C123 | image-pinned | m5stack-c123-official-reference | 固定官方出厂远程镜像；执行/板级适配状态见固件目录，来源与布局校验不等同运行验证。 |
| M5Stack Atom-Lite · C008 / C008 | image-pinned | m5stack-c008-official-factory | 固定官方出厂远程镜像；执行/板级适配状态见固件目录，来源与布局校验不等同运行验证。 |
| M5Stack Atom Voice / C008-C | image-pinned | m5stack-c008-c-factory-reference | 固定官方出厂远程镜像；执行/板级适配状态见固件目录，来源与布局校验不等同运行验证。 |
| M5Stack Atom-Matrix / C008-B | image-pinned | m5stack-c008-b-official-factory | 固定官方出厂远程镜像；执行/板级适配状态见固件目录，来源与布局校验不等同运行验证。 |
| M5Stack Atom-Matrix v1.1 · C008-B-V11 / C008-B-V11 | not-located | — | 有 Atom-Matrix 旧版 Factory Test，但缺 v1.1 对该构建的明确适配证据。 |
| M5Stack Tough / K034 | not-located | — | 找到 Tough 官方资料与 UIFlow 生态入口，未定位可信完整 Factory 镜像；不沿用 Core2 镜像。 |
| M5Stack StickS3 · K150 / K150 | not-located | — | 官方 API 有 StickS3 小智、ESP-Claw、UIFlow2 和 PMIC 升级程序；未明确哪项为当前出厂构建，不把推荐应用或 PMIC 更新当作出厂镜像。 |
| M5Stack StickT2 / K016-T2 | image-pinned | m5stack-k016-t2-official-factory | 固定官方出厂远程镜像；bootloader 附加摘要与 SPI 头字段不一致，保留原始文件并标 trial，详见下文。 |
| M5Stack StickC-Plus / K016-P | image-pinned | m5stack-k016-p-factory-reference | 固定官方出厂远程镜像；执行/板级适配状态见固件目录，来源与布局校验不等同运行验证。 |
| M5Stack Core2 / K010 | image-pinned | m5stack-k010-official-factory | 固定官方出厂远程镜像；执行/板级适配状态见固件目录，来源与布局校验不等同运行验证。 |
| M5Stack Core2 v1.1 / K010-V11 | image-pinned | m5stack-k010-v11-official-factory | 固定官方出厂远程镜像；执行/板级适配状态见固件目录，来源与布局校验不等同运行验证。 |
| M5Stack Core2 For AWS / K010-AWS | image-pinned | m5stack-k010-aws-factory-reference | 固定官方出厂远程镜像；执行/板级适配状态见固件目录，来源与布局校验不等同运行验证。 |
| M5Stack Core2 for AWS v1.3 · K010-AWS-V13 / K010-AWS-V13 | reference-only | m5stack-k010-aws-v13-official-reference | 官方 v1.3 有独立 EasyLoader；不能把旧 Core2_For_AWS 2021 镜像套给 v1.3。 |
| M5Stack FIRE v2.7 / K007-V27 | reference-only | m5stack-k007-v27-factory-reference | 官方 Fire_Factory_Firmware.exe 存在，未定位独立完整 BIN；不把 Core Factory Test 推定为 FIRE v2.7 出厂镜像。 |
| M5Stack M5GO IoT Kit v2.7 / K006-V27 | reference-only | m5stack-k006-v27-factory-reference | 官方 M5GO IoT Starter Kit v2.7.exe 存在，未定位该硬件版本独立完整 BIN；不把 Basic 固件套入。 |
| M5Stack CoreS3 Thread BR / K149 | image-pinned | m5stack-k149-official-factory | 固定官方出厂远程镜像；执行/板级适配状态见固件目录，来源与布局校验不等同运行验证。 |
| M5Stack VAMeter / K136 | image-pinned | m5stack-k136-official-reference | 固定官方出厂远程镜像；执行/板级适配状态见固件目录，来源与布局校验不等同运行验证。 |
| M5Stack CoreS3 / K128 | image-pinned | m5stack-k128-factory-reference | 固定官方出厂远程镜像；执行/板级适配状态见固件目录，来源与布局校验不等同运行验证。 |
| M5Stack CoreS3-SE · K128-SE / K128-SE | image-pinned | m5stack-k128-se-official-reference | 固定官方出厂远程镜像；执行/板级适配状态见固件目录，来源与布局校验不等同运行验证。 |
| M5Stack CoreS3-Lite / K128-Lite | image-pinned | m5stack-k128-lite-official-reference | 固定官方出厂远程镜像；执行/板级适配状态见固件目录，来源与布局校验不等同运行验证。 |
| M5Stack Core2 v1.3 / K010-V13 | reference-only | m5stack-k010-v13-factory-reference | 官方 v1.3 有独立 EasyLoader；API Core2FactoryTest 描述仅明确 Core2/Core2 v1.1，未宣称覆盖 v1.3。 |
| M5Stack PowerHub / C148/K148 | image-pinned | m5stack-c148-factory-reference | 固定官方出厂远程镜像；执行/板级适配状态见固件目录，来源与布局校验不等同运行验证。 |
| M5Stack Stamp-C3 / C056-B | not-located | — | 已查该 SKU 的官方资料；官方目录中 Arduino/LED 示例或 UIFlow 开发固件没有出厂身份依据。未定位此封装/排针版本的可信完整出厂镜像。 |
| M5Stack Stamp-C3U / C122-B | not-located | — | 已查该 SKU 的官方资料；官方目录中 Arduino/LED 示例或 UIFlow 开发固件没有出厂身份依据。未定位此封装/排针版本的可信完整出厂镜像。 |
| M5Stack Stamp-Pico · C050-B / C050-B | not-located | — | 已查该 SKU 的官方资料；官方目录中 Arduino/LED 示例或 UIFlow 开发固件没有出厂身份依据。未定位此封装/排针版本的可信完整出厂镜像。 |
| M5Stack Stamp-C5 / S016 | not-located | — | 已查该 SKU 的官方资料；官方目录中 Arduino/LED 示例或 UIFlow 开发固件没有出厂身份依据。未定位此封装/排针版本的可信完整出厂镜像。 |
| M5Stack StackChan / K151/K151-R | image-pinned | m5stack-k151-official-factory | 固定官方出厂远程镜像；执行/板级适配状态见固件目录，来源与布局校验不等同运行验证。 |
| M5Stack Stamp-P4 / S013 | not-located | — | 已查该 SKU 的官方资料；官方目录中 Arduino/LED 示例或 UIFlow 开发固件没有出厂身份依据。未定位此封装/排针版本的可信完整出厂镜像。 |
| M5Stack Stamp-C6LoRa / S012 | not-located | — | 已查该 SKU 的官方资料；官方目录中 Arduino/LED 示例或 UIFlow 开发固件没有出厂身份依据。未定位此封装/排针版本的可信完整出厂镜像。 |
| M5Stack StampFly / K138 | image-pinned | m5stack-k138-official-factory | 固定官方出厂远程镜像；执行/板级适配状态见固件目录，来源与布局校验不等同运行验证。 |
| M5Stack StickC · K016-C / K016-C | image-pinned | m5stack-k016-c-official-factory | 固定官方出厂远程镜像；执行/板级适配状态见固件目录，来源与布局校验不等同运行验证。 |
| M5Stack Faces Kit / K005 | reference-only | m5stack-k005-factory-reference | 官方 FACES_FactoryTest.exe 和 FACES-Firmware 示例存在；未定位该套件完整 BIN，不把普通 Basic Factory Test 套入。 |
| M5Stack Basic v2.7 / K001-V27 | image-pinned | m5stack-k001-v27-official-factory | 固定官方出厂远程镜像；执行/板级适配状态见固件目录，来源与布局校验不等同运行验证。 |
| M5Stack Basic / K001 | image-pinned | m5stack-k001-official-factory | 固定官方出厂远程镜像；执行/板级适配状态见固件目录，来源与布局校验不等同运行验证。 |

`not-located` 仅表示本轮官方产品资料和所查 M5Burner 集合未定位，不能推论厂商从未发布。新 WebBurner、EasyLoader 内嵌镜像、社区转发和独立硬件修订仍有后续调查空间。本轮没有运行 EXE、解包并镜像第三方 BIN，也没有把未知来源填为官方。

## CrossPoint 分组与启动布局

- M5Stack 分发的 PaperMono CrossPoint 1.5.0 保留在“官方”组，使用已有完整 merged 镜像。
- 上游 1.6.5 与 2.0.0rc 放“CrossPoint”组，来源为官方 Web Flasher 的 PaperMono endpoint。它们是 ESP32-S3 应用镜像；两份静态 JSON 清单使用 `app_direct:true`，固定 app 与本地 ROM 的 hash。
- 这是明确标注的实验直启：跳过 ROM 启动、bootloader 与分区初始化，可能缺少初始化数据而失败；没有伪装成 merged。现有刷机实现拒绝 app-direct 清单，实机应使用上游安装器。
- 稳定版有独立 watchlist；RC 固定版本暂不自动跟踪，因为现有 GitHub latest 适配器只查稳定版，不能用它覆盖 RC。可变 endpoint 内容漂移时 hash 校验会拒绝。

## 证据与复核

- 机器可读逐产品记录：`factory-firmware-audit-2026-10-27.json`。
- 来源、布局、分区与 hash：`../evidence/factory-firmware-catalog/`。
- 上游 app 浏览器 fetch/hash 回执：`../evidence/factory-firmware-catalog/crosspoint-app-source-receipt.json`。
- Schema 校验通过；目录顺序/default ID、trial 策略和 watchlist 共 9 个结构测试通过。

只检查来源/镜像布局与校验和，不执行固件：

```sh
python3 tools/audit-factory-firmware-sources.py \
  --receipts docs/platform/evidence/factory-firmware-catalog \
  --metadata-url https://m5burner-api.m5stack.com/api/firmware
```

该命令对固定版本进行重新核对；上游删除版本、改变发布者或替换内容会失败，不会默默升级目录。额外严格复核发现 **StickT2 User Demo v1.0** 的 bootloader 附加摘要不匹配，已通过有界 Range 读取定位 firmwareId `8f339a3d2b3a9bdb1b693eddecb756c1`。发布文件在 `0x1000 + 2` 的 SPI mode 是 DOUT（3），但仅把该字节改为 DIO（2）计算出的摘要便与已存摘要完全一致；段 XOR 校验通过。因此是公开镜像头部与内嵌摘要不一致，而非本工具段/填充计算偏移。没有修改、重新打包或执行原始镜像；目录仍为 trial 并增加明确限制。是否由上游打包时改写及实际启动表现未验证。其余镜像的全部内嵌摘要没有系统复核，不能统称通过。
