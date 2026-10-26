# 可供 agent 使用的产品知识

知识入口为 `web/platform/knowledge/index.json`。记录包含稳定 id、catalogId、kind、title/summary、hardwareRevision、facts、revisions、sources、assets、unknowns。产品目录以相对 knowledgeUrl 关联，路径适应 GitHub Pages 子目录。

facts 用有单位的值，每项 sourceIds 指向 sources。basis 区分 official、cad-measurement、image-measurement、unknown。官方图纸标注属于 official。多个硬件版本共存时记录适用版本或单列 revisions，不能合并成一个看似确定的值。

产品至少核对：芯片型号/架构/主频，Flash/PSRAM，屏幕尺寸/分辨率/控制器/总线及 framebuffer 差异，触摸、按键、引脚顺序、电压/电源/电池、音频、RTC、IMU、其他传感器、射频、尺寸/重量、软件框架及硬件版本差异。模块补充协议、地址/默认波特率、信号电平和连接器；固件补充硬件变体、镜像布局、版本、许可与兼容性结果。

实际抓取的来源：status=fetched、url、fetchedAt（ISO 8601 UTC）、sha256、bytes、mediaType；重定向时保留最终地址。仅从官网发现的 datasheet：status=linked、discoveredFrom，不填未取得的 hash 或 fetchedAt。updatedAt 只是知识记录更新时间，不替代逐来源时间。

保留厂商、资料版本、可确认的发布时间和每次抓取身份。较新页面不代表用户手里的硬件版本。简明概括介绍，不整页转载。更新知识不覆盖过去验证固定的资料身份。

事实应能独立读取，不写“同上”或把关键数值藏在图片里；未知项说明原因和可查的来源。知识 schema 校验与模拟能力校验分开：知识完整不等于外设已经实现。
