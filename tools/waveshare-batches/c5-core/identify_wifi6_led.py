#!/usr/bin/env python3
"""Identify the existing native CAD RGB package without altering its geometry."""
import json
from pathlib import Path

root = Path(__file__).resolve().parents[3]
path = root / 'web/platform/assets/waveshare-esp32-c5-wifi6-kit/model-plan.json'
plan = json.loads(path.read_text())
part = next(p for p in plan['parts'] if p.get('sourceComponent') == 275)
part.update(name='rgb-window', role='led', ledIndex=0, color='#d4d8ce',
            basis='Native STEP component 275 is the small package between the two USB connectors. Its location agrees with the unchanged dimension-photo RGB silkscreen; the exact schematic connects LED2 to GPIO27. No component is relocated, stretched, or added.')
plan['validation']['ledIndices'] = [0]
plan.setdefault('componentIdentifications', {})['rgb-window'] = {
    'sourceComponent': 275,
    'sourceIds': ['official-cad', 'dimension-photo'],
    'photoRegionPixels': [220, 201, 252, 237],
    'basis': 'RGB-marked package between the USB sockets on the exact official dimension photograph.',
    'runtimeIndex': 0,
    'opticalCalibrationVerified': False,
}
path.write_text(json.dumps(plan, ensure_ascii=False, indent=2) + '\n')
