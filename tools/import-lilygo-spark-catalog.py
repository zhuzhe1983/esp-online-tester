#!/usr/bin/env python3
"""Import the pinned Spark product inventory without inventing hardware adaptation.

Reads a curated product-only projection; no network, binaries, author records or
remote pictures. Existing products and knowledge keep their detailed hardware,
models, controls, firmware defaults and validation evidence.
"""
import argparse
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RESEARCH = ROOT / 'docs/platform/research'
PLATFORM = ROOT / 'web/platform'
DATE = '2026-10-28'
SOURCE_PATH = RESEARCH / f'lilygo-spark-product-source-{DATE}.json'
RECEIPT_PATH = RESEARCH / f'lilygo-spark-product-source-receipt-{DATE}.json'
AUDIT_PATH = RESEARCH / f'lilygo-spark-product-mapping-{DATE}.json'
# Public local identities already have exact hardware and saved scenes.
EXISTING = {
    't-display': 'lilygo-ttgo-t-display',
    't-display-s3': 'lilygo-t-display-s3',
    't-deck': 'lilygo-t-deck',
    't-embed-cc1101': 'lilygo-t-embed-cc1101',
    't-beam': 'lilygo-t-beam-v1-2',
    't-watch-s3': 'lilygo-t-watch-s3',
}
# These names identify commercial software editions, not hardware equivalence.
EDITIONS = {
    't-deck-meshtastic': 'Meshtastic',
    't-deck-plus-meshtastic': 'Meshtastic',
    't-deck-pro-meshtastic': 'Meshtastic',
    't-beam-meshtastic': 'Meshtastic',
    't-beam-softrf': 'SoftRF',
    't-beam-supreme-meshtastic': 'Meshtastic',
    't3-s3-meshtastic': 'Meshtastic',
    't-lora-pager-meshtastic': 'Meshtastic / MeshCore',
}


def read(path):
    return json.loads(path.read_text())


def serial(value):
    return json.dumps(value, ensure_ascii=False, indent=2) + '\n'


def classify(group, product):
    if group['id'] == 'general-esp32-series':
        return 'excluded-generic', '通用 Espressif 芯片入口，不是 LILYGO 商品。'
    if not product['mcu'].strip():
        return 'excluded-accessory', 'Spark 未列独立 ESP32 MCU；附件保留审计，不进入设备目录。'
    if 'ESP32' not in product['mcu'].upper():
        return 'excluded-non-esp32', f"Spark 标注 {product['mcu']}，不是 ESP32 系列产品。"
    return 'included', 'Spark 商品记录明确列出 ESP32；硬件细节与运行支持分别核验。'


def family(group):
    return {'id': group['id'], 'name': group['name'].replace(' Series', ' 系列').replace('Other T- Products', '其他 T 系列产品').replace('Other Products', '其他产品')}


def mapping(group, product, revision):
    return {'productId': product['product_id'], 'groupId': group['id'],
            'mappingScope': 'hardware-variant' if product['product_id'] in EXISTING else 'product',
            'catalogRevision': revision}


def knowledge_fact(fact_id, label, value, notes=None):
    result = {'id': fact_id, 'label': label, 'value': value, 'basis': 'official', 'sourceIds': ['spark-product-catalog']}
    if notes:
        result['notes'] = notes
    return result


def source_record(receipt):
    return {'id': 'spark-product-catalog', 'title': 'LILYGO Spark 固定提交产品目录', 'kind': 'official-product-inventory', 'status': 'fetched', **receipt['source']}


