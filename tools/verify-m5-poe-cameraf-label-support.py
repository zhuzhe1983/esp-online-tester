#!/usr/bin/env python3
"""Test every effective alpha texel against actual exported carrier triangles."""
import argparse
import datetime
import hashlib
import json
from pathlib import Path
import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parents[1]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def check(identity):
    folder = ROOT / 'web/platform/assets' / identity
    metadata = json.loads((folder / 'model.json').read_text())
    geometry = np.frombuffer((folder / metadata['binary']).read_bytes(), dtype='<f4').reshape(-1, 6)
    specifications = []
    for part in metadata['parts']:
        projection = part.get('texture')
        if projection:
            specifications.append((projection, ['display-cover-face'] if identity == 'm5stack-k115' else [part['name']]))
    if identity == 'm5stack-u138':
        paper = next(p for p in metadata['derivedParts'] if p['id'] == 'diagram-paper')
        specifications.append((dict(file=paper['texture'], side='back', boundsMm=[-13, -36, 13, 36],
                                    zPlanesMm=[paper['geometry']['centerMm'][2]]), ['shell-back', 'rj45-shield-back']))
    rows = []
    for projection, carrier_names in specifications:
        texture = folder / projection['file']
        image = np.asarray(Image.open(texture).convert('RGBA'))
        pixels = np.argwhere(image[:, :, 3] >= 128)
        height, width = image.shape[:2]
        u, v = (pixels[:, 1] + .5) / width, (pixels[:, 0] + .5) / height
        x0, y0, x1, y1 = projection['boundsMm']
        side = 1 if projection['side'] == 'front' else -1
        x = x0 + u * (x1 - x0) if side == 1 else x1 - u * (x1 - x0)
        xy = np.column_stack([x, y1 - v * (y1 - y0)])
        desired = projection['zPlanesMm'][0]
        covered = np.zeros(len(xy), dtype=bool)
        z_support = np.full(len(xy), -np.inf * side)
        for carrier_name in carrier_names:
            carrier = next(p for p in metadata['parts'] if p['name'] == carrier_name)
            start = carrier['byteOffset'] // 24
            values = geometry[start:start + carrier['vertexCount']].reshape(-1, 3, 6)
            triangles = values[:, :, :3]
            normals = values[:, :, 3:].mean(1)
            triangles = triangles[(normals[:, 2] * side > .995) & (abs(triangles[:, :, 2].mean(1) - desired) < .1)]
            for triangle in triangles:
                p0, p1, p2 = triangle
                low, high = triangle[:, :2].min(0) - 1e-7, triangle[:, :2].max(0) + 1e-7
                indices = np.flatnonzero((xy[:, 0] >= low[0]) & (xy[:, 0] <= high[0]) &
                                         (xy[:, 1] >= low[1]) & (xy[:, 1] <= high[1]))
                a, b = p1[:2] - p0[:2], p2[:2] - p0[:2]
                determinant = a[0] * b[1] - a[1] * b[0]
                if abs(determinant) < 1e-10:
                    continue
                delta = xy[indices] - p0[:2]
                s = (delta[:, 0] * b[1] - delta[:, 1] * b[0]) / determinant
                t = (a[0] * delta[:, 1] - a[1] * delta[:, 0]) / determinant
                inside = (s >= -1e-7) & (t >= -1e-7) & (s + t <= 1 + 1e-7)
                target = indices[inside]
                z = p0[2] + s[inside] * (p1[2] - p0[2]) + t[inside] * (p2[2] - p0[2])
                z_support[target] = np.maximum(z_support[target] * side, z * side) * side
                covered[target] = True
        gap = (desired - z_support[covered]) * side
        row = dict(productId=identity, texture=projection['file'], carrierParts=carrier_names,
                   opaqueTexels=len(xy), coveredTexels=int(covered.sum()), unsupportedTexels=int((~covered).sum()),
                   supportGapMm=[float(gap.min()), float(gap.max())], textureSha256=digest(texture),
                   modelSha256=digest(folder / 'model.json'), geometrySha256=digest(folder / metadata['binary']),
                   planSha256=digest(folder / 'model-plan.json'))
        assert covered.all(), row
        assert gap.min() >= -1e-5 and gap.max() <= .051, row
        rows.append(row)
    return rows


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    rows = [row for identity in ['m5stack-u138', 'm5stack-u037', 'm5stack-k115'] for row in check(identity)]
    receipt = dict(schemaVersion='1', checkedAt=datetime.datetime.now(datetime.timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ'),
                   method='All alpha>=128 texel centers mapped through exact UV bounds and ray-tested along the surface normal against actual exported triangles. Independent paper surfaces exclude themselves from support candidates.',
                   limitations='Geometry checks only. Estimated hardware carriers remain estimates; 0.025-0.05mm gaps are display anti-z-fighting offsets, not manufacturing accuracy. No browser, physical paper, optics or firmware acceptance.', rows=rows)
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(receipt, ensure_ascii=False, indent=2) + '\n')
    print(json.dumps(receipt, ensure_ascii=False, indent=2))
