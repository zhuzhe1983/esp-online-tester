#!/usr/bin/env python3
"""Qualify the audited vendor Test.bin without inventing factory provenance."""
import json
from pathlib import Path

path = Path(__file__).with_name('esp32-c6-lcd-1.3.json')
data = json.loads(path.read_text())
note = ('官方Demo包内Test.bin已通过完整镜像校验，可按双SHA-256固定的zip-bundle路径加载；'
        '官方资料未明确其为出厂恢复固件，因此归官方测试示例trial。原程序在3.221客体秒触发BLE断言并复位，完整应用未通过。')
entry_note = ('Official test image is an exact member of the 97,520,767-byte, 6,092-entry Demo ZIP. '
              'The public zip-bundle source pins both archive and selected BIN hashes. No explicit factory-restoration purpose was found.')
old_entry = ('Official factory is an exact member of a 97,520,767-byte, 6,092-entry Demo ZIP. It is not a direct BIN URL. '
             'Browser catalogue mapping awaits bounded exact-member support; no incompatible flat-ZIP source is invented.')

def qualify(value):
    if isinstance(value, dict):
        return {key: qualify(item) for key, item in value.items()}
    if isinstance(value, list):
        return [qualify(item) for item in value]
    if isinstance(value, str):
        return value.replace(old_entry, entry_note).replace('Pinned factory source', 'Pinned official test source').replace(
            '原厂3.221秒在BLE断言处重启，1次异常、背光关闭；ZIP精确成员待前端有界加载支持，不冒充BIN直链。', note).replace('原厂', '官方测试')
    return value

data = qualify(data)
source = {'url': 'https://files.waveshare.com/wiki/ESP32-C6-LCD-1.3/ESP32-C6-LCD-1.3_Demo.zip',
          'sha256': 'bbed53686d23afee448ab8eb2c8c94246c120005222aadb1e527fb05a4fb4948', 'bootMode': 'rom',
          'archive': {'format': 'zip-bundle', 'entry': 'ESP32-C6-LCD-1.3_Demo/Firmwork/ESP32C6-1.3inch-Test.bin',
                      'sha256': 'fdc8f1eb3e89bd643a7b2921ce9364dc5d516d3c351c3fdef3b82c2de7f342f4'}}
firmware_id = 'waveshare-c6-lcd-13-official-test'
data['firmwares'] = [{'id': firmware_id, 'name': 'ESP32-C6-LCD-1.3 · 官方测试示例（验证中）',
    'productIds': [p['id'] for p in data['products']], 'version': 'ESP32C6-1.3inch-Test',
    'role': 'example', 'status': 'trial', 'publisher': 'Waveshare', 'publisherGroup': '官方',
    'source': source, 'simulationVerified': False, 'hardwareValidated': False,
    'sources': [{'title': 'Original official Demo ZIP, exact Test.bin member', 'url': source['url'], 'checkedAt': '2026-11-23'}],
    'limitations': [note]}]
for product in data['products']:
    product['defaultFirmwareId'] = firmware_id
    if note not in product['limitations']: product['limitations'].append(note)
for knowledge in data['knowledge']:
    if note not in knowledge['collectionNotes']: knowledge['collectionNotes'].append(note)
    for fact in knowledge['facts']:
        if fact['id'] == 'factory-layout':
            fact['id'] = 'original-image-layout'
            fact['value'].update(publicTrialAssigned=True, catalogRole='example', factoryPurposeVerified=False, source=source)
        elif fact['id'] == 'ecosystem-investigation':
            fact['value']['factory'] = 'No explicit factory restoration purpose found; audited official Test.bin is an example trial.'
for watch in data['watchlist']:
    watch['metadataProvider'] = 'manual'
    watch['monitoringNote'] = 'Official static Demo ZIP; no release API is established. Review the exact member and both hashes manually.'
    watch['category'] = 'development'
    watch['name'] = 'ESP32-C6-LCD-1.3 official test image'
    watch['compatibility']['note'] = note + ' Future upstream changes still require independent approval.'
path.write_text(json.dumps(data, ensure_ascii=False, indent=2) + '\n')