def generate_record(product, row, provider, source, stamp):
    identity = {'manufacturer': 'LILYGO', 'sku': product['sku'], 'family': product['family'], 'providers': product['providers'], 'catalogImport': 'lilygo-spark-2026-10-28'}
    facts = [knowledge_fact('spark-product-identity', 'Spark 商品与系列标识', provider, '商品标识不是硬件兼容性证明。'),
             knowledge_fact('spark-mcu-label', 'Spark 目录所列 MCU', product['module'], '仅保留目录原文；不由模块名推导容量、架构或引脚。')]
    if product.get('firmwareEdition'):
        identity['firmwareEdition'] = product['firmwareEdition']
        facts.append(knowledge_fact('spark-software-edition', 'Spark 标注的软件预装商品版', product['firmwareEdition'], '商品版本独立列出；不证明与同系列其他商品硬件互换，也不是可下载的出厂镜像。'))
    facts.append(facts.pop(0))  # Provider identity stays last on subsequent imports.
    unknowns = []
    for key, label, note in [
        ('hardware-variant', '具体硬件版次与选配', '商品页可能覆盖多个硬件、射频或屏幕选项；需要逐个核对。'),
        ('memory', 'Flash / PSRAM', '未从 Spark 摘要推导容量；目录 null 表示未知。'),
        ('display', '屏幕、触摸、总线与控制器', 'display:null 配 displayStatus:unknown 表示待核对，不代表无屏幕。'),
        ('inputs-interfaces', '按键、引脚、接口与电气参数', '空 controls/interfaces 仅表示尚未建立配置。'),
        ('mechanical', '尺寸、结构和 CAD', '本轮只补商品列表，未完成该产品的模型检索与验证。'),
        ('factory-firmware', '确切硬件的官方出厂镜像', '已检查 Spark 商品目录及其商品页/源码链接（如有）；本轮未逐个抓取、核验硬件版次和出厂镜像，不设置默认固件。'),
        ('runtime', '模拟执行与实机结果', '尚未完成板级适配，无执行或实机验证。'),
    ]:
        unknowns.append({'id': key, 'label': label, 'value': None, 'basis': 'unknown', 'sourceIds': ['spark-product-catalog'], 'notes': note})
    sources = []
    for key, label in [('product_page', 'Spark 所列商品页'), ('github_repo', 'Spark 所列源码仓库')]:
        if row[key]:
            sources.append({'id': key.replace('_', '-'), 'title': label, 'url': row[key], 'kind': 'official-linked-entry', 'status': 'linked', 'discoveredFrom': 'spark-product-catalog'})
    sources.append(source)
    return {'schemaVersion': '1', 'id': product['id'], 'kind': 'product', 'catalogId': product['id'], 'title': product['name'],
            'summary': product['summary'], 'summarySourceIds': ['spark-product-catalog'], 'updatedAt': stamp,
            'hardwareRevision': product['hardwareRevision'], 'identity': identity, 'facts': facts, 'revisions': [], 'sources': sources, 'assets': [], 'unknowns': unknowns,
            'collectionNotes': ['目录来源的 generated_at 为 2026-06-26；抓取时间不代表商品发布日期或桌面端最新目录。',
                                '本轮按用户要求先补齐商品列表；原先六款继续作为硬件、模型与固件适配的优先队列。',
                                '商品图片未热链；未把预装软件名称、系列关系或 Spark 固件标签当作运行兼容证据。']}


