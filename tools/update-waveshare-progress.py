#!/usr/bin/env python3
"""Regenerate reviewable scope/profile progress without counting registration as acceptance."""
import collections
import datetime
import json
from pathlib import Path
import re
from urllib.parse import urlparse

root = Path(__file__).resolve().parents[1]
read = lambda p: json.loads((root / p).read_text())
inventory = read('docs/platform/research/waveshare-store-inventory-2026-11-23.json')
products = [p for p in read('web/platform/catalog/products.json')['items'] if p.get('manufacturer') == 'Waveshare']
normalize = lambda s: re.sub('[^a-z0-9]', '', s.lower())
listings = {normalize(x['listingId']): x for x in inventory['items']}
matches = collections.defaultdict(list)
outside = []
for product in products:
    found = set()
    for source in product.get('sources', []):
        if source.get('id') == 'overview':
            slug = urlparse(source['url']).path.strip('/').split('/')[-1].removesuffix('.htm')
            if normalize(slug) in listings:
                found.add(listings[normalize(slug)]['listingId'])
    if not found:
        name = normalize(product['name'].removeprefix('Waveshare ').split(' · ')[0])
        if name in listings:
            found.add(listings[name]['listingId'])
    for listing in found:
        matches[listing].append(product['id'])
    if not found:
        outside.append(product['id'])

