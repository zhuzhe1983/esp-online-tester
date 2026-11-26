#!/usr/bin/env python3
"""Curate the independently rendered K148 views without modifying model pixels."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
from PIL import Image, ImageDraw, ImageFont

p = argparse.ArgumentParser()
p.add_argument('--repo', type=Path, required=True)
p.add_argument('--renders-root', type=Path, required=True)
p.add_argument('--out', type=Path, required=True)
p.add_argument('--inspection', action='store_true')
a = p.parse_args()
repo, src, out = a.repo.resolve(), a.renders_root.resolve(), a.out.resolve()
out.mkdir(parents=True, exist_ok=True)
digest = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
cell = 350
font = ImageFont.truetype('/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf', 17)
small = ImageFont.truetype('/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf', 13)
photo_name = 'web/platform/assets/m5stack-c148/reference-product-photo-c8fad993ac.webp'
photo = repo / photo_name
assert digest(photo) == '351ad341b054e3b0d9da2649d89e73d5f62989adbb52fe194cbc2ae5cefb2400'
rows = []
for appearance in ['original', 'gold', 'purple']:
    receipt = json.loads((src / appearance / 'render-receipt.json').read_text())
    row = receipt['products'][0]
    assert row['id'] == 'm5stack-k148' and row['appearance'] == appearance
    image = Image.new('RGB', (cell * 4, cell + 50), '#edf0f4')
    draw = ImageDraw.Draw(image)
    draw.text((10, 7), 'K148 PowerHub Kit | ' + appearance + ' | battery-uninstalled shared host | independent production-mesh CPU render', font=font, fill='#142738')
    for i, view in enumerate(row['views']):
        source = src / appearance / view['file']
        assert digest(source) == view['sha256']
        tile = Image.open(source).convert('RGB').resize((cell, cell), Image.Resampling.LANCZOS)
        image.paste(tile, (cell * i, 50))
        draw.text((cell * i + 10, 31), 'CPU ' + view['view'], font=small, fill='#142738')
    reference = Image.open(photo).convert('RGB')
    reference.thumbnail((cell, cell), Image.Resampling.LANCZOS)
    image.paste(reference, (cell * 3 + (cell - reference.width) // 2, 50 + (cell - reference.height) // 2))
    draw.text((cell * 3 + 10, 31), 'Official rear; separate source photograph', font=small, fill='#142738')
    target = out / (appearance + '-contact.webp')
    image.save(target, format='WEBP', quality=76, method=6)
    assert target.stat().st_size < 50000
    row['rawImageRetention'] = 'Original PNG views are retained outside Git in the reproducible render output; their hashes below identify the exact inputs to the committed contact sheet.'
    (out / (appearance + '-render.json')).write_text(json.dumps(receipt, indent=2) + '\n')
    rows.append({'appearance': appearance, 'file': target.name, 'sha256': digest(target), 'bytes': target.stat().st_size,
                 'size': [cell * 4, cell + 50],
                 'encoding': 'WebP quality 76 method 6; four 350px cells; CPU views LANCZOS-downsampled from 450px; no other pixel edits',
                 'rawViews': [{'view': v['view'], 'file': v['file'], 'sha256': v['sha256'], 'bytes': (src / appearance / v['file']).stat().st_size} for v in row['views']]})
shutil.copyfile(src / 'export/export-receipt.json', out / 'export-receipt.json')
(out / 'images.json').write_text(json.dumps({
    'productId': 'm5stack-k148',
    'scope': 'Independent K148 production instance CPU render, front/back/oblique per appearance. The fourth cell is the separate retained official rear photograph, not model pixels or a pose-calibrated overlay.',
    'referencePhoto': {'file': photo_name, 'sha256': digest(photo), 'sourceId': 'product-photo-c8fad993ac'},
    'contactSheets': rows}, indent=2) + '\n')
if a.inspection:
    receipt = json.loads((src / 'inspection/render-receipt.json').read_text())
    row = receipt['products'][0]
    assert row['id'] == 'm5stack-k148' and row['appearance'] == 'original'
    view = next(v for v in row['views'] if v['view'] == 'oblique')
    assert view['cameraEye'] == [2.0, 3.0, 1.0]
    source = src / 'inspection' / view['file']
    assert digest(source) == view['sha256']
    target = out / 'inspection-oblique.webp'
    image = Image.open(source).convert('RGB')
    for quality in [76, 70, 64, 58]:
        image.save(target, format='WEBP', quality=quality, method=6)
        if target.stat().st_size < 50000:
            break
    assert target.stat().st_size < 50000
    (out / 'inspection-render.json').write_text(json.dumps(receipt, indent=2) + '\n')
    shutil.copyfile(src / 'inspection-export/export-receipt.json', out / 'inspection-export-receipt.json')
    (out / 'inspection-image.json').write_text(json.dumps({
        'productId': 'm5stack-k148', 'inspectionProgress': 1,
        'scope': 'Full production assembly in public inspection mode; no parts manually hidden. Original colors; UNKNOWN remains on its supported inward board face.',
        'file': target.name, 'sha256': digest(target), 'bytes': target.stat().st_size,
        'size': list(image.size), 'encoding': {'format': 'WebP', 'quality': quality, 'method': 6, 'resizing': False},
        'rawView': {**view, 'bytes': source.stat().st_size}}, indent=2) + '\n')
print('K148 original/gold/purple contact sheets and exact image hashes retained')