def build():
    receipt = read(RECEIPT_PATH)
    raw_projection = SOURCE_PATH.read_bytes()
    assert len(raw_projection) == receipt['projection']['bytes']
    assert hashlib.sha256(raw_projection).hexdigest() == receipt['projection']['sha256']
    inventory = json.loads(raw_projection)
    source = source_record(receipt)
    stamp = max([source['fetchedAt']] + [r['fetchedAt'] for r in receipt['identitySources']])
    revision = source['revision']['commit']
    catalog_path = PLATFORM / 'catalog/products.json'
    index_path = PLATFORM / 'knowledge/index.json'
    catalog, index = read(catalog_path), read(index_path)
    items = {p['id']: p for p in catalog['items']}
    output, audit, seen = {}, [], set()
    for group in inventory['groups']:
        for row in group['products']:
            pid = row['product_id']
            assert pid not in seen, f'Duplicate Spark product_id: {pid}'
            seen.add(pid)
            status, reason = classify(group, row)
            record = {'groupId': group['id'], 'productId': pid, 'name': row['name'], 'mcu': row['mcu'], 'status': status}
            if status != 'included':
                record['reason'] = reason
                audit.append(record)
                continue
            local_id = EXISTING.get(pid, 'lilygo-' + pid)
            record['catalogId'] = local_id
            record['mappingScope'] = 'hardware-variant' if pid in EXISTING else 'product'
            if pid in EDITIONS:
                record['firmwareEdition'] = EDITIONS[pid]
            audit.append(record)
            provider = mapping(group, row, revision)
            if local_id in items:
                product = items[local_id]
                current = product.get('providers', {}).get('lilygo-spark')
                assert not current or current['productId'] == pid, f'Conflicting provider identity: {local_id}'
            else:
                # Keep the full MCU declaration: multi-controller products must not
                # acquire a guessed primary MCU. No runtime exists for these rows.
                product = {'id': local_id, 'name': 'LILYGO ' + row['name'], 'manufacturer': 'LILYGO', 'sku': row['name'],
                           'hardwareRevision': 'unknown; Spark product listing', 'chip': row['mcu'], 'module': row['mcu'],
                           'summary': f"Spark 将此商品归入{family(group)['name']}，标注 MCU 为 {row['mcu']}；具体硬件版次及配置待核对。",
                           'knowledgeUrl': f'./knowledge/{local_id}.json',
                           'runtime': {'board': None, 'flashMb': None, 'psramMb': None, 'status': 'unavailable'},
                           'display': None, 'displayStatus': 'unknown', 'controls': [], 'interfaces': [],
                           'sources': [{'title': 'LILYGO Spark 固定提交产品目录', 'url': source['url'], 'checkedAt': source['fetchedAt'][:10]}],
                           'limitations': ['仅完成商品目录收录；屏幕、存储、接口、尺寸和硬件版次待核对，尚未完成适配。',
                                           '同系列、相似名称与预装固件版本均不代表硬件或固件可互换。']}
                if pid == 'watch-keyboard-c3-v1-0':
                    product['summary'] = '带 ESP32-C3 的可编程键盘商品；按设备商品收录，未建立与手表的附件连接或运行适配。'
                if '+' in row['mcu']:
                    product['limitations'].append('目录包含多个控制器；主从分工及各芯片配置待核对，未合并为单一模拟主控。')
                if pid in EDITIONS:
                    product['summary'] = f"{row['name']} 是 Spark 单独列出的 {EDITIONS[pid]} 预装商品版；具体硬件与固件版本待核对。"
                    product['firmwareEdition'] = EDITIONS[pid]
                catalog['items'].append(product)
                items[local_id] = product
            product['family'] = family(group)
            product.setdefault('providers', {})['lilygo-spark'] = provider
            knowledge_path = PLATFORM / 'knowledge' / f'{local_id}.json'
            if knowledge_path.exists():
                knowledge = read(knowledge_path)
                knowledge.setdefault('identity', {})['family'] = product['family']
                knowledge['identity'].setdefault('providers', {})['lilygo-spark'] = provider
                knowledge['sources'] = [s for s in knowledge['sources'] if s['id'] != source['id']] + [source]
                fact = knowledge_fact('spark-product-identity', 'Spark 商品与系列标识', provider,
                                      '本地条目保留已核验硬件变体；Spark 商品入口可能覆盖其他配置，不扩大固件适用范围。' if pid in EXISTING else '商品标识不是硬件兼容性证明。')
                knowledge['facts'] = [f for f in knowledge['facts'] if f['id'] != fact['id']] + [fact]
                knowledge['updatedAt'] = max(knowledge['updatedAt'], stamp)
            else:
                knowledge = generate_record(product, row, provider, source, stamp)
            output[knowledge_path] = serial(knowledge)
            indexed = next((e for e in index['entries'] if e['id'] == local_id), None)
            if indexed:
                indexed['updatedAt'] = knowledge['updatedAt']
            else:
                index['entries'].append({'id': local_id, 'kind': 'product', 'catalogId': local_id, 'name': product['name'], 'url': f'./{local_id}.json', 'updatedAt': knowledge['updatedAt']})
    index['updatedAt'] = max(index['updatedAt'], stamp)
    catalog['revision'] = '2026-10-28-lilygo-spark-products'
    output[catalog_path], output[index_path] = serial(catalog), serial(index)
    output[AUDIT_PATH] = serial({'schemaVersion': '1', 'sourceReceipt': RECEIPT_PATH.name, 'manifestGeneratedAt': inventory['manifestGeneratedAt'],
                                'counts': {'groups': len(inventory['groups']), 'sourceProducts': len(audit), 'included': sum(r['status'] == 'included' for r in audit), 'excluded': sum(r['status'] != 'included' for r in audit)},
                                'products': audit})
    return output


def verify_source(path):
    receipt = read(RECEIPT_PATH)
    raw = Path(path).read_bytes()
    assert len(raw) == receipt['source']['bytes'], 'Original manifest byte count changed'
    assert hashlib.sha256(raw).hexdigest() == receipt['source']['sha256'], 'Original manifest SHA-256 changed'
    manifest = json.loads(raw)
    projection = {'schemaVersion': '1', 'manifestGeneratedAt': manifest['generated_at'], 'groups': [
        {'id': g['id'], 'name': g['name'], 'products': [{k: p[k] for k in ['product_id', 'name', 'mcu', 'github_repo', 'product_page']} for p in g['products']]}
        for g in manifest['product_list']]}
    assert projection == read(SOURCE_PATH), 'Product projection differs from the pinned source'
    print('Original manifest bytes/hash and exact product projection verified.')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true', help='Check deterministic output without writing')
    parser.add_argument('--verify-source', metavar='RAW_MANIFEST', help='Verify the downloaded original against its receipt and projection')
    args = parser.parse_args()
    if args.verify_source:
        verify_source(args.verify_source)
    changed = []
    for path, data in build().items():
        if not path.exists() or path.read_text() != data:
            changed.append(str(path.relative_to(ROOT)))
            if not args.check:
                path.write_text(data)
    if args.check and changed:
        raise SystemExit('Stale generated metadata:\n' + '\n'.join(changed))
    print(f'LILYGO Spark catalog: {len(changed)} files {"would change" if args.check else "updated"}.')


if __name__ == '__main__':
    main()
