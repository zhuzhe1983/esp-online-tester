#!/usr/bin/env python3
"""Restore explicitly reviewed paper regions without rebuilding manufacturer CAD.

Per-SKU paper-label-repair.json contains the original-image masks, placement
estimates and positive/negative probes. The ordinary model-plan textureSources
also retain those masks for a full offline product rebuild.
"""
import argparse
import copy
import hashlib
import json
from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[1]
METHOD = 'Fixed source-photo crop; explicit original-coordinate alpha regions at 4x supersampling exclude shell, screen, keys, openings and fasteners. Original RGB pixels only; no generated text, recoloring or perspective warp.'


def digest(data):
    return hashlib.sha256(data).hexdigest()


def dump(value):
    return json.dumps(value, ensure_ascii=False, indent=2) + '\n'


def masked(image, spec):
    factor = 4
    alpha = Image.new('L', (image.width * factor, image.height * factor), 0)
    draw = ImageDraw.Draw(alpha)
    for polygon in spec.get('includePolygons', []):
        draw.polygon([(round(x * factor), round(y * factor)) for x, y in polygon], fill=255)
    for rectangle in spec.get('includeRoundedRects', []):
        draw.rounded_rectangle([round(v * factor) for v in rectangle['xyxy']], radius=rectangle.get('radiusPx', 0) * factor, fill=255)
    for polygon in spec.get('excludePolygons', []):
        draw.polygon([(round(x * factor), round(y * factor)) for x, y in polygon], fill=0)
    for rectangle in spec.get('excludeRects', []):
        draw.rectangle([round(v * factor) for v in rectangle], fill=0)
    for ellipse in spec.get('excludeEllipses', []):
        draw.ellipse([round(v * factor) for v in ellipse], fill=0)
    result = image.convert('RGBA')
    result.putalpha(alpha.resize(image.size, Image.Resampling.LANCZOS))
    return result


def replace_by_id(items, entry, key):
    index = next((i for i, item in enumerate(items) if item[key] == entry[key]), None)
    if index is None:
        items.append(copy.deepcopy(entry))
    else:
        items[index] = copy.deepcopy(entry)


def update(product, write):
    assets = ROOT / 'web/platform/assets' / product
    definition = json.loads((assets / 'paper-label-repair.json').read_text())
    metadata = json.loads((assets / 'model.json').read_text())
    plan = json.loads((assets / 'model-plan.json').read_text())
    for name, expected in definition['unchangedArtifacts'].items():
        assert digest((assets / name).read_bytes()) == expected, 'Manufacturer geometry/source changed: ' + name
    before_parts = copy.deepcopy(metadata['parts'])
    checks = []
    for repair in definition['labels']:
        texture = repair['texture']
        source = next(photo for photo in metadata['referencePhotos'] if photo['sourceId'] == texture['sourceId'])
        source_bytes = (assets / source['file']).read_bytes()
        assert digest(source_bytes) == source['sourceSha256'], 'Pinned photograph changed'
        original = Image.open(assets / source['file']).convert('RGB')
        image = masked(original, texture['alphaMask']).crop(texture['cropXYXY'])
        assert image.convert('RGB').tobytes() == original.crop(texture['cropXYXY']).tobytes(), 'Photograph RGB changed'
        for probe in repair['probes']:
            x, y = probe['sourcePixel']; left, top, _, _ = texture['cropXYXY']
            actual = image.getpixel((x - left, y - top))[3]
            assert actual == probe['alpha'], (product, texture['file'], probe, actual)
            checks.append({'texture': texture['file'], **probe, 'actualAlpha': actual})
        record = {**texture, 'sourceSha256': source['sourceSha256'], 'outputPixels': list(image.size), 'method': METHOD}
        if write:
            image.save(assets / texture['file'])
            replace_by_id(plan.setdefault('textureSources', []), texture, 'file')
            replace_by_id(plan['derivedParts'], repair['part'], 'id')
            replace_by_id(metadata['textures'], record, 'file')
            replace_by_id(metadata['derivedParts'], repair['part'], 'id')
            metadata['artifacts'][texture['file']] = digest((assets / texture['file']).read_bytes())
        else:
            assert Image.open(assets / texture['file']).convert('RGBA').tobytes() == image.tobytes(), 'Committed texture differs from the mask'
            assert next(t for t in plan['textureSources'] if t['file'] == texture['file']) == texture
            assert next(t for t in metadata['textures'] if t['file'] == texture['file']) == record
            for document in [metadata, plan]:
                assert next(p for p in document['derivedParts'] if p['id'] == repair['part']['id']) == repair['part']
    for document in [metadata, plan]:
        for group_id, labels in definition.get('inspectionGroups', {}).items():
            group = next(g for g in document['presentation']['inspectionGroups'] if g['id'] == group_id)
            for label in labels:
                if write and label not in group['parts']:
                    group['parts'].append(label)
                assert label in group['parts'], 'Paper must follow its enclosure during inspection'
    assert metadata['parts'] == before_parts
    if write:
        (assets / 'model-plan.json').write_text(dump(plan))
        (assets / 'model.json').write_text(dump(metadata))
    receipt = {'schemaVersion': '1', 'productId': product, 'unchangedArtifacts': definition['unchangedArtifacts'], 'manufacturerPartsUnchanged': True, 'sourceRgbUnchanged': True, 'alphaProbes': checks, 'modelSha256': digest((assets / 'model.json').read_bytes()), 'planSha256': digest((assets / 'model-plan.json').read_bytes()), 'scope': definition['scope'], 'mode': 'write' if write else 'check'}
    print(dump(receipt), end='')
    return receipt


