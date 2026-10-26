# 固件关注列表的浏览器直连来源审计

**本地白名单不能解除远程服务器的 CORS 限制。** 从正式 GitHub Pages origin（https://zhuzhe1983.github.io）进行真实 Chromium 检查后，原始 7 项关注列表的版本元信息均可读取，原固件 URL 均无法直接跨域读取。随后找到官方替代入口，并在内存中完整读取 4 个候选：**CrossPoint、WLED、Tasmota 与原 GitHub 固件 SHA256 完全一致；UIFlow2 不一致，禁止作为同一资产替换。** StopWatch 已有单独核验的官方 CDN 来源。

完整 UTC、响应头、来源文本哈希、镜像布局和 SHA256 回执见[审计 JSON](watchlist-browser-sources-2026-10-26.json)。本研究未修改关注列表、检查器或 UI，也未保存第三方固件二进制。

## 检查方法

审计开始于 2026-10-26T15:34:37Z，完整固件校验结束于 2026-10-26T15:48:05.264Z。使用无登录状态的 Headless Chromium 155，在正式 Pages 页面执行 fetch，mode=cors、credentials=omit、cache=no-store。任务浏览器已关闭，并验证进程退出。

检查分为四层，不能互相替代：

1. HTTP 可达：终端携带 Origin 的 HEAD，记录状态及重定向各跳响应头。HTTP 200 不等于浏览器能读。
2. 浏览器响应可读：真实浏览器 HEAD，以及 GET 到响应头后立即取消，不读取固件正文。这只证明 CORS 放行到响应阶段。
3. 完整资产身份：仅对 4 个候选完整 GET，流式累计字节数，单文件上限 32 MiB、超时 45 秒，在内存用 WebCrypto 计算 SHA256；不写文件或持久缓存。
4. 布局与兼容性：读取镜像头和常规分区表位置，只判断组成。本轮未运行这 4 个固件、未刷写硬件；SHA 相同不能证明目标产品的屏幕、供电、按键和外设兼容。

