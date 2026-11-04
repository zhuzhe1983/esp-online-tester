"""Run from repository root with tools/requirements-model.txt installed."""
import hashlib
import json
from pathlib import Path

import numpy as np
from PIL import Image

asset = Path('web/platform/assets/m5stack-k141')
meta = json.loads((asset / 'model.json').read_text())
import sys
paper = next(p for p in meta['derivedParts'] if p['id'] == sys.argv[1])
image = np.array(Image.open(asset / paper['texture']).convert('RGBA'))
height, width = image.shape[:2]
size_x, size_y = paper['geometry']['sizeMm']
cx, cy, plane = paper['geometry']['centerMm']
heights = np.full((height, width), -np.inf)
raw = (asset / meta['binary']).read_bytes()
for part in meta['parts']:
    if part['name'] not in ['shell-front']:
        continue
    triangles = np.frombuffer(raw, dtype='<f4', count=part['vertexCount'] * 6,
                              offset=part['byteOffset']).reshape(-1, 3, 6)[:, :, :3]
    for tri in triangles:
        p = np.column_stack(((tri[:, 0] - cx + size_x / 2) / size_x * width - .5,
                             (cy + size_y / 2 - tri[:, 1]) / size_y * height - .5))
        x0, x1 = max(0, int(np.floor(p[:, 0].min()))), min(width - 1, int(np.ceil(p[:, 0].max())))
        y0, y1 = max(0, int(np.floor(p[:, 1].min()))), min(height - 1, int(np.ceil(p[:, 1].max())))
        if x1 < x0 or y1 < y0:
            continue
        a, b, c = p
        den = (b[1] - c[1]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[1] - c[1])
        if abs(den) < 1e-12:
            continue
        yy, xx = np.mgrid[y0:y1 + 1, x0:x1 + 1]
        u = ((b[1] - c[1]) * (xx - c[0]) + (c[0] - b[0]) * (yy - c[1])) / den
        v = ((c[1] - a[1]) * (xx - c[0]) + (a[0] - c[0]) * (yy - c[1])) / den
        w = 1 - u - v
        z = u * tri[0, 2] + v * tri[1, 2] + w * tri[2, 2]
        patch = heights[y0:y1 + 1, x0:x1 + 1]
        np.maximum(patch, np.where((u >= -1e-8) & (v >= -1e-8) & (w >= -1e-8), z, -np.inf), out=patch)
active = image[:, :, 3] >= 128
z = heights[active]
receipt = {
    'productId': meta['id'], 'modelSha256': hashlib.sha256((asset / 'model.json').read_bytes()).hexdigest(),
    'paperId': paper['id'], 'method': 'Pixel-centre vertical ray raster of native shell-front triangles, alpha>=128; physical image registration from final plan.',
    'printedPixels': int(active.sum()), 'paperPlaneMm': plane,
    'intersectionsAbovePaper': int((z > plane + .002).sum()),
    'supportAtSeatPixels': int((np.abs(z - 10.2) < .002).sum()),
    'lowerOrAbsentSupportPixels': int((z < 10.198).sum()),
    'limitations': ['Actual native front-shell triangles checked; label registration and0.015mm visual offset are not manufacturing metrology.'],
}
print(json.dumps(receipt, indent=2))
assert receipt['intersectionsAbovePaper'] == 0

assert receipt['lowerOrAbsentSupportPixels'] == 0
