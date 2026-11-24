#!/usr/bin/env python3
"""Exact C5 core SKUs, retrieved source receipts and bounded digital contracts.

Original binaries remain remote. A TestCode image is an official test image;
its filename alone is not evidence that it is a factory restoration release.
"""
import argparse
import copy
import datetime
import gzip
import hashlib
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]


def read(p):
    return json.loads(p.read_text())


def write(p, value):
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')


def sha(b):
    return hashlib.sha256(b).hexdigest()


def utc(t):
    return datetime.datetime.fromisoformat(t.replace('Z', '+00:00')).astimezone(datetime.timezone.utc).isoformat(timespec='seconds').replace('+00:00', 'Z')


def slug(s):
    return re.sub('[^a-z0-9]+', '-', s.lower()).strip('-')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source-root', required=True, type=Path)
    parser.add_argument('--inventory-root', required=True, type=Path)
    a = parser.parse_args()
    inventory = {p['listingId']: p for p in read(a.inventory_root / 'queue-c5-classic.json')['items']}
    stamp = datetime.datetime.now(datetime.timezone.utc).isoformat(timespec='seconds').replace('+00:00', 'Z')
    families = [
        ('ESP32-C5-MINI-KIT', 'esp32-c5-mini-kit', 'waveshare-c5-mini-kit', 'RGB'),
        ('ESP32-C5-Pico', 'esp32-c5-pico', 'waveshare-c5-pico', 'RGB'),
        ('ESP32-C5-Zero', 'esp32-c5-zero', 'waveshare-c5-zero', 'GRB'),
        ('ESP32-C5-WIFI6-KIT', 'esp32-c5-wifi6-kit-n16r4', None, 'GRB'),
    ]
    for family, listing, common_board, order in families:
        source_root = a.source_root / family
        target = ROOT / 'docs/platform/research/waveshare-c5-core-20261124' / slug(family)
        target.mkdir(parents=True, exist_ok=True)
        model_id = 'waveshare-' + slug(family)
        model_dir = ROOT / 'web/platform/assets' / model_id
        model_manifest = read(model_dir / 'source-manifest.json')
        plan = read(model_dir / 'model-plan.json')
        sources, assets, manifest = [], [], []
        aliases = {}

        def retain(sid, title, original, receipt, local_only=False):
            body = original.read_bytes()
            assert len(body) == receipt['bytes'] and sha(body) == receipt['sha256'], original
            url = receipt.get('requestedUrl', receipt.get('url'))
            row = {'id': sid, 'title': title, 'url': url, 'kind': 'manufacturer-source',
                   'status': 'fetched', 'resolvedUrl': receipt.get('finalUrl', url),
                   'fetchedAt': utc(receipt['fetchedAt']), 'bytes': len(body), 'sha256': sha(body),
                   'archiveName': slug(family) + '--' + original.name}
            evidence = {'id': sid, 'source': dict(receipt)}
            if local_only:
                evidence['retention'] = 'local-only-no-redistribution'
                row['notes'] = 'Original downloaded bytes retained locally; only the URL, hash and review are redistributed. No complete application acceptance is implied.'
            else:
                name = original.name + '.gz'
                compressed = gzip.compress(body, mtime=0)
                (target / name).write_bytes(compressed)
                evidence.update(archive=name, archiveSha256=sha(compressed))
                row['notes'] = 'Original bytes retained at ' + str((target / name).relative_to(ROOT))
            sources.append(row)
            assets.append({'id': sid, 'sourceId': sid, 'role': 'source-document', 'url': url})
            manifest.append(evidence)
            return sid

        overview = read(a.inventory_root / 'docs' / (family + '.json'))
        retain('overview', 'Exact official overview and SKU table', a.inventory_root / 'docs' / (family + '.html'), overview['source'])
        for number, receipt_path in enumerate(sorted(source_root.glob('*.receipt.json'))):
            r = read(receipt_path)
            if 'error' in r or not r.get('bytes') or not r.get('fetchedAt'):
                # An embedded data-URI is part of the retained overview, not
                # a second independent HTTP retrieval. Do not invent a receipt.
                continue
            name = 'tree.json' if receipt_path.name == 'tree.receipt.json' else receipt_path.name.removesuffix('.receipt.json')
            p = source_root / name
            if not p.is_file():
                continue
            sid = 'reference-' + str(number)
            if name == 'resources.html': sid = 'resources'
            elif name == 'tree.json': sid = 'repository-tree'
            elif name == 'factory-test-code.bin': sid = 'test-image'
            elif name.endswith('.pdf') and ('schematic' in name.lower() or name in ('ESP32-C5-MINI-KIT.pdf', 'hardware__schematics__ESP32-C5-Zero.pdf')):
                sid = 'schematic' if 'schematic' not in aliases else sid
                aliases['schematic'] = sid
            elif name.endswith('.ino') and ('WS2812' in name or 'BlinkRGB' in name):
                sid = 'rgb-example'
            elif name.endswith('test_app_tca9554.c'): sid = 'expander-example'
            elif name.endswith('NXRX-Demo.zip'): sid = 'demo-archive'
            retain(sid, name, p, r, name.endswith('.bin') or p.stat().st_size > 12_000_000)
        source_map = {s['id']: s for s in sources}
        assert len(source_map) == len(sources)
        schematic = aliases['schematic']
        for i, s in enumerate(model_manifest['sources']):
            sid = 'model-' + str(i)
            source = {'id': sid, 'title': s.get('title', s.get('id', s['file'])),
                      'url': s['url'], 'kind': 'manufacturer-mechanical-source', 'status': 'fetched',
                      'resolvedUrl': s.get('finalUrl', s['url']), 'fetchedAt': utc(s['fetchedAt']),
                      'bytes': s['bytes'], 'sha256': s['sha256'], 'archiveName': model_id + '--' + Path(s['file']).name,
                      'notes': 'Retained model source and compression receipt: ' + str((model_dir / 'source-manifest.json').relative_to(ROOT))}
            sources.append(source)
            assets.append({'id': sid, 'sourceId': sid, 'role': 'model-reference', 'url': s['url']})
        memory_sources = ['overview', schematic]
        rgb_sources = [schematic, 'rgb-example' if 'rgb-example' in source_map else 'demo-archive']
        limits = [
            '仅实现有依据的BOOT GPIO28和GPIO27 WS2812数字RMT输出；USB、射频、充电模拟电气和整机功耗没有实机验收。',
            '当前C5公开运行时仅支持SRAM直启；完整ROM/Flash镜像保持trial尝试入口并明确返回C5_SRAM_REQUIRED，不把内部诊断程序上架。',
            '预焊排针和外接天线SKU分别标识；共用主板外观模型不表示已还原各SKU的排针、电池、FPC天线附件。',
            '机械检查是生产Three加载器的尺寸/来源/射线检查；没有浏览器WebGL或实物装配验收。',
        ]
        if family.endswith('Pico'):
            limits += ['TCA9554@0x20仅在SDA23/SCL24路由；芯片8端口与排针引出数量分开。官方overview写7个扩展IO，官方示例仅暴露EXTIO1..6，冲突保留。',
                       'TCA9554 INT未接入MCU；BAT ADC6及200k/100k分压、ETA6098充电电路未建模。']
        if family.endswith('WIFI6-KIT'):
            limits += ['商品URL仍含N16R4，但当前官方SKU表为N16R8/N32R8；不据URL生成不存在的4MiB PSRAM条目。',
                       'GPIO27灯序GRB依据原厂IDF led_strip默认初始化；没有把名称为BlinkRGB但只切换数字GPIO的Arduino示例当作RGB时序证据。',
                       '此原厂STEP的RGB器件位置尚未可靠对应，几何不虚构LED交互定位；数字RGB输出仍可观察。']
        if family.endswith('Zero'):
            limits += ['官方TestCode文件是完整测试镜像，原目录未明确称FactoryOnly恢复包，保留test-image身份，不虚构出厂版本。',
                       '厂家STEP与当前照片中的RF屏蔽结构存在差异；不凭空补上未知屏蔽高度。']
        products, knowledge, capabilities = [], [], {}
        for variant in inventory[listing]['variants']:
            names = variant['documentedProductNames']
            assert len(names) == 1
            name, sku = names[0], variant['sku']
            flash = 32 if 'N32R8' in name else 16 if 'N16R8' in name else 4
            psram = 8 if common_board is None else 0
            board = common_board or 'waveshare-c5-wifi6-n' + str(flash) + 'r8'
            pid = 'waveshare-' + slug(name)
            summary = f'{flash} MiB Flash / {psram} MiB PSRAM；BOOT GPIO28，GPIO27 WS2812（{order}线序）；精确SKU {sku}。'
            public_sources = [{'id': s['id'], 'title': s['title'], 'url': s['url'], 'checkedAt': s['fetchedAt'][:10]} for s in sources if s['id'] in ['overview', 'resources', schematic, 'rgb-example', 'demo-archive', 'test-image']]
            interfaces = [{'id': 'usb-c', 'name': 'USB-C 编程与供电', 'type': 'USB-C', 'external': True, 'pins': [], 'sourceIds': ['overview', schematic]},
                          {'id': 'gpio-header', 'name': '主板GPIO焊盘/排针', 'type': 'GPIO', 'external': True, 'pins': [], 'sourceIds': [schematic]}]
            if family.endswith('Pico'):
                interfaces += [{'id': 'gpio-expander', 'name': 'TCA9554 扩展IO（引出数量冲突已记录）', 'type': 'I2C', 'external': True, 'pins': [23, 24], 'sourceIds': [schematic, 'expander-example']}]
            product = {'id': pid, 'name': 'Waveshare ' + name, 'manufacturer': 'Waveshare', 'sku': sku,
                       'hardwareRevision': 'unspecified', 'chip': 'esp32c5', 'summary': summary,
                       'knowledgeUrl': './knowledge/' + pid + '.json', 'family': {'id': 'waveshare-' + slug(family), 'name': family},
                       'runtime': {'board': board, 'flashMb': flash, 'psramMb': psram, 'status': 'partial', 'touch': False},
                       'displayStatus': 'none', 'display': None,
                       'controls': [{'id': 'boot', 'name': 'BOOT', 'type': 'button', 'pin': 28, 'activeLow': True, 'defaultKeyCode': 'Space'}],
                       'interfaces': interfaces, 'sources': public_sources, 'limitations': limits,
                       'chronology': {'date': None, 'basis': 'unknown', 'sourceId': None, 'sourceUrl': None, 'confidence': 'unknown',
                                      'notes': '型号按官方店铺顺序排序；固件/资料日期不冒充产品发布日期。'},
                       'model': {'type': 'manufacturer-cad', 'url': './assets/' + model_id + '/model.json', 'status': 'partial',
                                 'provenance': 'official-cad' if family.endswith(('Zero', 'WIFI6-KIT')) else 'dimension-photo-reconstruction'}}
            facts = [
                {'id': 'identity', 'label': '精确SKU与版本', 'basis': 'official', 'sourceIds': ['overview'], 'value': {'sku': sku, 'name': name, 'options': variant['options']}},
                {'id': 'memory', 'label': '芯片和实际存储容量', 'basis': 'official', 'sourceIds': memory_sources, 'value': {'chip': 'ESP32-C5', 'flashMiB': flash, 'psramMiB': psram}},
                {'id': 'boot', 'label': 'BOOT输入', 'basis': 'official', 'sourceIds': [schematic], 'value': {'gpio': 28, 'activeLow': True}},
                {'id': 'rgb', 'label': '板载WS2812数字路径', 'basis': 'official', 'sourceIds': rgb_sources, 'value': {'gpio': 27, 'count': 1, 'order': order}},
                {'id': 'model-sources', 'label': '外观建模依据与边界', 'basis': 'official', 'sourceIds': [s['id'] for s in sources if s['id'].startswith('model-')],
                 'value': {'method': plan['method'], 'limitations': model_manifest['limitations']}, 'notes': '此条说明取得的厂家资料及重建方法，不把重建结果当作官方新尺寸。'},
            ]
            if family.endswith('Pico'):
                facts.append({'id': 'expander', 'label': 'TCA9554真实总线与官方数量冲突', 'basis': 'official', 'sourceIds': [schematic, 'expander-example', 'overview'],
                              'value': {'address': 32, 'sda': 23, 'scl': 24, 'chipPorts': 8, 'exampleExposedPorts': 6, 'overviewExposedPorts': 7, 'interruptRouted': False}})
            unknowns = [{'id': 'release-date', 'label': '产品发布日期', 'basis': 'unknown', 'value': None, 'sourceIds': ['overview']},
                        {'id': 'exact-uiflow', 'label': '精确板型UIFlow恢复镜像', 'basis': 'unknown', 'value': None, 'sourceIds': ['resources'],
                         'notes': '共享UIFlow/Xiaozhi官方目标树调查位于docs/platform/research/waveshare-shared-ecosystem-sources；未据通用C5支持推断该板已适配。'},
                        {'id': 'hardware-acceptance', 'label': '真实硬件验收', 'basis': 'unknown', 'value': None, 'sourceIds': []}]
            record = {'schemaVersion': '1', 'id': pid, 'kind': 'product', 'catalogId': pid, 'title': product['name'],
                      'summary': summary, 'summarySourceIds': list(dict.fromkeys(memory_sources + rgb_sources)), 'updatedAt': stamp,
                      'hardwareRevision': 'unspecified', 'facts': facts, 'unknowns': unknowns, 'revisions': [],
                      'sources': sources, 'assets': assets, 'collectionNotes': limits + model_manifest['limitations']}
            assert all(source['fetchedAt'] <= stamp for source in sources)
            knowledge.append(record)
            products.append(product)
            capabilities[board] = {'chip': 'esp32c5', 'flashMb': flash, 'psramMb': psram, 'sdCard': False, 'boardButtons': []}
        ids = [p['id'] for p in products]
        fid = model_id + ('-testcode' if 'test-image' in source_map else '-official-reference')
        firmware = {'id': fid, 'name': family + (' · 官方TestCode完整测试镜像（trial）' if 'test-image' in source_map else ' · 官方示例源码资料'),
                    'productIds': ids, 'status': 'trial' if 'test-image' in source_map else 'reference',
                    'source': None, 'role': 'example', 'publisher': 'Waveshare', 'publisherGroup': '官方',
                    'simulationVerified': False, 'hardwareValidated': False, 'storage': 'remote-links-only',
                    'version': 'pinned-official-test-image' if 'test-image' in source_map else 'source-reference',
                    'publicationUrl': source_map['resources']['url'], 'sources': products[0]['sources'], 'limitations': limits}
        if 'test-image' in source_map:
            s = source_map['test-image']
            audit = read(source_root / 'factory-test-code.audit.json')
            assert {x['chipId'] for x in audit['images']} == {23} and audit['partitionMd5Valid']
            assert max(p['offset'] + p['size'] for p in audit['partitions']) <= 4 << 20
            firmware['source'] = {'url': s['url'], 'sha256': s['sha256'], 'bootMode': 'rom'}
            write(target / 'official-test-image-audit.json', audit)
            for k in knowledge:
                k['facts'].append({'id': 'official-test-image', 'label': 'TestCode原镜像布局校验', 'basis': 'official', 'sourceIds': ['test-image'], 'value': audit,
                                   'notes': '完整测试镜像，未宣称FactoryOnly出厂恢复身份；checksum/digest/MD5通过不等于原厂应用成功。'})
        else:
            firmware['limitations'] = limits + ['本次原厂资源和精确仓库调查未发现完整可启动镜像；仅源码参考，不把资源位图或编译探测bin当固件。']
        for p in products:
            p['defaultFirmwareId'] = fid
        watch = []
        if 'repository-tree' in source_map:
            tree = read(source_root / 'tree.json')
            url = source_map['repository-tree']['url']
            repo = re.search(r'/repos/([^/]+/[^/]+)/', url).group(1)
            watch.append({'id': model_id + '-official', 'name': family + ' 官方示例发布', 'category': 'development',
                          'upstreamRepo': repo, 'metadataUrl': 'https://api.github.com/repos/' + repo + '/releases/latest',
                          'releaseUrl': 'https://github.com/' + repo + '/releases', 'version': tree['sha'], 'checkedAt': stamp,
                          'assets': ([{'name': Path(source_map['test-image']['url']).name, 'url': source_map['test-image']['url'],
                                       'sha256': source_map['test-image']['sha256'], 'bytes': source_map['test-image']['bytes'], 'format': 'merged'}] if 'test-image' in source_map else []),
                          'compatibility': {'status': 'reference-only', 'productIds': ids, 'simulationVerified': False, 'hardwareValidated': False,
                                            'note': '精确官方仓库发布监视；未建立自动替换目录固件的映射，源码Release不等于可运行固件。'},
                          'catalogTargets': [], 'storage': 'remote-links-only'})
        else:
            watch.append({'id': model_id + '-official', 'name': family + ' 官方静态资料',
                          'category': 'development', 'metadataProvider': 'manual',
                          'upstreamRepo': 'manufacturer:Waveshare/' + family,
                          'metadataUrl': source_map['resources']['url'], 'releaseUrl': source_map['resources']['url'],
                          'version': 'pinned-manufacturer-resource', 'checkedAt': stamp, 'assets': [],
                          'monitoringNote': 'Official static Demo ZIP resource; no release API or complete original BIN was found. Review future downloads manually.',
                          'compatibility': {'status': 'reference-only', 'productIds': ids,
                                            'simulationVerified': False, 'hardwareValidated': False,
                                            'note': 'Static source reference only; no automatic catalog firmware replacement.'},
                          'catalogTargets': [], 'storage': 'remote-links-only'})
            for k in knowledge:
                k['collectionNotes'].append('原厂只提供下载ZIP，本次未发现精确官方GitHub发布仓库；使用metadataProvider=manual保存真实厂家资料页和人工复核时间，不伪造发布API或自动替换固件。')
        write(target / 'source-manifest.json', {'schemaVersion': 1, 'id': slug(family), 'sources': manifest})
        for k in knowledge:
            write(ROOT / 'web/platform/knowledge' / (k['id'] + '.json'), k)
        fragment = {'listingId': listing, 'products': products, 'firmwares': [firmware], 'watchlist': watch,
                    'knowledge': [{'path': 'web/platform/knowledge/' + k['id'] + '.json'} for k in knowledge],
                    'boardCapabilities': capabilities, 'sourceReceiptPaths': [str((target / 'source-manifest.json').relative_to(ROOT))],
                    'limitations': limits}
        write(Path(__file__).parent / (listing + '.json'), fragment)
        print(listing, len(products), 'SKU', len(sources), 'sources')


if __name__ == '__main__':
    main()
