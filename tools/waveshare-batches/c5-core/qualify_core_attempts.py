#!/usr/bin/env python3
"""Attach bounded original-image observations without promoting factory status."""
import json
from pathlib import Path

root = Path(__file__).resolve().parents[3]
evidence = root / 'docs/platform/evidence/waveshare-batch015-20261124'
cases = [
    ('tools/waveshare-batches/c5-core/esp32-c5-zero.json', 'c5-zero-original',
     '原始官方TestCode完整镜像SHA核验后，经公开运行时被C5_SRAM_REQUIRED明确拒绝；没有执行ROM或把诊断程序替换为原厂镜像。'),
    ('tools/waveshare-batches/p4-s31/esp32-p4-nano.json', 'p4-nano-original',
     '7寸与10.1寸两套未修改原厂完整镜像，经公开Worker各运行3.00客体秒；UART到达SPIFFS、codec控制初始化、LVGL adapter及MIPI DSI PHY Powered on，均未产生帧。DSI、以太网、协处理器射频和音频PCM尚未验收。'),
]
def save(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')
for relative, attempt, note in cases:
    path = root / relative
    fragment = json.loads(path.read_text())
    result = json.loads((evidence / attempt / 'result.json').read_text())
    expected = 'attempt-blocked' if attempt.startswith('c5') else 'bounded-attempt-completed'
    assert result['status'] == expected
    if attempt.startswith('c5'):
        assert result['lastError']['code'] == 'C5_SRAM_REQUIRED'
    else:
        other = json.loads((evidence / 'p4-nano-101-original/result.json').read_text())
        assert other['stats']['time'] >= 3 and other['stats']['frames'] == 0
    for value in [fragment, *fragment['products'], *fragment['firmwares']]:
        notes = value.setdefault('limitations', [])
        if note not in notes:
            notes.append(note)
    fragment.setdefault('tests', []).append({
        'kind': 'original-image-public-runtime', 'status': expected,
        'evidencePath': str((evidence / attempt / 'result.json').relative_to(root)),
        'scope': note, 'simulationVerified': False, 'hardwareValidated': False,
    })
    for row in fragment['knowledge']:
        kp = root / row['path']
        knowledge = json.loads(kp.read_text())
        notes = knowledge.setdefault('collectionNotes', [])
        assert isinstance(notes, list)
        if note not in notes:
            notes.append(note)
        save(kp, knowledge)
    save(path, fragment)
