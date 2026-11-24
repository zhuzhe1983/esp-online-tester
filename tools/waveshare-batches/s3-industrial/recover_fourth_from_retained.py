#!/usr/bin/env python3
"""Recreate final core-board metadata from retained public sources and model plans.

Unlike the historical first-pass curator, this entry point needs no private
download cache or unpublished base catalog. It preserves published products,
original firmware manifests, historical evidence and all Rust/Cargo inputs.
"""
import argparse
import copy
import datetime as dt
import gzip
import hashlib
import json
from pathlib import Path
import re
from urllib.parse import urlparse

ROOT = Path(__file__).resolve().parents[3]
PLATFORM = ROOT / 'web/platform'
RESEARCH = ROOT / 'docs/research/waveshare-s3-industrial-2026-11-24'
BASE = '61447a576c36f40adcd40810fd9180f137937bdc'
CONFIGS = [
    ('esp32-s3-nano', 'ESP32-S3-Nano', ['nano', 'nano-m'], [(16, 8)] * 2),
    ('esp32-s3-zero', 'ESP32-S3-Zero', ['zero', 'zero-m', 'zero-n8r8', 'zero-n8r8-m'], [(4, 2), (4, 2), (8, 8), (8, 8)]),
    ('esp32-s3-dev-kit-n8r8', 'ESP32-S3-DEV-KIT-N8R8', ['dev-kit-n8r8', 'dev-kit-n8r8-m', 'dev-kit-n16r8', 'dev-kit-n16r8-m', 'dev-kit-n32r16v', 'dev-kit-n32r16v-m', 'dev-kit-n16r8-u', 'dev-kit-n16r8-um'], [(8, 8), (8, 8), (16, 8), (16, 8), (32, 16), (32, 16), (16, 8), (16, 8)]),
    ('esp32-s3-pico', 'ESP32-S3-Pico', ['pico', 'pico-m'], [(16, 2)] * 2),
]
COMMON = [
    '适配范围为精确SKU容量、GPIO/LEDC/RMT数字输出及已声明的BOOT输入；不据此宣称完整原厂应用通过。',
    'USB桥接/集线器、射频、模拟电压、电流、热特性与实机刷写未验收。RESET使用平台重启操作，不伪装成普通GPIO按键。',
    '排针、内存与天线选项分别建档；原四基础款的原BIN执行证据只属于原型号和原镜像，不向新增SKU继承。',
]
FACTORY_NOTE = '精确SKU的官方产品、资源及开发资料、原始程序ZIP、固定M5Burner公开分类与UIFlow/小智发布记录中，未确认独立出厂恢复镜像。官方示例和MicroPython维持example/development身份。'


def read(path):
    return json.loads(path.read_text())


def sha(data):
    return hashlib.sha256(data).hexdigest()


def write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')


def utc(value):
    return dt.datetime.fromisoformat(value.replace('Z', '+00:00')).astimezone(dt.timezone.utc).isoformat(timespec='seconds').replace('+00:00', 'Z')


def fact(identifier, label, value, source_ids, basis='official', **extra):
    return dict(id=identifier, label=label, value=value, sourceIds=source_ids, basis=basis, **extra)


def knowledge_source(record, identifier=None, title=None):
    """Normalize provenance into the strict public knowledge schema."""
    url = record.get('url') or record.get('requestedUrl') or record.get('originalUrl')
    timestamp = record.get('fetchedAt') or record.get('fetchedAtUTC') or record.get('containerFetchedAt')
    assert url and timestamp and record['bytes'] > 0 and re.fullmatch('[a-f0-9]{64}', record['sha256'])
    archive_name = record.get('archiveName') or record.get('file') or Path(urlparse(url).path).name or 'source.json'
    archive_name = re.sub('[^a-zA-Z0-9._-]', '_', archive_name)
    notes = record.get('notes', '')
    if record.get('archive'):
        archive = record['archive']
        notes += ' Original archive member ' + str(archive.get('entry', '')) + '; container SHA-256 ' + str(archive.get('containerSha256', archive.get('sha256', ''))) + '. Member and HTTP identities remain distinct.'
    result = dict(id=identifier or record['id'], title=title or record.get('title') or record['id'],
                  url=url, resolvedUrl=record.get('resolvedUrl') or record.get('finalUrl') or url,
                  kind=record.get('kind', 'manufacturer-document'), status='fetched',
                  fetchedAt=utc(timestamp), bytes=record['bytes'], sha256=record['sha256'],
                  archiveName=archive_name, mediaType=record.get('mediaType') or record.get('contentType') or 'application/octet-stream')
    if notes:
        result['notes'] = notes.strip()
    return result