CORS 由目标服务器响应头决定；本应用允许某个域名，不能替远端服务器授权。[MDN CORS 说明](https://developer.mozilla.org/en-US/docs/Web/HTTP/Guides/CORS)。

## 逐项目结果

| 项目 | 官方来源或入口 | 与固定资产的关系 | 建议 |
| --- | --- | --- | --- |
| StopWatch V0.5 | [M5Burner CDN](https://m5burner-cdn.m5stack.com/firmware/6ea670900726398df06ab72c2915db3e.bin) | 本审计 HEAD/GET 响应可读；现有目录另有完整 SHA 回执 | 保留核验过的 CDN、原 GitHub 链接和固定 SHA |
| UIFlow2 PaperMono v2.5.3 | [M5Burner CDN 候选](https://m5burner-cdn.m5stack.com/firmware/4f78f660c97149036ed9a19966775a83.bin) | 同版本、同大小，**SHA 不同** | 不替换原资产；可另列 [M5Burner 官方入口](https://burner.m5stack.com/) |
| CrossPoint 1.6.5 | [官方 PaperMono release API](https://crosspointreader.com/api/release/firmware?device=papermono) | 完整 SHA 匹配；应用分区镜像；URL 跟随 latest | 可作为固定 SHA 来源，漂移拒绝；整机安装另链[官方安装器](https://crosspointreader.com/#flash-tools) |
| WLED v16.0.1 | [官方下载镜像](https://download.wled.me/wled/WLED/releases/download/v16.0.1/WLED_16.0.1_ESP32.bin) | 完整 SHA 匹配；经典 ESP32 应用镜像 | 可用同版本镜像及固定 SHA；完整安装另链[官方安装器](https://install.wled.me/) |
| Tasmota v16.0.0 | [官方安装站 factory 镜像](https://tasmota.github.io/install/firmware/release/tasmota32s3.factory.bin) | 完整 SHA 匹配；S3 factory 布局；路径动态更新 | 固定 SHA，漂移拒绝；另列[官方安装器](https://tasmota.github.io/install/) |
| Meshtastic | [官方安装器](https://flasher.meshtastic.org/) | 文件服务只允许指定官方 origin；关注资产是多板型 ZIP | 不伪造可直连镜像，保留参考用途和官方安装入口 |
| MicroPython | [官方 ESP32_GENERIC_S3 下载页](https://micropython.org/download/ESP32_GENERIC_S3/) | 官网 bin 无跨域放行；未找到已验证官方替代源 | 保留官方下载和本地文件流程，不宣称直连已解决 |

本轮只采用上游项目自身公开的官方来源，没有使用第三方代理，也没有要求关闭浏览器安全检查。

## 内存完整下载的证据

| 候选 | 实际字节数 | 实测 SHA256 | 与固定 GitHub 资产比较 |
| --- | ---: | --- | --- |
| UIFlow2 M5Burner CDN | 16,773,120 | f3da049b65b5a3899d3484594e3bede92a0928e5217fa260925951a777447b05 | **不匹配** |
| CrossPoint 官方 API | 5,639,376 | 9bae73edef15acc53566a1b4b466361846d4b2e5202d466ab890ec610cb3a0ab | 匹配 |
| WLED 官方镜像 | 1,304,720 | 628917b0753edcfc9a8408e6387c6d1ace6a360d315441e9563d60299fef8594 | 匹配 |
| Tasmota 官方安装站 | 3,074,224 | b336f8933e720cf00b25296c0b54845c652248386773f4768b2bddceaefee91b | 匹配 |

UIFlow2 原 GitHub 资产期望 SHA 是 edb3b8678c09deb1862a3448a17bca1fa44592ac4d95b9ae88bdd2b879e88366。[M5Burner 官方目录](https://m5burner-api.m5stack.com/api/firmware)的 fid=99f8186afdcd3404618a3f80ef6abe22 确实标明 PaperMono、v2.5.3、2026-09-11 和官方发布者，仍不足以证明两个分发文件相同。负面结果已保留；没有保存完整目录，只保留所需官方产品摘录与原响应哈希，省略其他作者信息。

StopWatch 现有关注列表另有 2026-10-26T15:29:31.718Z 的内存 browser-GET 回执：3,854,192 字节，SHA 为 6395799089341e0ae96072c2a3418d469d6797b2ab8ee0cab1c9ed24a42acbbe，与 V0.5 GitHub 资产一致。该既有回执原样摘入 JSON；本轮未重复下载正文。

## 镜像格式与适配边界

- CrossPoint：开头为 ESP32-S3 镜像，常规 0x8000 / 0x7000 位置无分区表。固定 1.6.5 标签的发布流程直接复制 PlatformIO 的 firmware.bin；PaperMono 继承的上传偏移为 0x10000。因此是应用分区镜像，不能当作 0x0 完整固件。[发布流程](https://github.com/crosspoint-reader/crosspoint-reader/blob/1.6.5/.github/workflows/release.yml)、[固定版本配置](https://github.com/crosspoint-reader/crosspoint-reader/blob/1.6.5/platformio.ini)。
- WLED：镜像头 chip ID 为 0，属于经典 ESP32，常规位置无分区表。官方安装器将应用写入 0x10000，并另配 bootloader 与分区表；不是 StopWatch / PaperMono 的 S3 整机固件。[官方安装器源码](https://install.wled.me/app.js)。
- Tasmota：chip ID 为 9，0x8000 有分区表，0x10000 是 safeboot，0xE0000 是 app0，符合 factory 合并布局。官方 manifest 也指定 ESP32-S3、偏移 0；仍未验证目标产品外设。[官方 manifest](https://tasmota.github.io/install/manifest_ext/release.tasmota.manifest.json)。
- UIFlow2 候选：chip ID 为 9，0x8000 有分区表、0x10000 有 factory 应用，后有 sys / vfs。它只是结构可识别的另一份官方分发，不得因布局完整而忽略 SHA 不同。

头字段按 Espressif 的[镜像格式定义](https://docs.espressif.com/projects/esptool/en/latest/esp32s3/advanced-topics/firmware-image-format.html)读取；S3 的 chip ID=9 对照[官方 esptool 实现](https://github.com/espressif/esptool/blob/master/esptool/targets/esp32s3.py)。这里只检查结构特征，没有冒称完整启动验证。

## 容易误判的响应

Meshtastic 的限制作用于 origin。对同一个官方 Heltec V3 factory.bin 做 HEAD，两个请求都返回 200，但只有官方刷机站得到 CORS 放行：

| Origin | Access-Control-Allow-Origin |
| --- | --- |
| https://zhuzhe1983.github.io | 缺失 |
| https://flasher.meshtastic.org | https://flasher.meshtastic.org |

真实 Pages 浏览器 GET 同样被拒绝。其[官方版本 API](https://api.meshtastic.org/github/firmware/list)可以跨域读取，不代表固件文件服务允许任意站点。[目标 manifest](https://release.meshtastic.org/2.7.26.54e0d8d/firmware-heltec-v3-2.7.26.54e0d8d.mt.json)只用于确定测试文件；Heltec 并非当前产品适配推荐。关注列表的 170,272,165 字节 ZIP 也不能直接视为单板完整镜像。

GitHub API 限额与 CORS 是两回事。最初 7 项版本元信息均为可读 JSON；后续 asset API 加 Accept: application/octet-stream 的尝试遇到 403，响应显示匿名限额剩余 0。因此不能断言 asset API 永久不支持 CORS，也不能包装成已验证的通用替代方案。应保留上次元信息、单独显示限额错误；匿名 REST API 公开额度为每小时 60 次。[GitHub 官方限额说明](https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api)。

Content-Length 也可能是压缩传输长度。Tasmota 的 gzip 传输长度为 1,930,762，实际解压后为 3,074,224，SHA 与 GitHub 一致。其 manifest 的 size=2156720 已过期；不能替代实际字节和哈希验证。

## 应用建议

分别记录版本元信息、原发布链接、浏览器下载 URL 和官方安装入口。镜像替换必须带既有固定 SHA；CrossPoint、Tasmota 的动态地址变动后应报版本不一致，不自动升级或改掉期望哈希。UIFlow2 只能保留为独立候选，按其实际哈希与来源另行审核。

检查状态应区分元信息可读、HTTP 可达但 CORS 拒绝、API 限额、404、哈希不匹配、应用分区镜像以及产品兼容性未验证。七个项目都可以有官方入口；不能据此声称七个当前资产都能从纯静态网页直接加载。