model_artifacts = [p.parent.name for p in (root / 'web/platform/assets').glob('waveshare-*/model.json')]
now = datetime.datetime.now(datetime.timezone.utc).isoformat(timespec='seconds').replace('+00:00', 'Z')
included = [i for i in inventory['items'] if i['scope'] == 'included-controller-or-controller-kit']
rows = [{**item, 'registeredProductIds': matches[item['listingId']], 'registrationState': 'partial-profile-registered' if matches[item['listingId']] else 'not-yet-integrated', 'skuExpansionComplete': False} for item in included]
summary = {
    'scopeListings': len(included),
    'scopeListingsWithAtLeastOnePartialProfile': sum(bool(matches[i['listingId']]) for i in included),
    'scopeListingsWithoutAnIntegratedProfile': sum(not matches[i['listingId']] for i in included),
    'exactCatalogVariants': len(products),
    'runtimeStatusCounts': dict(collections.Counter(p['runtime']['status'] for p in products)),
    'modelsConnectedToCatalogProducts': sum(bool(p.get('model')) for p in products),
    'materializedModelArtifacts': len(model_artifacts),
    'catalogVariantsNotMatchedToOneListingByExactOfficialOverview': outside,
}
result = {'schemaVersion': 1, 'updatedAt': now, 'scopeSource': 'docs/platform/research/waveshare-store-inventory-2026-11-23.json', 'countsAreNotCompletionPercentages': True, 'skuExpansionComplete': False, 'summary': summary, 'modelArtifacts': sorted(model_artifacts), 'products': [{'id':p['id'],'name':p['name'],'board':p['runtime']['board'],'runtimeStatus':p['runtime']['status'],'model':p.get('model'),'limitations':p['limitations']} for p in products], 'scopeListings': rows}
(root / 'docs/platform/research/waveshare-progress-2026-11-23.json').write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n')
intro = f'''# Waveshare 开发进度（2026-11-23）

更新时间：{now}。本文件记录可核验的当前状态，**全量适配尚未完成，尚未发布 GitHub／Pages**。

## 计数口径

- 官方指定分类的 8 页包含 180 个独立商品入口，其中 149 个入口属于 ESP32 主控或包含主控的套件，1 个混合系统待复核，30 个外设／非目标入口排除。
- 当前目录有 **{len(products)} 个 Waveshare 精确配置**，全部仍标为 `partial`。配置数包括同一入口下不同容量、接口选项和芯片修订，不能直接除以 149 当完成率。
- {summary['scopeListingsWithAtLeastOnePartialProfile']} 个目标入口已有至少一个部分适配配置；{summary['scopeListingsWithoutAnIntegratedProfile']} 个目标入口尚无合入的配置。已有资料或子任务中的开发不计入合入数量。
- {summary['materializedModelArtifacts']} 个独立模型文件已经落地，其中 {summary['modelsConnectedToCatalogProducts']} 个已经连接至现有产品目录。
- SKU／套装展开仍在继续。官方概览中未能与分类入口精确对应的配置单列在 JSON 中，不擅自扩大已覆盖入口数。

清单来源：[完整分类盘点](research/waveshare-store-inventory-2026-11-23.json)。逐项状态：[进度 JSON](research/waveshare-progress-2026-11-23.json)。

## 已验证的能力与剩余工作

| 批次 | 精确配置数 | 已验证 | 主要剩余项 |
| --- | ---: | --- | --- |
| S3 ePaper 13.3E6 | 1 | 原厂画面；原生与公开 Worker 帧一致；Basic4 例程 BOOT 刷新 | 其余音频、无线、全部外设；驱动板与外接玻璃的套装差异 |
| S3 LCD／Touch LCD 1.28 | 2 | 原厂出屏、LCD 动画；QMI 数字输入；独立模型 | Touch 原厂后段 BLE 阶段和触摸页面响应；物理朝向 |
| S3 Zero／Pico／Nano／DEV-KIT N8R8 | 4 | 官方例程 LED／REPL；真实容量、BOOT 接线；4 个模型 | 各厂商程序和全部接口的进一步验收 |
| C5 LCD 1.47 基础／M | 2 | 原创诊断经公开 Worker 出屏、BOOT 反馈；TF 块读写后整卡恢复；基础 SKU 官方 CAD | 原厂 ROM／Flash 启动、原厂 FAT 挂载；M 排针几何 |
| S3 RGB 2.1／2.1B／2.8B／2.8C | 6 | 6 个原厂程序出屏并挂载默认 FAT32；4 个触摸型号早期滑动；QMI 数字输入；6 个独立官方 CAD 模型 | 后段 BLE 阶段、原厂 I2C 失败定位；部分官方 CAD 缺主芯片封装外观 |
| P4 模块与载板 | 17 | 7 个配置经公开 Worker 运行原始 IDF hello；10 个配置原厂 ROM 启动到应用阶段；精确容量与版本检查 | DSI／CSI、无线协处理器、以太网、音频；WIFI6-DB 原厂重启；独立模型 |
| C5 Touch LCD 1.69 | 1 | 完整 8 MiB PSRAM；显示、触摸、BOOT 的 6 个阶段与原生逐像素一致；独立照片与尺寸重建模型 | 原厂启动、电量计、音频、无线；两份官方尺寸图的小幅差异与未知内部几何 |
| C5 Touch LCD 2.8 | 1 | 32F8P；完整 PSRAM；公开显示／触摸／BOOT 与原生 6 阶段一致；整张默认 64 MiB FAT32 卡读写后恢复；完整官方 CAD | 原厂 PMU 启动、电源／温湿度／音频、FAT 挂载 |
| C5 Touch LCD3.5／LCD2.73 | 2 | 原创诊断完整 PSRAM、DMA、按键／触摸和 SD 整卡恢复；2 个独立模型 | 公开 Worker 验证；原厂启动与完整外设 |
| S3 AMOLED1.43C／EN包装 | 2 | 原厂九应用桌面、触摸画线、BOOT 返回；11.22秒原生零重启 | 公开 Worker；独立模型、网络与音频 |
| C6 LCD0.85／1.9、Touch1.9／1.69／1.47基础与M | 6 | 精确芯片／容量／接线；原厂镜像头校验；1.69、1.47、0.85 原生出屏；1.47 两个官方 CAD 装配 | SPI DMA 接收与 SD 挂载回归；触摸与无触摸1.9分别验收；公开 Worker |
| P4 WIFI6 Touch LCD 3.5 V1.3／V3.2 | 2 | 两个原厂镜像独立匹配版本；实际启动画面 | C6 SDIO 初始化失败后异常重启；完整交互、外设、模型 |

上述“公开 Worker”指实际平台 runtime → production Worker → WASM 在 Node 下执行。浏览器页面／WebGL 验收仍未完成，不与这些测试混为一谈。

## 关键证据

- [13.3E6](../evidence/waveshare-s3-epaper133-2026-11-23/README.md)
- [1.28 原厂运行与来源](../evidence/waveshare-legacy128-2026-11-23/manifest.json)
- [4 款 S3 核心板](../evidence/waveshare-s3-core-2026-11-23/README.md)
- [C5 LCD1.47 公开执行](../evidence/waveshare-c5-lcd147-web-2026-11-23/public-runtime-after-i2c-result.json)
- [6 款 RGB 原厂 FAT32 公开验证](../evidence/waveshare-tca-rgb-public-sdmmc-2026-11-23/results.json)
- [P4 10 组原厂执行](../evidence/waveshare-p4-platform-2026-11-23/factory-runs.json)
- [C5 Touch1.69 公开执行及逐像素对照](../evidence/waveshare-c5-touch169-web-2026-11-23/result.json)
- [P4 3.5 两个修订的真实失败边界](../evidence/waveshare-p4-lcd35-2026-11-23/results.json)
- [两款 C5 官方 CAD 与包件边界](../evidence/waveshare-c5-models-2026-11-23/README.md)

## 当前并行开发

1. C5 Touch3.5／LCD2.73：代码、知识和模型已合入；公开 Worker 验证及原厂启动继续推进。
2. S3 AMOLED1.43C：原生原厂桌面、画线与 BOOT 已通过；2.06／2.16 继续精确电源、RTC 与触摸适配。
3. AMOLED1.43C：独立尺寸／照片模型制作中；C5 LCD2.73 和 C6 Touch1.47 两个装配的模型已合入。
4. 主工作树：整合、回归、P4 后续屏幕和剩余 C6 型号。

不使用文件日期推定产品发布日期；有可靠发布日期的新品优先，其余以当前带屏产品和可共享的真实协议推进。

## 149 个目标入口

“已有部分配置”只表示下面列出的配置已进入目录；该入口的全部 SKU 和功能可能仍未完成。

| 官方商品入口 | 当前合入状态 | 配置数量 |
| --- | --- | ---: |
'''
for row in rows:
    intro += f"| [{row['listingId']}]({row['url']}) | {'已有部分配置' if row['registeredProductIds'] else '尚无合入配置'} | {len(row['registeredProductIds'])} |\n"
(root / 'docs/platform/waveshare-progress-2026-11-23.md').write_text(intro)
print(json.dumps(summary, ensure_ascii=False))