def retained_sources(listing, checks):
    records = read(RESEARCH / listing / 'sources.json')['sources']
    for record in records:
        retained = record.get('retained')
        if not retained:
            continue
        path = ROOT / retained['path']
        body = path.read_bytes()
        assert len(body) == retained['bytes'] and sha(body) == retained['sha256'], path
        decoded = gzip.decompress(body) if retained.get('encoding') == 'gzip' else body
        assert sha(decoded) == retained['uncompressedSha256'], path
        checks.append(dict(path=retained['path'], sha256=sha(body), bytes=len(body), uncompressedSha256=sha(decoded), transform=retained.get('transform')))
    return records


def model_inputs(product_id, checks):
    folder = PLATFORM / 'assets' / product_id
    plan = read(folder / 'model-plan.json')
    model = read(folder / 'model.json')
    assert plan['id'] == product_id and model['id'] == product_id
    sources = []
    for record in plan['sources']:
        data = (folder / record['file']).read_bytes()
        retained = record.get('retained', record)
        assert sha(data) == retained['sha256'] and len(data) == retained['bytes'], (product_id, record['id'])
        decoded = gzip.decompress(data) if record.get('retained', {}).get('encoding') == 'gzip' else data
        assert sha(decoded) == record['sha256'] and len(decoded) == record['bytes'], (product_id, record['id'])
        source = knowledge_source(record, 'model-' + record['id'], 'Mechanical source: ' + record.get('title', record['id']))
        source['kind'] = 'manufacturer-cad' if record.get('role') == 'cad-source' else 'official-visual-reference'
        source['notes'] = (source.get('notes', '') + ' Original retained under assets/' + product_id + '/' + record['file'] + '. ' + record.get('rights', '')).strip()
        sources.append(source)
    checks.append(dict(productId=product_id, modelSha256=sha((folder / 'model.json').read_bytes()), planSha256=sha((folder / 'model-plan.json').read_bytes()), method=plan['method'], sources=len(sources)))
    return plan, sources


def ecosystem_sources():
    directory = ROOT / 'docs/research/waveshare-p4-s31-2026-11-24/ecosystem-releases'
    manifest = read(directory / 'source-manifest.json')['sources']
    sources, releases = [], {}
    for kind, filename in [('uiflow', 'm5stack-uiflow-micropython.json.gz'), ('xiaozhi', '78-xiaozhi-esp32.json.gz')]:
        record = next(row for row in manifest if row['archiveName'] == filename)
        body = gzip.decompress((directory / filename).read_bytes())
        assert sha(body) == record['sha256'] and len(body) == record['bytes']
        releases[kind] = json.loads(body)
        sources.append(knowledge_source(record, kind + '-release', kind + ' pinned upstream release'))
    directory = ROOT / 'docs/research/waveshare-classic-s2-2026-11-24/ecosystem'
    record = read(directory / 'source-manifest.json')['sources'][0]
    body = gzip.decompress((directory / 'm5burner-devices.json.gz').read_bytes())
    assert sha(body) == record['sha256'] and len(body) == record['bytes']
    data = json.loads(body)['data']
    assert len(data) == 55 and not any('waveshare' in json.dumps(row).lower() for row in data)
    sources.append(knowledge_source(record, 'm5burner-devices', 'Pinned M5Burner public device categories; not every community upload'))
    return sources, releases


