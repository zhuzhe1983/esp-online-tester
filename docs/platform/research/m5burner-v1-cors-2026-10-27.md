# M5Burner 新版网页 API：可直接跨域读取和下载

2026-10-27 实测，从 `https://zhuzhe1983.github.io` 和本地 `http://127.0.0.1:8088` 的真实无头浏览器匿名请求，设备信息、固件列表与公开固件下载均可读。无需本站代理，也无需 GitHub Actions 下载或保存固件 BIN。[结构化回执](m5burner-v1-cors-2026-10-27/receipt.json)保存接口、请求方式、响应头、源码哈希及完整下载哈希。

## 找到的接口

来源为 [Cardputer 页面](https://burner.m5stack.com/device/cardputer)实际网络请求及其 app-version=131 前端模块，不是猜测路径。共同前缀为 `https://burner.m5stack.com/api/v1`。

| 用途 | GET 路径 | 结果位置 |
| --- | --- | --- |
| 设备列表 | `/devices` | 前端使用此路径；本次未单独验证跨域响应 |
| 按型号查设备 | `/devices/by-slug/cardputer` | `data.deviceId` |
| 按设备查固件 | `/firmwares?deviceId={deviceId}&pageNum=1&pageSize=50&sort=LATEST` | `rows`、`total` |
| 固件详情 | `/firmwares/{firmwareId}` | `data` |
| 公开版本 | `/firmwares/{firmwareId}/versions?scope=PUBLIC` | `data[]` |
| 固定版本下载 | `/firmwares/{firmwareId}/versions/{versionId}/download` | HTTP 302 到官方 CDN，随后读取二进制 |

Cardputer 的 `deviceId` 是字符串 `1897100000000000005`，本次 `total=346`，表示固件项目数量，不是版本数量。必须分页，不能把首屏 50 项当作全部。`firmwareId`、`versionId` 等大整数 ID 应一直当字符串使用，不能先转 JavaScript Number 再转回，否则可能丢失精度。

详情和版本提供固件名称、版本名称、发布时间、适用设备、源码地址等信息。`sourceType=OFFICIAL` 可以区分官方与社区发布，但“官方”不自动等于该设备的出厂固件，仍需明确项目身份。每个版本的 `supportedDevices` 也需要核对，不能只用名字包含 Cardputer 来判断 Adv、原版等板型通用。

## CORS 与下载实测

- 新 API 对带我们 Pages Origin 的请求返回 `Access-Control-Allow-Origin: https://zhuzhe1983.github.io`；真实浏览器 `Response.type=cors`，能够读取 JSON。
- 模拟前端 `language,x-request-id` 的 OPTIONS 预检也返回 200，并允许这些请求头。集成公开读取接口时可以使用简单 GET，不必复制所有前端请求头。
- 下载 API 的 302 本身也带跨域许可；最终 `m5burner-cdn.m5stack.com` 文件响应带 `Access-Control-Allow-Origin: *`。两个公开固件样本均能跨域读到正文。
- Cardputer UIFlow2 v2.5.3 在 Pages 浏览器内完整读取 **8,384,512 字节**，SHA-256 为 `c6cc93073b6480da93ffb64d2e6f0f0e9b2fd4689108a1a5106d44d8ca98ad0b`。只在内存读取，没有保存 BIN，也没有启动此固件。
- 社区 TICKER 0.1.0 下载返回 200、声明 662,592 字节；读取首块后取消。它只证明响应与正文可跨域读取，未做完整哈希或兼容性验证。
- localhost 上同样验证了设备、分页和社区下载响应；没有借用用户登录态。

旧接口 `https://m5burner-api.m5stack.com/api/firmware` 本次也返回跨域可读 JSON（2,651 项，约 2.89 MB）。我们的 `m5burner` watchlist provider 目前用的是这套旧目录；新网页接口是另一套分页 API，不能把旧 `fid` 和新 `firmwareId` 混用。

## 最小调用示例

```js
const base = 'https://burner.m5stack.com/api/v1';
async function getJson(path) {
  const response = await fetch(base + path, {mode: 'cors', credentials: 'omit'});
  if (!response.ok) throw new Error(`HTTP ${response.status}`);
  const result = await response.json();
  if (result.code !== 200) throw new Error(result.msg || 'M5Burner API error');
  return result;
}
const {data: device} = await getJson('/devices/by-slug/cardputer');
const page = await getJson(`/firmwares?deviceId=${encodeURIComponent(device.deviceId)}&pageNum=1&pageSize=50&sort=LATEST`);
// page.rows 是这一页的固件；继续分页，直到取得 page.total 所示的全部项目。
const firmware = page.rows[0];
const downloadUrl = `${base}/firmwares/${encodeURIComponent(firmware.firmwareId)}/versions/${encodeURIComponent(firmware.currentVersion.versionId)}/download`;
```

将 `downloadUrl` 交给已有固件加载器，由用户浏览器跟随重定向并下载；它不是供后台定时保存 BIN 的地址。不要使用 `mode:'no-cors'`，那会得到应用无法读取的 opaque 响应。

## 接入建议与边界

新增独立 `m5burner-v1` 元信息 provider：按订阅的设备/项目分页取元信息和版本，存入本地设备 JSON；保留官方详情页、固定版本下载 URL、抓取时间、设备映射与已核对的哈希。构建前或 Action 只更新 JSON，运行时仍由用户浏览器直接取得固件并按现有逻辑缓存。保留旧来源兜底，不能直接把旧 ID 改成新 ID。

本次只研究接口，未修改固件库、watchlist 或执行策略。没有看到这套网页接口的公开稳定性或限额承诺；集成需保留上次成功结果，处理 401/403/404/429 和 API 字段变化。下载成功不证明镜像布局完整或板级模型兼容，继续沿用完整镜像/分段清单校验及现有试运行规则。
