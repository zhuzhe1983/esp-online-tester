#!/usr/bin/env python3
"""Rebuild only the StackChan rear appearance, preserving accepted mesh bytes.

The fixed old model checkpoint and every old artifact are hash-checked. This
recipe does not rerun unstable CAD component splitting or claim new CAD fidelity.
"""
import argparse
import gzip
import hashlib
import json
from pathlib import Path

import cv2
import numpy as np
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT/'web/platform/assets/m5stack-k151'

def sha(data):
    return hashlib.sha256(data).hexdigest()

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--scratch', required=True, type=Path)
    args = parser.parse_args()
    out = args.scratch.resolve()/'m5stack-k151'
    if out.exists() or out.is_relative_to(ROOT/'web') or out.is_relative_to(ROOT/'docs'):
        raise ValueError('Use a fresh independent scratch directory')
    out.mkdir(parents=True)
    plan = json.loads((ASSETS/'model-plan.json').read_text())
    checkpoint = plan['appearanceBaseModel']
    data = (ASSETS/checkpoint['file']).read_bytes()
    assert sha(data) == checkpoint['sha256']
    baseline = gzip.decompress(data)
    assert sha(baseline) == checkpoint['expandedSha256']
    model = json.loads(baseline)
    for name, digest in model['artifacts'].items():
        assert Path(name).name == name
        content = (ASSETS/name).read_bytes()
        assert sha(content) == digest, name
        (out/name).write_bytes(content)
    assert model['artifacts']['geometry.bin'] == checkpoint['geometrySha256']
    (out/checkpoint['file']).write_bytes(data)
    model['artifacts'][checkpoint['file']] = sha(data)
    texture = next(t for t in plan['textureSources'] if t['file'] == 'pcb-power-back.png')
    source = next(s for s in model['provenance']['sources'] if s['id'] == texture['sourceId'])
    original = ASSETS/('reference-'+source['archiveName'])
    assert sha(original.read_bytes()) == source['sha256']
    image = Image.open(original).convert('RGBA')
    mask = Image.new('L', (image.width*4, image.height*4), 0)
    draw = ImageDraw.Draw(mask)
    for poly in texture['alphaMask']['includePolygons']:
        draw.polygon([(x*4, y*4) for x, y in poly], fill=255)
    for rect in texture['alphaMask'].get('excludeRects', []):
        draw.rectangle([v*4 for v in rect], fill=0)
    image.putalpha(mask.resize(image.size, Image.Resampling.LANCZOS))
    w, h = texture['rectifiedPixels']
    matrix = cv2.getPerspectiveTransform(np.asarray(texture['perspectiveQuad'], np.float32),
        np.asarray([[0, 0], [w-1, 0], [w-1, h-1], [0, h-1]], np.float32))
    result = Image.fromarray(cv2.warpPerspective(np.asarray(image), matrix, (w, h),
        flags=cv2.INTER_LANCZOS4, borderMode=cv2.BORDER_CONSTANT, borderValue=(0, 0, 0, 0)))
    result.save(out/texture['file'])
    model['artifacts'][texture['file']] = sha((out/texture['file']).read_bytes())
    model['textures'].append({**texture, 'sourceSha256': source['sha256'], 'outputPixels': [w, h],
        'perspectiveMatrix': matrix.tolist(), 'method': 'Source-coordinate4x alpha mask; exact photo rectified with OpenCV Lanczos4. No CAD retessellation or new component heights.'})
    for field in ['derivedParts', 'presentation', 'limitations']:
        model[field] = plan[field]
    model['appearanceProvenance'] = {'recipe': 'tools/rebuild-stackchan-rear-appearance.py',
        'baseModel': checkpoint, 'sourceId': source['id'], 'sourceSha256': source['sha256'],
        'scope': 'Masked visible board photo only; original native geometry and all preexisting paper/source artifacts retained byte-for-byte.'}
    data = (json.dumps(model, ensure_ascii=False, indent=2)+'\n').encode()
    (out/'model.json').write_bytes(data)
    expected = ASSETS/'model.json'
    receipt = {'productId': model['id'], 'modelSha256': sha(data), 'geometrySha256': checkpoint['geometrySha256'],
        'preservedArtifacts': len(json.loads(baseline)['artifacts']), 'modelByteIdentical': data == expected.read_bytes(),
        'newTextureSha256': model['artifacts'][texture['file']], 'newCadFidelityClaimed': False}
    (args.scratch/'receipt.json').write_text(json.dumps(receipt, indent=2)+'\n')
    print(json.dumps(receipt, indent=2))

if __name__ == '__main__':
    main()