def validate_output(fragments, knowledge_records):
    from jsonschema import Draft202012Validator
    from referencing import Registry, Resource
    directory = PLATFORM / 'schemas'
    schemas = {path.name: read(path) for path in directory.glob('*.json')}
    registry = Registry().with_resources((name, Resource.from_contents(value)) for name, value in schemas.items())
    validators = {key: Draft202012Validator({'$ref': reference}, registry=registry) for key, reference in [
        ('products', 'catalog.schema.json#/$defs/product'),
        ('firmwares', 'catalog.schema.json#/$defs/firmware'),
        ('watchlist', 'firmware-watchlist.schema.json#/properties/entries/items'),
    ]}
    knowledge_validator = Draft202012Validator(schemas['knowledge.schema.json'], registry=registry)
    seen = {key: set() for key in validators}
    skus = set()
    for fragment in fragments:
        for key, validator in validators.items():
            for record in fragment[key]:
                validator.validate(record)
                assert record['id'] not in seen[key]
                seen[key].add(record['id'])
        products = {record['id']: record for record in fragment['products']}
        firmware = {record['id']: record for record in fragment['firmwares']}
        for product in products.values():
            assert product['sku'] not in skus
            skus.add(product['sku'])
            assert product['id'] in firmware[product['defaultFirmwareId']]['productIds']
            assert product['runtime']['status'] == 'partial'
        for record in firmware.values():
            assert set(record['productIds']) <= set(products)
            assert record['simulationVerified'] is False and record['hardwareValidated'] is False
    for _, record in knowledge_records:
        knowledge_validator.validate(record)
        source_ids = {source['id'] for source in record['sources']}
        assert len(source_ids) == len(record['sources'])
        for item in [*record['facts'], *record['unknowns']]:
            assert set(item['sourceIds']) <= source_ids
    assert len(skus) == len(seen['products']) == 16
    assert len(seen['firmwares']) == len(seen['watchlist']) == 25
    return dict(products=16, knowledge=16, firmwares=25, watchlist=25, status='passed',
                schemaSha256={name: sha((directory / name).read_bytes()) for name in ['catalog.schema.json', 'knowledge.schema.json', 'firmware-watchlist.schema.json']})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output-root', type=Path, default=ROOT)
    parser.add_argument('--integrate', action='store_true')
    parser.add_argument('--receipt', type=Path, required=True)
    args = parser.parse_args()
    output = args.output_root.resolve()
    now = dt.datetime.now(dt.timezone.utc).isoformat(timespec='seconds').replace('+00:00', 'Z')
    catalog_paths = ['catalog/products.json', 'catalog/firmwares.json', 'catalog/firmware-watchlist.json', 'knowledge/index.json']
    before = {path: read(PLATFORM / path) for path in catalog_paths}
    frozen = {path: sha((ROOT / path).read_bytes()) for path in ['esp-soc/src/waveshare_profiles.rs', 'web/platform/waveshare-board-families.js']}
    capabilities = json.loads((PLATFORM / 'waveshare-board-families.js').read_text().split('Object.create(null),', 1)[1].rsplit('));', 1)[0])
    source_checks, model_checks, all_fragments, all_knowledge = [], [], [], []
    eco, releases = ecosystem_sources()
    for listing, document, suffixes, memories in CONFIGS:
        identity = read(RESEARCH / listing / 'storefront-identity.json')
        variants = identity['variants']
        assert len(variants) == len(suffixes)
        records = retained_sources(listing, source_checks)
        sources = [knowledge_source(record) for record in records]
        ids = ['waveshare-esp32-s3-' + suffix for suffix in suffixes]
        family_id = 'waveshare-' + listing
        factory_id = family_id + '-factory-reference'
        firmware_entries, watchers = [], []
        defaults = {}

        def add_firmware(identifier, name, role, product_ids, plan_filename=None, release=None, note=FACTORY_NOTE):
            publication = 'https://docs.waveshare.com/' + document + '/Resources-And-Documents'
            source, version = None, 'source-review'
            publisher, group, upstream = 'Waveshare', '官方', 'manufacturer:Waveshare/' + document
            if plan_filename:
                path = PLATFORM / 'firmware-plans' / plan_filename
                plan = read(path)
                board = plan['board']
                assert board in capabilities and plan['flash_mb'] == capabilities[board]['flashMb'] and plan['psram_mb'] == capabilities[board]['psramMb']
                assert plan['files'] == {} and plan['flash_at']
                source = {'manifestUrl': './firmware-plans/' + plan_filename, 'sha256': sha(path.read_bytes())}
                version = plan['version']
            if release:
                item = releases[release]
                version, publication = item['tag_name'], item['html_url']
                publisher = 'M5Stack' if release == 'uiflow' else '78/xiaozhi-esp32'
                group = '开源项目'
                upstream = 'm5stack/uiflow-micropython' if release == 'uiflow' else '78/xiaozhi-esp32'
            watch_id = identifier + '-watch'
            firmware_entries.append(dict(id=identifier, name=name, productIds=product_ids, version=version,
                status='trial' if source else 'reference', source=source, role=role, publisher=publisher,
                publisherGroup=group, simulationVerified=False, hardwareValidated=False, storage='remote-links-only',
                compatibility='partial' if source else 'unavailable', publicationUrl=publication, watchlistId=watch_id,
                sources=[dict(title='Fixed primary source', url=publication, checkedAt=now[:10])], limitations=[note, *COMMON]))
            watchers.append(dict(id=watch_id, name=name, category='factory' if role == 'factory' else 'voice' if release == 'xiaozhi' else 'development',
                metadataProvider='manual', upstreamRepo=upstream, metadataUrl=publication, releaseUrl=publication,
                version=version, checkedAt=now, assets=[], compatibility=dict(status='reference-only', productIds=product_ids,
                simulationVerified=False, hardwareValidated=False, note=note), storage='remote-links-only', catalogTargets=[],
                monitoringNote='Manual exact SKU/source mapping. No automatic cross-capacity, PSRAM or flash-mode substitution; no inherited execution acceptance.'))

        add_firmware(factory_id, document + ' · 出厂程序调查资料', 'factory', ids)
        for suffix in suffixes:
            product_id = 'waveshare-esp32-s3-' + suffix
            ending = 'official-test' if suffix.startswith('nano') else 'blinkrgb' if suffix in ['zero', 'zero-m'] else 'micropython'
            filename = product_id + '-' + ending + '.json'
            if (PLATFORM / 'firmware-plans' / filename).is_file():
                role = 'example' if ending in ['official-test', 'blinkrgb'] else 'development'
                identifier = filename.removesuffix('.json')
                note = ('原始官方示例或开发镜像；保持已提交清单、原ZIP成员、偏移和hash。仅原基础款有历史原BIN执行记录；新增排针/内存/天线SKU不继承执行结论。')
                add_firmware(identifier, product_id + ' · 官方原始' + ('示例' if role == 'example' else 'MicroPython') + '（试运行）', role, [product_id], filename, note=note)
                defaults[product_id] = identifier
        if listing == 'esp32-s3-zero':
            add_firmware(ids[0] + '-micropython', document + ' · 官方文档链接MicroPython（试运行）', 'development', [ids[0]], ids[0] + '-micropython.json', note='厂家文档链接的原始4MiB软件布局；只映射4F/2P基础Zero，不移植到N8R8选项。')
        for kind, label in [('uiflow', 'UIFlow'), ('xiaozhi', '小智')]:
            add_firmware(family_id + '-' + kind + '-reference', label + ' · ' + document + ' 精确映射调查', 'project', ids, release=kind,
                note='固定上游发布记录未找到这些精确SKU的唯一镜像映射；保留资料入口，不以近似设备名称替代。')

        products, knowledge_paths, caps = [], [], {}
        for index, (suffix, memory, variant) in enumerate(zip(suffixes, memories, variants)):
            product_id = ids[index]
            board = 'waveshare-s3-' + suffix.replace('dev-kit', 'devkit')
            cap = copy.deepcopy(capabilities[board])
            assert (cap['flashMb'], cap['psramMb']) == memory and cap['chip'] == 'esp32s3'
            caps[board] = cap
            nano, devkit, octal = suffix.startswith('nano'), suffix.startswith('dev-kit'), 'n32r16v' in suffix
            plan, model_sources = model_inputs(product_id, model_checks)
            model_ids = [row['id'] for row in model_sources]
            name = variant['documentedProductNames'][0].split(' (')[0]
            assert variant['sku'] == read(RESEARCH / 'batch-04/manifest.json')['items'][CONFIGS.index((listing, document, suffixes, memories))]['skus'][index]
            default = defaults.get(product_id, factory_id)
            notes = list(COMMON)
            if nano:
                notes.append('实际只有RESET；GPIO0是离散LED通道，不提供虚构BOOT。历史LED封装引脚号与ROHM手册的冲突保留；RGB命名依官方程序及固定Arduino Nano映射，实物光色未校准。')
            if suffix.startswith('pico'):
                notes.append('精确S3R2/2MiB身份与官方Octal-PSRAM段落有冲突；4MiB MicroPython软件布局不改变物理16MiB Flash。')
            if octal:
                notes.append('N32R16V为32MiB OPI Flash/16MiB PSRAM；GPIO47/48属于官方1.8V域。数字模拟不表示实际电压；尚无确认的精确可启动原厂镜像。')
            if suffix.startswith('zero-n8r8'):
                notes.append('独立PICO-1-N8R8为8F/8P；FH4R2的4F/2P镜像与内部CAD不替代此SiP型号。')
            notes.extend(plan.get('limitations', []))
            summary = f'{memory[0]} MiB Flash / {memory[1]} MiB PSRAM；' + ('离散RGB GPIO46/0/45、GPIO48指示灯，无物理BOOT。' if nano else ('OPI Flash；' if octal else '') + 'GPIO' + ('38' if devkit else '21') + ' RGB与GPIO0 BOOT。')
            controls = [] if nano else [dict(id='boot', name='BOOT', type='button', pin=0, activeLow=True, defaultKeyCode='Space')]
            interface_note = '官方USB-C供电/下载接口；USB总线、CH343/CH334桥接或集线器行为未由此条目模拟。' if devkit or suffix.startswith('pico') else '官方USB-C供电/下载接口；USB总线行为未模拟。'
            usb_pins = [dict(name='D-', gpio=19), dict(name='D+', gpio=20)] if nano or suffix in ['zero', 'zero-m'] else []
            product = dict(id=product_id, name='Waveshare ' + name, manufacturer='Waveshare', sku=variant['sku'], chip='esp32s3',
                hardwareRevision='unspecified', summary=summary, knowledgeUrl='./knowledge/' + product_id + '.json',
                family=dict(id=family_id, name='Waveshare ' + document.split('-N8R8')[0]),
                runtime=dict(board=board, flashMb=memory[0], psramMb=memory[1], status='partial', touch=False),
                displayStatus='none', display=None, controls=controls,
                interfaces=[dict(id='usb-c', name='USB-C · 电源/程序下载', type='USB-C', external=True, pins=usb_pins, sourceIds=['overview', 'schematic'], notes=interface_note + ' 已列GPIO仅表示对应基础板原理图电气网络；插头逐脚三维朝向未验收。')],
                sources=[dict(id=row['id'], title=row['title'], url=row['url'], checkedAt=row['fetchedAt'][:10]) for row in sources],
                limitations=[*notes, FACTORY_NOTE], chronology=dict(date=None, basis='unknown', sourceId=None, sourceUrl=None, confidence='unknown', notes='未确认产品发布日期；文档及固件日期不替代硬件发布日。'),
                defaultFirmwareId=default, model=dict(type='manufacturer-cad', url='./assets/' + product_id + '/model.json', status='partial', provenance=plan['method']))
            if index:
                product['visualVariantOf'] = family_id
            products.append(product)
            led = dict(kind='discrete-common-anode', gpioRGB=[46, 0, 45], activeLow=True, indicatorGpio=48, indicatorActiveHigh=True) if nano else dict(kind='addressable', gpio=38 if devkit else 21, wireOrder='RGB' if devkit or suffix.startswith('pico') else 'GRB', count=1)
            module = ('ESP32-S3-WROOM-2-N32R16V' if octal else 'ESP32-S3-WROOM-1U-N16R8' if suffix.endswith(('-u', '-um')) else 'ESP32-S3-WROOM-1-N16R8' if 'n16r8' in suffix else 'ESP32-S3-WROOM-1-N8R8') if devkit else ('ESP32-S3R8' if nano else 'ESP32-S3R2' if suffix.startswith('pico') else 'ESP32-S3-PICO-1-N8R8' if 'n8r8' in suffix else 'ESP32-S3FH4R2')
            facts = [
                fact('identity', '精确SKU与包装选项', dict(sku=variant['sku'], model=name, runtimeBoard=board, options=variant['options'], currentlyUnsaleable=variant['unsaleable']), ['storefront', 'overview']),
                fact('cpu', '主控架构', dict(chip='ESP32-S3', architecture='Xtensa LX7', cores=2, maxClockMHz=240), ['overview', 'storefront']),
                fact('module', '精确芯片或模块选项', module, ['overview', 'storefront']),
                fact('memory', '精确存储与Flash模式', dict(flashMiB=memory[0], psramMiB=memory[1], flashInterface='OPI' if octal else 'QSPI', gpio47and48NominalHighV=1.8 if octal else 3.3 if devkit else None), ['storefront', 'overview'], notes='软件分区、PSRAM协议与物理Flash容量分别记录。'),
                fact('buttons', '板上按键', dict(bootGpio=None if nano else 0, bootActiveLow=None if nano else True, reset='physical-reset; platform reset action'), ['overview', 'schematic']),
                fact('led', 'LED数字接线与线序', led, ['schematic', 'led-bsp'] + (['arduino-pins', 'led-datasheet'] if nano else []), notes='线序依据原始官方LED示例；数字亮度与光学校准不同。'),
                fact('display', '随板显示器', dict(included=False), ['overview', 'storefront']),
                fact('interfaces', '接口与实现边界', dict(usb='USB-C', usbHub='CH334' if devkit or suffix.startswith('pico') else None, usbUart='CH343' if devkit or suffix.startswith('pico') else None, serialAndGpioRouting='See retained exact schematic; no electrical cable mating claim'), ['overview', 'schematic']),
                fact('power', '供电资料', dict(usbNominalV=5, externalPadRangeV=[6, 21] if nano else [3.7, 6] if suffix in ['zero', 'zero-m', 'pico', 'pico-m'] else None), ['overview', 'schematic'], notes='Nano范围对应VIN，不是USB端口；基础Zero/Pico范围对应标5V外部焊盘。Zero-N8R8和DevKit其他输入范围未逐版本核实。N32的47/48特殊1.8V域在memory事实中单列；本模型未实现模拟电源。'),
                fact('radio', '物理无线能力', dict(wifiBandGHz=2.4, bluetooth='Bluetooth LE'), ['overview'], notes='硬件能力不表示射频/天线传播或无线兼容通过。'),
                fact('mechanical', '机械来源与精度边界', plan['dimensions'], model_ids, 'cad-measurement' if plan['method'].startswith('official-cad') else 'image-measurement', method='Unscaled exact native geometry where available; explicit photo/dimension envelopes elsewhere. Current model plans and new verification bind actual assets.', uncertainty='Partial geometry and estimates are not manufacturing or physical mating tolerances.'),
                fact('factory-investigation', '出厂与生态固件调查', dict(factoryImagePinned=False, defaultFirmwareId=default, originalExampleOrDevelopmentAvailable=product_id in defaults, baseExecutionEvidence='docs/evidence/waveshare-s3-core-2026-11-23' if index == 0 else None, newSkuInheritsBaseExecution=False), ['overview', 'resources', 'micropython-guide', 'm5burner-devices', 'uiflow-release', 'xiaozhi-release'], notes=FACTORY_NOTE),
            ]
            if devkit:
                facts.append(fact('reserved-pins', '精确模块禁用引脚', dict(GPIO35='NC', GPIO36='NC', GPIO37='NC', externalAntenna=suffix.endswith(('-u', '-um'))), ['overview']))
            unknowns = [fact('revision', '实物PCB修订未知', None, ['overview'], 'unknown'), fact('weight', '精确SKU重量未知', None, ['overview'], 'unknown'), fact('pin-mating', '扩展插口逐脚三维插合未验收', None, ['schematic'], 'unknown'), fact('mechanical-fit', '实物装配公差未知', None, model_ids, 'unknown'), fact('full-firmware', '完整原厂应用与实机运行', None, ['resources'], 'unknown')]
            knowledge = dict(schemaVersion='1', id=product_id, catalogId=product_id, kind='product', title=product['name'], summary=summary,
                summarySourceIds=['storefront', 'overview'], hardwareRevision='unspecified', updatedAt=now, facts=facts,
                sources=[*sources, *eco, *model_sources], revisions=[],
                assets=[dict(id='model', sourceId=model_ids[0], role='source-backed-partial-model', url=model_sources[0]['url'], localUrl='./assets/' + product_id + '/model.json')],
                unknowns=unknowns, collectionNotes=[FACTORY_NOTE, *notes])
            path = 'web/platform/knowledge/' + product_id + '.json'
            all_knowledge.append((path, knowledge))
            knowledge_paths.append({'path': path})
        fragment = dict(listingId=listing, products=products, firmwares=firmware_entries, watchlist=watchers, knowledge=knowledge_paths,
            boardCapabilities=caps, modelIds=ids,
            sourceReceiptPaths=['docs/research/waveshare-s3-industrial-2026-11-24/' + listing + '/' + name for name in ['sources.json', 'storefront-identity.json', 'archive-members.json', 'firmware-investigation.json', 'runtime-verification.json']],
            tests=[dict(command='cargo test -p esp32s3 --test waveshare_core_skus', status='passed', scope='Historical retained exact16 digital diagnostics; complete final workspace validation is separately reported.', evidence='docs/research/waveshare-s3-industrial-2026-11-24/batch-04/runtime-checks.json')],
            limitations=COMMON, integrationNotes=['Recovered from retained source identities and original firmware manifests; no private first-pass authoring cache required.', 'Published391 product objects and all Rust/Cargo sources remain unchanged. Model revisions bind their new evidence rather than inheriting historical image hashes.'])
        all_fragments.append(fragment)

    assert sum(len(row['products']) for row in all_fragments) == 16
    assert sum(len(row['firmwares']) for row in all_fragments) == 25
    assert sum(len(row['watchlist']) for row in all_fragments) == 25
    validation = validate_output(all_fragments, all_knowledge)
    for path, value in all_knowledge:
        write(output / path, value)
    for fragment in all_fragments:
        write(output / 'tools/waveshare-batches/s3-industrial' / (fragment['listingId'] + '.json'), fragment)

    def append_unique(existing, additions):
        seen = {row['id'] for row in existing}
        assert not seen.intersection(row['id'] for row in additions), 'Do not overwrite published records'
        return [*existing, *additions]

    totals = None
    if args.integrate:
        assert output == ROOT, '--integrate writes only this checkout'
        additions = {
            'catalog/products.json': ('items', [p for f in all_fragments for p in f['products']]),
            'catalog/firmwares.json': ('items', [p for f in all_fragments for p in f['firmwares']]),
            'catalog/firmware-watchlist.json': ('entries', [p for f in all_fragments for p in f['watchlist']]),
            'knowledge/index.json': ('entries', [dict(id=k['id'], kind='product', catalogId=k['catalogId'], url='./' + k['id'] + '.json', updatedAt=k['updatedAt']) for _, k in all_knowledge]),
        }
        guards = read(RESEARCH / 'batch-04-catalog-recovery/published-catalog-guards.json')
        assert guards['publishedCommit'] == BASE
        merged = {}
        for path in catalog_paths:
            guard = guards['files'][path]
            key, new_rows = additions[path]
            owned = {row['id'] for row in new_rows}
            retained = copy.deepcopy(before[path])
            retained[key] = [row for row in retained[key] if row['id'] not in owned]
            if path == 'knowledge/index.json':
                retained['updatedAt'] = guard['metadata']['updatedAt']
            canonical = json.dumps(retained, ensure_ascii=False, sort_keys=True, separators=(',', ':')).encode()
            assert sha(canonical) == guard['canonicalSha256'], ('Published catalog changed before recovery', path)
            merged[path] = retained
            merged[path][key] = append_unique(retained[key], new_rows)
        merged['knowledge/index.json']['updatedAt'] = now
        for path, value in merged.items():
            write(PLATFORM / path, value)
        totals = dict(products=len(merged['catalog/products.json']['items']), firmwares=len(merged['catalog/firmwares.json']['items']), watchlist=len(merged['catalog/firmware-watchlist.json']['entries']), knowledge=len(merged['knowledge/index.json']['entries']))
        assert totals == dict(products=676, firmwares=609, watchlist=441, knowledge=684), totals
    assert frozen == {path: sha((ROOT / path).read_bytes()) for path in frozen}, 'Runtime registries changed'
    receipt = dict(schemaVersion=1, checkedAt=now, inputPublishedCommit=BASE, families=4, products=16, firmwareEntries=25, watchEntries=25,
        sourceChecks=source_checks, modelChecks=model_checks, validation=validation, publishedRecordsPreserved=True, registryHashes=frozen, integrated=args.integrate,
        catalogTotals=totals, historicalEvidence='Unmodified source, firmware and original16 model-review records remain under batch-04. New model hashes require their independently retained new verification.',
        outputFiles=[dict(path=path, bytes=(output / path).stat().st_size, sha256=sha((output / path).read_bytes())) for path, _ in all_knowledge])
    write(args.receipt, receipt)
    print(json.dumps(dict(families=4, products=16, firmwares=25, watchlist=25, integrated=args.integrate, totals=totals)))


if __name__ == '__main__':
    main()
