#!/usr/bin/env python3
"""Bind the photographed GPIO27 RGB package on each exact WROxx variant."""
import json
from pathlib import Path

root = Path(__file__).resolve().parents[3]
for path in sorted((root / 'web/platform/assets').glob('waveshare-esp32-devkit-*/model-plan.json')):
    plan = json.loads(path.read_text())
    registration = plan['photoRegistration']['pcb-main']
    left, top, right, bottom = registration['pcbPixelBounds']
    width, height = registration['nominalSizeMm']
    # RGB silkscreen and white package are visible immediately behind USB.
    # Exact variant photos independently show the same carrier location.
    cx, cy = 333, 315
    x = (cx-left)/(right-left)*width-width/2
    y = height/2-(cy-top)/(bottom-top)*height
    surface = next(p for p in plan['parts'] if p['name'] == 'pcb-main-front-photo')['geometry']['zMm']
    part = {'name': 'rgb-window', 'role': 'led', 'ledIndex': 0, 'color': '#d7ddd1',
            'geometry': {'shape': 'box', 'sizeMm': [1.6, 1.8, .6], 'centerMm': [x, y, surface+.3]},
            'basis': 'RGB-marked carrier package in unchanged official dimension and exact-variant photographs; XY is photo registered. The 1.6x1.8x0.6mm package envelope is an explicit unmeasured rendering estimate, not native CAD.'}
    plan['parts'] = [p for p in plan['parts'] if p['name'] != 'rgb-window'] + [part]
    plan['validation']['ledIndices'] = [0]
    plan['limitations'] = [s for s in plan['limitations'] if not s.startswith('No RGB spatial package binding')]
    note = 'GPIO27 RGB index0 is mapped to the source-visible single carrier LED. Package dimensions/height and optical brightness remain estimated; no enclosure or cable mating acceptance.'
    if note not in plan['limitations']:
        plan['limitations'].append(note)
    plan['photoRegistration']['rgb'] = {'sourceIds': ['dimension-photo', 'exact-variant-photo'],
                                         'centerPixels': [cx, cy], 'xyUncertaintyMm': .2,
                                         'heightMmEstimate': .6, 'runtimeIndex': 0}
    plan['photoRegistration']['review'] = {
        'sourceId': 'dimension-photo', 'file': 'dimension-photo.webp', 'side': 'front',
        'pixelFromModelXY': [[(right-left)/width, 0, (left+right)/2], [0, -(bottom-top)/height, (top+bottom)/2]],
        'includeParts': ['pcb-main', 'boot-button', 'rgb-window'],
    }
    path.write_text(json.dumps(plan, ensure_ascii=False, indent=2) + '\n')