def support(product):
    """Check every opaque pixel against unchanged native triangles, both sides."""
    import numpy as np
    assets = ROOT / 'web/platform/assets' / product
    definition = json.loads((assets / 'paper-label-repair.json').read_text())
    metadata = json.loads((assets / 'model.json').read_text())
    binary = (assets / metadata['binary']).read_bytes()
    triangles = np.concatenate([np.frombuffer(binary, dtype='<f4', count=part['vertexCount'] * 6, offset=part['byteOffset']).reshape(-1, 3, 6)[:, :, :3] for part in metadata['parts']]).astype(float)
    rows = []
    for repair in definition['labels']:
        part = repair['part']; geometry = part['geometry']
        assert geometry['rotation'] in [[0, 0, 0], [0, 3.141592653589793, 0]]
        vertices = triangles - np.asarray(geometry['centerMm'])
        if geometry['rotation'][1]:
            vertices = vertices * np.asarray([-1, 1, -1])
        image = np.asarray(Image.open(assets / part['texture']).convert('RGBA'))
        height, width = image.shape[:2]; size_x, size_y = geometry['sizeMm']
        alpha = image[:, :, 3] >= 128
        depth_buffer = np.full((height, width), -np.inf)
        for triangle in vertices:
            x = (triangle[:, 0] / size_x + .5) * width - .5
            y = (.5 - triangle[:, 1] / size_y) * height - .5
            left, right = max(0, int(np.ceil(x.min() - 1e-7))), min(width - 1, int(np.floor(x.max() + 1e-7)))
            top, bottom = max(0, int(np.ceil(y.min() - 1e-7))), min(height - 1, int(np.floor(y.max() + 1e-7)))
            if right < left or bottom < top:
                continue
            denominator = (y[1] - y[2]) * (x[0] - x[2]) + (x[2] - x[1]) * (y[0] - y[2])
            if abs(denominator) < 1e-10:
                continue
            xx, yy = np.meshgrid(np.arange(left, right + 1), np.arange(top, bottom + 1))
            a = ((y[1] - y[2]) * (xx - x[2]) + (x[2] - x[1]) * (yy - y[2])) / denominator
            b = ((y[2] - y[0]) * (xx - x[2]) + (x[0] - x[2]) * (yy - y[2])) / denominator
            c = 1 - a - b
            inside = (a >= -1e-7) & (b >= -1e-7) & (c >= -1e-7)
            depths = a * triangle[0, 2] + b * triangle[1, 2] + c * triangle[2, 2]
            view = depth_buffer[top:bottom + 1, left:right + 1]
            np.maximum(view, np.where(inside, depths, -np.inf), out=view)
        gaps = -depth_buffer[alpha]; finite = np.isfinite(gaps)
        row = {'productId': product, 'label': part['id'], 'opaquePixels': int(alpha.sum()), 'missingNativeSupportPixels': int((~finite).sum()), 'intrudingPixels': int((gaps < -.005).sum()), 'gapMmQuantiles': dict(zip(['min', 'p01', 'median', 'p99', 'max'], np.quantile(gaps[finite], [0, .01, .5, .99, 1]).tolist())), 'method': 'Every alpha>=128 texture-pixel center ray-cast against the retained manufacturer triangles in paper-local coordinates. Nearest outward triangle on either side; no derived label, invisible control or UNKNOWN board supplies support. Numerical model-space check, not calibrated hardware fit.'}
        for key, selected in [('missingSourcePixelBounds', alpha & ~np.isfinite(depth_buffer)), ('intrudingSourcePixelBounds', alpha & (depth_buffer > .005))]:
            yy, xx = np.nonzero(selected)
            left, top, _, _ = repair['texture']['cropXYXY']
            row[key] = [int(xx.min()) + left, int(yy.min()) + top, int(xx.max()) + left, int(yy.max()) + top] if len(xx) else None
        rows.append(row)
        print(dump(row), end='')
    return rows


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--ids', required=True, help='Comma-separated catalog IDs')
    parser.add_argument('--write', action='store_true')
    parser.add_argument('--support', action='store_true', help='Also report full opaque-pixel native-triangle support (NumPy)')
    args = parser.parse_args()
    for product in args.ids.split(','):
        update(product, args.write)
        if args.support:
            support(product)
