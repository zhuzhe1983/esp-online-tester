#!/usr/bin/env python3
"""Merge reviewed LilyGO evidence/profiles without inferring sibling hardware.

The source audit is a dated snapshot, not a generator of runtime capabilities.
Only explicit reviewed profile files may enable a board. Missing data stays null.
"""
import copy
import json
import re
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RESEARCH = ROOT / 'docs/platform/research'
WEB = ROOT / 'web/platform'


def read(path):
    return json.loads(path.read_text())


def write(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')


def receipt(source):
    result = copy.deepcopy(source)
    result.setdefault('title', result['id'].replace('-', ' '))
    result.setdefault('kind', 'official-product-document')
    if result['status'] == 'fetched':
        result['fetchedAt'] = datetime.fromisoformat(result['fetchedAt'].replace('Z', '+00:00')).astimezone(timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ')
        if 'archiveName' not in result:
            if result.get('file'):
                result['archiveName'] = result['id'] + Path(result['file']).suffix
            elif result.get('path'):
                result['archiveName'] = result['path'].replace('/', '__')
            elif result['id'].startswith('official-store-'):
                result['archiveName'] = result['id'][len('official-store-'):] + '.raw'
            else:
                result['archiveName'] = result['id'] + '.raw'
        if not re.fullmatch(r'[a-zA-Z0-9][a-zA-Z0-9._-]*', result['archiveName']):
            original = result['archiveName']
            if original.startswith('target/'):
                result['sourceCachePath'] = original
                result['archiveName'] = re.sub(r'[^a-zA-Z0-9._-]', '_', Path(original).name)
            else:
                result['archiveName'] = re.sub(r'[^a-zA-Z0-9._-]', '_', result['id'] + (Path(result.get('path', '')).suffix or '.raw'))
    return result


def upsert(items, value):
    items[:] = [item for item in items if item['id'] != value['id']]
    items.append(value)


def c5_profiles():
    records = read(RESEARCH / 'lilygo-c5-sources-2026-11-22.json')['items']
    factory = {item['productId']: item for item in read(RESEARCH / 'lilygo-c5-factory-investigation-2026-11-22.json')['items']}
    common = ['仅限 ESP32-C5 rev 1.0 SRAM app-direct 裸机程序；ROM、Flash/MMU 和普通 ESP-IDF 出厂固件启动未实现。',
              '无线、USB 数据、SD 卡传输、APA102、PMIC、PSRAM 和电池模拟未实现；未作实机验证。']
    result = []
    for record in records:
        dongle = record['productId'] == 'lilygo-t-dongle-c5'
        result.append({
            **record, 'boardId': record['productId'], 'chip': 'esp32c5',
            'module': 'ESP32-C5HR8' if dongle else 'ESP32-C5-MINI-1-N4',
            'flashMB': 16 if dongle else 4, 'psramMB': 0, 'physicalPsramMB': 8 if dongle else 0,
            'hardwareRevision': 'Official T-Dongle-C5 pin_config.h; PCB revision unspecified' if dongle else 'T7-C5 V1.1 BOOT source profile; V1.0 manufacturer CAD',
            'display': {'width': 80, 'height': 160, 'controller': 'ST7735', 'type': 'LCD', 'interface': 'SPI', 'touchImplemented': False, 'physicalTouch': False} if dongle else None,
            'pins': {'display': {'sclk': 6, 'mosi': 2, 'cs': 10, 'dc': 3, 'reset': 1,
                'backlight': {'pin': 0, 'activeHigh': False}}} if dongle else {},
            'sdCard': {'slot': dongle, 'implemented': False, 'pins': {'sclk': 6, 'mosi': 2, 'miso': 7, 'cs': 23} if dongle else {}},
            'controls': [{'id': 'boot', 'name': 'BOOT / GPIO28', 'type': 'button', 'pin': 28, 'activeLow': True}],
            'scope': '真实 SRAM 指令驱动 GPIO 软件 SPI，ST7735 80×160、复位、低电平背光和 BOOT 输入。' if dongle else '真实 SRAM 指令执行、UART0 和 GPIO28 BOOT 输入；本板无内置屏幕或用户 LED。',
            'unsupported': common + (['硬件 SPI 控制器到屏幕的传输尚未实现；当前屏幕调试需使用 GPIO 软件 SPI。',
                '屏幕当前实现 RGB565 像素；18 位像素与可视反色尚未实现，COLMOD / INVON 寄存器记录不代表这些显示模式已模拟。'] if dongle else []),
            'sourceIds': [source['id'] for source in record['sources']],
            'factoryFirmwareInvestigation': {'status': 'official-image-candidate-inspected',
                'sourceIds': [source['id'] for source in record['sources']], 'artifact': factory[record['productId']],
                'binaryFetched': True, 'binaryRetained': False, 'simulationVerified': False,
                'notes': '已在内存读取官方仓库镜像并记录哈希；完整 C5 出厂镜像不属于 SRAM 调试加载范围，不宣称能够运行。'},
        })
    return result


def main():
    audit = read(RESEARCH / 'lilygo-adaptation-sources-2026-11-22.json')
    audited = {item['id']: item for item in audit['items']}
    sources = {item['id']: receipt(item) for item in audit['sources']}
    profiles = c5_profiles()
    blockers = {}
    for path in sorted(RESEARCH.glob('lilygo-*-profiles-2026-11-22.json')):
        document = read(path)
        for source in document.get('sources', []):
            sources[source['id']] = receipt(source)
        profiles += document['items']
        for item in document.get('deferred', []):
            blockers[item['productId']] = item
    for path in sorted(RESEARCH.glob('lilygo-*-blockers-2026-11-22.json')):
        document = read(path)
        for source in document.get('sources', []):
            sources[source['id']] = receipt(source)
        for item in document['items']:
            blockers[item['productId']] = item
    if len({item['productId'] for item in profiles}) != len(profiles):
        raise ValueError('A product must have one explicitly selected runtime profile')
    profiles = {item['productId']: item for item in profiles}
    chips = {item['boardId']: item['chip'] for item in profiles.values()}
    capabilities = {}
    for item in profiles.values():
        board = item['boardId']
        keyboard = item.get('keyboard')
        matrix = [control for control in item['controls'] if control['type'] == 'matrix-key']
        if keyboard is None and matrix and board.startswith('lilygo-t-deck'):
            keyboard = {'rows': 4, 'columns': 10,
                'model': 'tca8418' if 'pro' in board else 'esp32c3-i2c-raw', 'layout': board}
        capabilities[board] = {'sdCard': item.get('sdCard', {}).get('implemented') is True,
            'display': item.get('display') is not None and item['display'].get('implemented', True),
            'boardButtons': [control['id'] for control in item['controls'] if control.get('transport') == 'board'],
            **({'lora': {'model': item['radio']['controller'].lower() + '-lora-digital'}} if item.get('radio', {}).get('implemented') is True else {}),
            **({'keyboard': keyboard} if keyboard else {})}
    if any(chip not in ['esp32', 'esp32s2', 'esp32s3', 'esp32c3', 'esp32c5', 'esp32c6', 'esp32p4'] for chip in chips.values()):
        raise ValueError('Unsupported simulated chip in reviewed LilyGO board profile')
    (WEB / 'lilygo-board-families.js').write_text('// Generated by tools/apply-lilygo-adaptations.py from reviewed board profiles.\n'
        'export const LILYGO_BOARD_CHIPS=Object.freeze(Object.assign(Object.create(null),'
        + json.dumps(dict(sorted(chips.items())), indent=2) + '));\n'
        'export const LILYGO_BOARD_CAPABILITIES=Object.freeze(Object.assign(Object.create(null),'
        + json.dumps(dict(sorted(capabilities.items())), indent=2) + '));\n')
    model_path = RESEARCH / 'lilygo-model-catalog-patches-2026-11-22.json'
    models = read(model_path) if model_path.exists() else {'items': []}
    for source in models.get('sources', []):
        sources[source['id']] = receipt(source)
    model_by_id = {item['id']: item for item in models['items']}
    catalog = read(WEB / 'catalog/products.json')
    index = read(WEB / 'knowledge/index.json')
    now = datetime.now(timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ')
    changed = set()
    for product in catalog['items']:
        if product['id'] not in audited:
            continue
        item = audited[product['id']]
        profile = profiles.get(product['id'])
        blocker = blockers.get(product['id'])
        chronology = profile.get('chronology', item['chronology']) if profile else item['chronology']
        product['chronology'] = chronology
        primary_url = item.get('documentationUrl') or item['productUrl']
        if not any(source['url'] == primary_url for source in product['sources']):
            product['sources'].insert(0, {'title': 'LILYGO 官方设备文档', 'url': primary_url, 'checkedAt': '2026-11-22'})
        knowledge_path = WEB / 'knowledge' / (product['id'] + '.json')
        knowledge = read(knowledge_path)
        for source_id in item['sourceIds']:
            upsert(knowledge['sources'], sources[source_id])
        if profile:
            for source in profile.get('sources', []):
                sources[source['id']] = receipt(source)
            for source_id in profile['sourceIds']:
                upsert(knowledge['sources'], sources[source_id])
        if blocker:
            for source_id in blocker.get('sourceIds', []):
                upsert(knowledge['sources'], sources[source_id])
            if profile:
                knowledge['sourceCorrections'] = {**blocker, 'resolvedByBoard': profile['boardId']}
                knowledge.pop('adaptationBlockers', None)
            else:
                knowledge['adaptationBlockers'] = blocker
            if not profile:
                product['chip'] = blocker.get('correctedChip', blocker.get('physicalChip', product['chip']))
                if 'standaloneCpu' in blocker and blocker['standaloneCpu'] is None:
                    product['module'] = blocker['correctedChip'] + ' / external host expansion'
                product['summary'] = blocker.get('scope', '模型预览；固件执行需要解决知识页中记录的主控或接线缺口。')
                product['limitations'] = [product['summary'], *blocker.get('blockers', [])]
                knowledge['runtimeSupport'] = {'status': 'unavailable', 'board': None,
                    'scope': product['summary'], 'blockers': blocker.get('blockers', [])}
        if chronology['date']:
            upsert(knowledge['facts'], {'id': 'adaptation-chronology', 'label': '排序日期及依据',
                'value': chronology, 'basis': 'official', 'sourceIds': [chronology['sourceId']],
                'notes': '官方硬件版次日期优先；上架时间只是代用排序依据，不宣称零售首发日期。'})
        if knowledge.get('factoryFirmwareInvestigation', {}).get('status') != 'image-pinned':
            knowledge['factoryFirmwareInvestigation'] = profile.get('factoryFirmwareInvestigation', item['factoryFirmwareInvestigation']) if profile else (blocker or {}).get('factoryFirmwareInvestigation', item['factoryFirmwareInvestigation'])
        model = model_by_id.get(product['id'])
        if model:
            product['model'] = model['model']
            product['model']['mode'] = model['mode']
            knowledge['modelReview'] = {key: value for key, value in model.items() if key != 'id'}
            model_metadata = read(WEB / model['model']['url'].removeprefix('./'))
            for source in model_metadata.get('provenance', {}).get('sources', []):
                upsert(knowledge['sources'], receipt(sources.get(source['id'], source)))
            source_by_id = {source['id']: source for source in knowledge['sources']}
            for photo in model_metadata.get('referencePhotos', []):
                source = source_by_id[photo['sourceId']]
                asset = {'sourceId': source['id'], 'url': source['url'], 'kind': 'manufacturer-photo',
                    'localAsset': './assets/' + product['id'] + '/' + photo['file']}
                knowledge['assets'] = [value for value in knowledge['assets'] if value['sourceId'] != source['id']]
                knowledge['assets'].append(asset)
            for source in model.get('sources', []):
                upsert(knowledge['sources'], receipt(source))
            for source_id in model.get('sourceIds', []):
                if source_id in sources:
                    upsert(knowledge['sources'], sources[source_id])
            for unknown in knowledge['unknowns']:
                if unknown['id'] == 'mechanical':
                    unknown['notes'] = '已建立可检查的模型；来源、估算和缺失结构见 modelReview。调试占位的尺寸不代表实物尺寸。'
        if profile:
            product['runtime'] = {'board': profile['boardId'], 'flashMb': profile['flashMB'],
                'psramMb': profile['psramMB'], 'status': 'diagnostic'}
            if profile.get('flashMbBasis') or profile.get('physicalFlashMB', profile['flashMB']) != profile['flashMB']:
                product['runtime']['flashMbBasis'] = profile.get('flashMbBasis', 'simulation')
            if profile.get('physicalPsramMB', profile['psramMB']) != profile['psramMB']:
                product['runtime']['psramMbBasis'] = 'simulation'
            product['chip'] = profile.get('physicalChip', profile['chip'])
            if profile.get('module'):
                product['module'] = profile['module']
            if profile.get('hardwareRevision'):
                product['hardwareRevision'] = profile['hardwareRevision']
            display = profile.get('display') or (profile.get('physicalDisplay') if isinstance(profile.get('physicalDisplay'), dict) else None)
            product['display'] = None if display is None else {
                'width': display['width'], 'height': display['height'], 'type': display.get('type', 'LCD'),
                'controller': display['controller'], 'bus': display['interface']}
            if display:
                product['runtime']['touch'] = display.get('touchImplemented', False)
                for key in ['debugLayout', 'physicalPanels']:
                    if key in display:
                        product['display'][key] = copy.deepcopy(display[key])
            product['displayStatus'] = 'known' if display else 'none'
            product['controls'] = copy.deepcopy(profile['controls'])
            if any(control['type'] == 'encoder' for control in product['controls']):
                product['runtime']['encoder'] = True
            for control in product['controls']:
                if control['type'] == 'encoder' and control.get('detents') is None:
                    control.update(detents=24, detentsBasis='simulation',
                        notes='调试旋钮每圈采用 24 步；实物机械档位数尚未核实。')
            if profile.get('sdCard', {}).get('slot'):
                sd = profile['sdCard']
                upsert(product['interfaces'], {'id': 'micro-sd', 'name': 'microSD / TF',
                    'type': 'microSD', 'external': True,
                    'pins': [{'name': name.upper(), 'gpio': pin} for name, pin in sd.get('pins', {}).items() if isinstance(pin, int)],
                    'sourceIds': sd.get('sourceIds', profile['sourceIds']),
                    'notes': '默认准备 64 MiB FAT32 卡；' + ((sd.get('transport', 'SPI').upper() + ' 传输已实现。') if sd.get('implemented') else '当前未实现该板的卡传输。')})
            product['summary'] = profile['scope']
            previous_model_notes = [value for value in product.get('limitations', []) if '模型' in value
                and '仅完成商品目录' not in value and '尚无经过验证的板级固件执行适配' not in value
                and '需要新增 ESP32-S2 芯片内核' not in value]
            product['limitations'] = list(dict.fromkeys([profile['scope'], *profile['unsupported'], *previous_model_notes]))
            knowledge['runtimeSupport'] = {'board': profile['boardId'], 'status': 'diagnostic',
                'scope': profile['scope'], 'unsupported': profile['unsupported'],
                'firmwareCompatibility': 'No factory image is claimed working by a board protocol test.'}
            upsert(knowledge['facts'], {'id': 'adapter-hardware', 'label': '本次适配核对的硬件',
                'value': {'display': display, 'flashMb': profile.get('physicalFlashMB', profile['flashMB']),
                    'simulatedFlashMb': profile['flashMB'], 'flashMbBasis': profile.get('flashMbBasis', 'verified-profile'),
                    'physicalPsramMb': profile.get('physicalPsramMB', profile['psramMB']),
                    'controls': profile['controls'], 'pins': profile.get('pins', {})},
                'basis': 'official', 'sourceIds': profile['sourceIds'],
                'notes': '物理硬件与已实现模拟能力分开记录；出厂固件运行不由引脚或协议单测推导。'})
            knowledge['adapterProfile'] = {key: copy.deepcopy(value) for key, value in profile.items() if key != 'sources'}
            if profile.get('sourceConflicts') or profile.get('memoryEvidence'):
                upsert(knowledge['facts'], {'id': 'adapter-source-conflicts', 'label': '版本冲突与容量依据',
                    'value': {'sourceConflicts': profile.get('sourceConflicts', []), 'memoryEvidence': profile.get('memoryEvidence')},
                    'basis': 'official', 'sourceIds': profile['sourceIds'],
                    'notes': '保留官方资料之间的冲突；所选调试配置不自动适用于其他硬件版次。'})
            knowledge['unknowns'] = [value for value in knowledge['unknowns'] if value['id'] not in ['memory', 'display']]
            if profile.get('physicalFlashMB', profile['flashMB']) is None or profile.get('physicalPsramMB', profile['psramMB']) is None:
                knowledge['unknowns'].append({'id': 'memory', 'label': '实体存储容量仍需核对',
                    'value': None, 'basis': 'unknown', 'sourceIds': profile['sourceIds'],
                    'notes': '部分实体 Flash / PSRAM 容量未确认；运行时字段是所选调试或编译配置，PSRAM 为 0 不表示实板没有该器件。冲突、已知候选值和容量依据见本型号适配来源记录。'})
            for unknown in knowledge['unknowns']:
                if unknown['id'] == 'runtime':
                    unknown['notes'] = '板级协议和调试程序的实现范围见 runtimeSupport；完整出厂固件、未实现外设与实机结果仍未验证。'
                if unknown['id'] == 'inputs-interfaces':
                    unknown['notes'] = '已核实的按键与接线见 adapter-hardware；其余接口与电气细节仍需按具体硬件确认。'
                if unknown['id'] == 'factory-firmware':
                    unknown['notes'] = '逐款检索结果见 factoryFirmwareInvestigation；发现官方入口或候选镜像不等于已完成硬件版次、启动和模拟兼容验证。'
        elif model:
            display = (blocker or {}).get('displayEvidence') or item.get('displayEvidence')
            complete_display = display and all(display.get(key) is not None for key in ['type', 'controller', 'width', 'height', 'interface', 'sourceIds'])
            if complete_display:
                product['display'] = {key: display[key] for key in ['type', 'controller', 'width', 'height']}
                product['display']['bus'] = display['interface']
                product['displayStatus'] = 'known'
                upsert(knowledge['facts'], {'id': 'adaptation-display', 'label': '所述硬件选项的屏幕资料',
                    'value': display, 'basis': 'official', 'sourceIds': display['sourceIds'],
                    'notes': '仅限 representedHardware 所描述的屏幕；推导链与置信度保留，不推定其余引脚、容量或硬件版次。'})
                knowledge['unknowns'] = [value for value in knowledge['unknowns'] if value['id'] != 'display']
            product['summary'] = '已补充设备调试预览；' + ('官方资料中的屏幕参数仍需按具体硬件选项核对。' if product.get('displayStatus') == 'unknown' else '板级运行支持范围见限制说明。')
            product['limitations'] = ['已提供可查看的 3D 调试模型；模型来源、估算和缺失项在模型检查面板及知识资料中单独说明。',
                '该型号尚无经过验证的板级固件执行适配；模型可查看不代表出厂固件已能运行。',
                '同系列、相似名称和预装软件版本不代表硬件或固件可互换。']
            if display and display.get('representedHardware'):
                product['limitations'].append(display['representedHardware'])
        if blocker and not profile:
            product['summary'] = blocker.get('scope', '模型预览；固件执行需要解决知识页中记录的主控或接线缺口。')
            product['limitations'] = list(dict.fromkeys([product['summary'], *blocker.get('blockers', []), *product.get('limitations', [])]))
            if blocker.get('physicalDisplay') is False or blocker.get('displayEvidence', {}).get('present') is False:
                product['display'] = None
                product['displayStatus'] = 'none'
            for unknown in knowledge['unknowns']:
                if unknown['id'] == 'runtime':
                    unknown['notes'] = product['summary']
        knowledge['summary'] = product['summary']
        knowledge['hardwareRevision'] = product['hardwareRevision']
        knowledge['updatedAt'] = now
        knowledge['collectionNotes'] = [note for note in knowledge['collectionNotes'] if '本轮按用户要求先补齐商品列表' not in note]
        note = '2026-11-22 按官方版次日期优先、官方上架日期代用的顺序补充适配；日期不明的项目排在末尾。'
        if note not in knowledge['collectionNotes']:
            knowledge['collectionNotes'].append(note)
        write(knowledge_path, knowledge)
        changed.add(product['id'])
    for entry in index['entries']:
        if entry['id'] in changed:
            entry['updatedAt'] = now
    index['updatedAt'] = now
    if catalog.get('revision', '')[:10] <= '2026-11-22':
        catalog['revision'] = '2026-11-22-lilygo-adaptation'
    write(WEB / 'catalog/products.json', catalog)
    write(WEB / 'knowledge/index.json', index)
    print(f'Updated {len(changed)} chronologies, {len(profiles)} explicit runtime profiles and {len(model_by_id)} model previews.')


if __name__ == '__main__':
    main()
