#!/usr/bin/env python3
"""Attach bounded original-image observations without changing hardware facts."""
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
EVIDENCE = ROOT / 'docs/platform/evidence/waveshare-batch011-20261124'
observations = {
    'esp32-s3-rlcd-4.2': ('rlcd42-original', '末帧300×400单色；SHTC3唤醒、复位和ID读取失败，RF校准告警保留。'),
    'esp32-s3-touch-amoled-1.32': ('amoled132-original', '原程序产生95帧，466×466末帧16色；尚未验收触摸交互、音频和完整应用。'),
    'esp32-s3-touch-lcd-1.83': ('lcd183-original', '240×284末帧单色；物理TF座对应64MiB卡仅准备、未挂载，未虚报已支持未知SD引脚。'),
    'esp32-s3-touch-lcd-3.49': ('lcd349-original', '172×640末帧单色；QMI复位/自检失败和RF校准告警保留。该镜像仍是明确的official-demo，非默认factory。'),
    'esp32-s3-epaper-1.54': ('epaper154-active-low', '按原源码纠正GPIO6屏幕、GPIO42音频低有效门控后，原图越过ES8311初始化并启动LVGL，200×200末帧2色，64MiB FAT32卡真实修改4页；SHTC3缺失仍报错。修复前无帧及音频abort证据另存epaper154-original。'),
}

def save(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')

for listing, (folder, description) in observations.items():
    path = Path(__file__).with_name(listing + '.json')
    fragment = json.loads(path.read_text())
    result = json.loads((EVIDENCE / folder / 'result.json').read_text())
    firmware = next(f for f in fragment['firmwares'] if f['id'] == result['firmwareId'])
    assert firmware['source']['sha256'] == result['firmwareSha256']
    assert result['productId'] in firmware['productIds']
    assert result['status'] == 'bounded-attempt-completed'
    note = (f"2026-11-23公开Worker/生产WASM原镜像尝试：代表配置{result['productId']}运行"
            f"{result['stats']['time']}客体秒。{description}"
            f"记录见docs/platform/evidence/waveshare-batch011-20261124/{folder}。"
            '仅记录所选配置的有界原图执行，不代表同家族所有SKU/硬件版本、浏览器WebGL或实物整机验收。')
    for value in [fragment, firmware, *fragment['products']]:
        values = value.setdefault('limitations', [])
        if note not in values:
            values.append(note)
    for entry in fragment['knowledge']:
        knowledge_path = ROOT / entry['path']
        record = json.loads(knowledge_path.read_text())
        if note not in record['collectionNotes']:
            record['collectionNotes'].append(note)
        save(knowledge_path, record)
    save(path, fragment)
