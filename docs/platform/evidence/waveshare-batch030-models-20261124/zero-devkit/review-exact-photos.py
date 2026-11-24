#!/usr/bin/env python3
"""Project new production-loader XY footprints onto unchanged exact-SKU photos."""
from pathlib import Path
import argparse, hashlib, json, textwrap
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
import numpy as np
from PIL import Image
from scipy.spatial import ConvexHull, QhullError

ROOT = Path(__file__).resolve().parents[5]
ap = argparse.ArgumentParser()
ap.add_argument('--meshes', type=Path, required=True)
ap.add_argument('--output', type=Path, default=Path(__file__).parent / 'reviews')
a = ap.parse_args()
a.output.mkdir(parents=True, exist_ok=True)
records = []
for suffix in ['zero-n8r8', 'zero-n8r8-m', 'dev-kit-n16r8', 'dev-kit-n16r8-m',
               'dev-kit-n32r16v', 'dev-kit-n32r16v-m', 'dev-kit-n16r8-u', 'dev-kit-n16r8-um']:
    mid = 'waveshare-esp32-s3-' + suffix
    d = ROOT / 'web/platform/assets' / mid
    plan = json.loads((d / 'model-plan.json').read_text())
    src = next(s for s in plan['sources'] if s['id'] == 'exact-variant-photo')
    raw_photo = (d / src['file']).read_bytes()
    assert hashlib.sha256(raw_photo).hexdigest() == src['sha256']
    reg = plan['photoRegistration']['exactVariant']
    matrix = np.asarray(reg['homogeneousPixelFromModelXY'])
    pixels = np.asarray(reg['sourcePixels'])
    im = Image.open(d / src['file'])
    rec = json.loads((a.meshes / (mid + '.json')).read_text())
    note = reg['basis'] + ' ' + reg['scope']
    if suffix.endswith(('-u', '-um')):
        note += ' Coax and antenna remain reference-only. The source-bounded notch is separately ray-checked.'
    fig, axes = plt.subplots(1, 2, figsize=(12, 8))
    for ax in axes:
        ax.imshow(im)
        ax.axis('off')
    axes[0].set_title('Unchanged official exact-SKU photograph')
    axes[1].set_title('Actual new loader XY footprints / manual registration')
    projected = []
    for mesh in rec['meshes']:
        if mesh.get('unknown') or mesh['name'].startswith('header-'):
            continue
        pts = np.unique(np.asarray(mesh['position']).reshape(-1, 3)[:, :2], axis=0)
        if len(pts) < 3:
            continue
        try:
            points = pts[ConvexHull(pts).vertices]
        except QhullError:
            continue
        pixel = np.column_stack([points, np.ones(len(points))]) @ matrix.T
        pixel = pixel[:, :2] / pixel[:, 2:3]
        pixel = np.vstack([pixel, pixel[0]])
        axes[1].plot(pixel[:, 0], pixel[:, 1], linewidth=.85, alpha=.9)
        projected.append(mesh['name'])
    axes[1].scatter(*pixels.T, s=14, color='#d53e4f')
    fig.suptitle(mid, fontsize=11, y=.99)
    fig.text(.5, .95, textwrap.fill(note, 128), ha='center', va='top', fontsize=8)
    fig.tight_layout(rect=(0, 0, 1, .80))
    name = mid + '-exact-photo-overlay.png'
    fig.savefig(a.output / name, dpi=120)
    plt.close(fig)
    records.append({'model': mid,
                    'modelSha256': hashlib.sha256((d / 'model.json').read_bytes()).hexdigest(),
                    'planSha256': hashlib.sha256((d / 'model-plan.json').read_bytes()).hexdigest(),
                    'sourceId': src['id'], 'sourceSha256': src['sha256'],
                    'modelPointsMm': reg['modelPointsMm'], 'sourcePixels': reg['sourcePixels'],
                    'homogeneousPixelFromModelXY': matrix.tolist(),
                    'projectedParts': projected, 'overlay': name, 'notes': note,
                    'scope': 'New model hash and unchanged source pixels. Manual planar review only, not depth/material/fit or firmware acceptance.'})
(a.output / 'exact-photo-registration.json').write_text(json.dumps({'records': records}, indent=2) + '\n')
print(f'{len(records)} new exact-option overlays written.')
