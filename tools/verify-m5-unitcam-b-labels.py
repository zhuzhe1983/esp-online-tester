#!/usr/bin/env python3
"""Check every U109-B opaque texel against carrier triangles from the real loader."""
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


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('carriers', type=Path, help='JSON emitted as the second output of verify-m5-unitcam-b-model.mjs')
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    data = json.loads(args.carriers.read_text())
    folder = ROOT / 'web/platform/assets/m5stack-u109-b'
    assert data['productId'] == 'm5stack-u109-b'
    assert digest(folder / 'model.json') == data['modelSha256']
    assert digest(folder / 'model-plan.json') == data['planSha256']
    assert digest(folder / 'geometry.bin') == data['geometrySha256']
    rows = []
    for label in data['labels']:
        image = np.asarray(Image.open(folder / label['file']).convert('RGBA'))
        pixels = np.argwhere(image[:, :, 3] >= 128)
        height, width = image.shape[:2]
        u, v = (pixels[:, 1] + .5) / width, (pixels[:, 0] + .5) / height
        x0, y0, x1, y1 = label['boundsMm']
        side = 1 if label['side'] == 'front' else -1
        xy = np.column_stack([x0 + u * (x1-x0) if side == 1 else x1 - u * (x1-x0), y1-v*(y1-y0)])
        triangles = np.asarray(data['carrierGeometry'][label['carrier']]).reshape(-1, 3, 3)
        normals = np.cross(triangles[:, 1]-triangles[:, 0], triangles[:, 2]-triangles[:, 0])
        lengths = np.linalg.norm(normals, axis=1)
        usable = lengths > 1e-10
        usable &= normals[:, 2] * side > .995 * lengths
        usable &= abs(triangles[:, :, 2].mean(1)-label['zMm']) < .1
        covered = np.zeros(len(xy), dtype=bool)
        support = np.full(len(xy), -np.inf*side)
        for triangle in triangles[usable]:
            p0, p1, p2 = triangle
            low, high = triangle[:, :2].min(0)-1e-7, triangle[:, :2].max(0)+1e-7
            ix = np.flatnonzero((xy[:, 0] >= low[0]) & (xy[:, 0] <= high[0]) & (xy[:, 1] >= low[1]) & (xy[:, 1] <= high[1]))
            a, b = p1[:2]-p0[:2], p2[:2]-p0[:2]
            det = a[0]*b[1]-a[1]*b[0]
            if abs(det) < 1e-10:
                continue
            delta = xy[ix]-p0[:2]
            s = (delta[:, 0]*b[1]-delta[:, 1]*b[0])/det
            t = (a[0]*delta[:, 1]-a[1]*delta[:, 0])/det
            inside = (s >= -1e-7) & (t >= -1e-7) & (s+t <= 1+1e-7)
            indices = ix[inside]
            z = p0[2]+s[inside]*(p1[2]-p0[2])+t[inside]*(p2[2]-p0[2])
            support[indices] = np.maximum(support[indices]*side, z*side)*side
            covered[indices] = True
        gap = (label['zMm']-support[covered])*side
        row = dict(texture=label['file'], carrier=label['carrier'], opaqueTexels=len(xy), coveredTexels=int(covered.sum()),
                   unsupportedTexels=int((~covered).sum()), supportGapMm=[float(gap.min()), float(gap.max())], textureSha256=digest(folder / label['file']))
        assert covered.all(), row
        assert gap.min() >= -1e-5 and gap.max() <= .026, row
        rows.append(row)
    receipt = dict(schemaVersion='1', productId=data['productId'], checkedAt=datetime.datetime.now(datetime.timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ'),
                   modelSha256=data['modelSha256'], planSha256=data['planSha256'], geometrySha256=data['geometrySha256'],
                   loadedCarrierExportSha256=digest(args.carriers), rows=rows,
                   method='Every alpha>=128 texel center is ray-tested along its normal against the actual loader-created rear-cover/RF-shield triangles. Paper and photo planes are excluded as their own support.',
                   limitations='Geometry/material checks only; carrier/window placement is photo-estimated.0.025mm clearance prevents display z-fighting, not manufacturing accuracy. No browser or hardware acceptance.')
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(receipt, ensure_ascii=False, indent=2)+'\n')
    print(json.dumps(receipt, ensure_ascii=False, indent=2))
