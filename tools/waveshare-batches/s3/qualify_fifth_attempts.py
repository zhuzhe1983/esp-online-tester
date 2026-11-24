#!/usr/bin/env python3
"""Retain exact public Worker observations, including failed or incomplete apps."""
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
EVIDENCE = ROOT / 'docs/platform/evidence/waveshare-batch013-20261124'
OBSERVATIONS = {
    'esp32-s3-dualeye-touch-lcd-1.28': ('dualeye-worker',
        '240×240末帧单色、0显示帧；两个独立GRAM/触摸总线已建模，当前公开显示API只输出主屏。'
        '64MiB FAT32卡挂载，未观察到更改页；不等于已完成原厂双眼应用。'),
    'esp32-s3-lcd-3.16': ('lcd316-worker',
        '原始214,706,718字节ZIP经生产选择器解包后，320×820末帧单色、0显示帧；'
        '64MiB FAT32卡挂载，未观察到更改页。'),
    'esp32-s3-knob-touch-lcd-1.8': ('knob18-worker',
        '360×360末帧25色、78显示帧；人工检查可见时钟/低电量内容，但画面倒置且偏暗，未验收方向和亮度。'
        '64MiB FAT32卡挂载，未观察到更改页；ZIP内第二颗经典ESP32固件未共同执行。'),
}


def save(path, record):
    path.write_text(json.dumps(record, ensure_ascii=False, indent=2) + '\n')


for listing, (folder, description) in OBSERVATIONS.items():
    path = Path(__file__).with_name(listing + '.json')
    fragment = json.loads(path.read_text())
    result = json.loads((EVIDENCE / folder / 'result.json').read_text())
    firmware = next(f for f in fragment['firmwares'] if f['id'] == result['firmwareId'])
    assert firmware['source']['sha256'] == result['firmwareSha256']
    assert result['productId'] in firmware['productIds']
    assert result['status'] == 'bounded-attempt-completed'
    note = (f"公开Worker/生产WASM原镜像有界记录：{result['productId']}运行{result['stats']['time']}客体秒。"
            + description + f'证据：docs/platform/evidence/waveshare-batch013-20261124/{folder}。'
            '不推定其他SKU、完整应用、浏览器WebGL、实物、音频或RF通过。')
    for item in [fragment, firmware, *fragment['products']]:
        if note not in item.setdefault('limitations', []):
            item['limitations'].append(note)
    for entry in fragment['knowledge']:
        knowledge_path = ROOT / entry['path']
        record = json.loads(knowledge_path.read_text())
        if note not in record['collectionNotes']:
            record['collectionNotes'].append(note)
        save(knowledge_path, record)
    save(path, fragment)
