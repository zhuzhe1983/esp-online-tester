#!/usr/bin/env python3
"""Expand source-confirmed package variants without inheriting firmware verification.

The input directory contains exact official storefront/doc captures and receipts.
Their raw HTML remains outside Git; this tool preserves selected facts and hashes.
"""
import argparse
import copy
import datetime
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]


def read(path):
    return json.loads(path.read_text())


def write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')


def utc(value):
    return datetime.datetime.fromisoformat(value.replace('Z', '+00:00')).astimezone(
        datetime.timezone.utc).isoformat(timespec='seconds').replace('+00:00', 'Z')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('inventory', type=Path)
    args = parser.parse_args()
    now = utc(datetime.datetime.now(datetime.timezone.utc).isoformat())
    catalog = read(ROOT / 'web/platform/catalog/products.json')
    firmwares = read(ROOT / 'web/platform/catalog/firmwares.json')
    index = read(ROOT / 'web/platform/knowledge/index.json')
    watch = read(ROOT / 'web/platform/catalog/firmware-watchlist.json')
    products = {p['id']: p for p in catalog['items']}
    index_rows = {p['id']: p for p in index['entries']}
    names = ['ESP32-C5-Touch-LCD-3.5', 'ESP32-C5-Touch-LCD-1.69',
             'ESP32-C5-LCD-2.73', 'ESP32-C5-Touch-LCD-2.8', 'ESP32-C5-LCD-1.47']
    results = []
    for name in names:
        listing = name.lower()
        pid = 'waveshare-' + re.sub(r'[^a-z0-9]+', '-', listing)
        base = products[pid]
        original_knowledge = read(ROOT / 'web/platform/knowledge' / (pid + '.json'))
        doc = read(args.inventory / 'docs' / (name + '.json'))
        store = read(args.inventory / 'products' / (listing + '.json'))
        table = next(t for t in doc['tables'] if t and t[0] == ['SKU', 'Product'])
        sku_names = {r[0]: r[1] for r in table[1:]}
        variants = store['variants']
        assert set(sku_names) == {v['sku'] for v in variants}, name
        sources = []
        for sid, source, title in [('sku-store-20261124', store['source'], 'Official storefront package options'),
                                   ('sku-doc-20261124', doc['source'], 'Official exact SKU and product table')]:
            sources.append({'id': sid, 'title': title, 'url': source['requestedUrl'],
                'resolvedUrl': source['finalUrl'], 'kind': 'manufacturer-product', 'status': 'fetched',
                'fetchedAt': utc(source['fetchedAt']), 'bytes': source['bytes'], 'sha256': source['sha256'],
                'archiveName': listing + '-' + sid + '.html',
                'notes': 'Hash identifies the fetched official HTTP body. Raw storefront pages remain local because they contain transient session metadata; selected SKU facts and this receipt are retained.'})
        receipt_path = Path('docs/platform/research/waveshare-20261124/variants') / (listing + '.json')
        write(ROOT / receipt_path, {'schemaVersion': 1, 'listingId': listing, 'sources': sources,
            'variants': variants, 'officialSkuNames': sku_names,
            'scope': 'Package and identity evidence; no firmware or physical validation is inherited.'})
        family_products, family_knowledge = [], []
        for variant in variants:
            sku, exact_name = variant['sku'], sku_names[variant['sku']]
            identity = 'waveshare-' + re.sub(r'[^a-z0-9]+', '-', exact_name.lower())
            old = products.get(identity)
            product = copy.deepcopy(old or base)
            product.update({'id': identity, 'name': 'Waveshare ' + exact_name, 'sku': sku,
                'knowledgeUrl': './knowledge/' + identity + '.json'})
            attrs = [a.strip() for a in variant['attributes']]
            scope_note = '官方 SKU ' + sku + '；套装选项：' + ' / '.join(attrs) + '。电池与排针包装差异不代表额外固件或实机验收。'
            if identity != pid:
                product['visualVariantOf'] = pid
                product['model'] = copy.deepcopy(base.get('model'))
                product['limitations'] = list(dict.fromkeys(product['limitations'] + [scope_note,
                    '3D 复用本系列已核验的主板／屏幕几何；电池、排针或摄像头包装的独立外观差异未完整还原。']))
            else:
                product['limitations'] = list(dict.fromkeys(product['limitations'] + [scope_note]))
            if '-C' in exact_name.removeprefix('ESP32-C5'):
                product['limitations'].append('本 SKU 带摄像头；当前板型仅运行共享显示／触摸子集，摄像头传输与摄像头外壳未模拟。')
            product['sources'] = [s for s in product['sources'] if s.get('id') not in {'sku-store-20261124', 'sku-doc-20261124'}]
            product['sources'] += [{'id': s['id'], 'title': s['title'], 'url': s['url'], 'checkedAt': s['fetchedAt'][:10]} for s in sources]
            knowledge = copy.deepcopy(original_knowledge)
            knowledge.update({'id': identity, 'catalogId': identity, 'title': product['name'],
                'summary': product['summary'], 'updatedAt': now})
            knowledge['sources'] = [s for s in knowledge['sources'] if not s['id'].startswith('sku-')] + copy.deepcopy(sources)
            knowledge['facts'] = [f for f in knowledge['facts'] if f['id'] != 'exact-package-sku'] + [{
                'id': 'exact-package-sku', 'label': '官方 SKU 与包装选项',
                'value': {'sku': sku, 'name': exact_name, 'options': attrs}, 'basis': 'official',
                'sourceIds': ['sku-store-20261124', 'sku-doc-20261124']}]
            knowledge['collectionNotes'] = list(dict.fromkeys(knowledge['collectionNotes'] + [scope_note,
                '共用板型的既有测试只证明已建模数字接口；不继承任何真实硬件、摄像头、射频或电池验证。']))
            products[identity] = product
            write(ROOT / 'web/platform/knowledge' / (identity + '.json'), knowledge)
            index_rows[identity] = {'id': identity, 'kind': 'product', 'catalogId': identity,
                'name': product['name'], 'url': './' + identity + '.json', 'updatedAt': now}
            family_products.append(product)
            family_knowledge.append(knowledge)
        ids = [p['id'] for p in family_products]
        for firmware in firmwares['items']:
            if pid in firmware.get('productIds', []):
                # Package mappings may share unverified trial images, never inherit verification.
                if firmware.get('publisher') != 'esp-online-tester':
                    assert firmware['status'] in ('reference', 'trial') and not firmware.get('simulationVerified')
                    firmware['productIds'] = list(dict.fromkeys(firmware['productIds'] + ids))
        for item in watch['entries']:
            compatible = item.get('compatibility', {})
            if pid in compatible.get('productIds', []):
                compatible['productIds'] = list(dict.fromkeys(compatible['productIds'] + ids))
        result = {'listingId': listing, 'products': family_products, 'knowledge': family_knowledge,
            'firmwares': [f for f in firmwares['items'] if set(f.get('productIds', [])) & set(ids) and f.get('publisher') != 'esp-online-tester'],
            'watchlist': [w for w in watch['entries'] if set(w.get('compatibility', {}).get('productIds', [])) & set(ids)],
            'sourceReceiptPaths': [str(receipt_path)], 'limitations': [
                'C5 runtime remains SRAM app-direct. Official merged factory boot is not supported.',
                'All physical hardware, RF, package-specific mechanical fit and camera behavior remain unverified.']}
        write(ROOT / 'tools/waveshare-batches/c5' / (listing + '.json'), result)
        results.append({'listingId': listing, 'productIds': ids, 'skuCount': len(ids)})
    catalog['items'] = list(products.values())
    index['entries'] = list(index_rows.values()); index['updatedAt'] = now
    write(ROOT / 'web/platform/catalog/products.json', catalog)
    write(ROOT / 'web/platform/catalog/firmwares.json', firmwares)
    write(ROOT / 'web/platform/catalog/firmware-watchlist.json', watch)
    write(ROOT / 'web/platform/knowledge/index.json', index)
    print(json.dumps(results, ensure_ascii=False))


if __name__ == '__main__':
    main()
