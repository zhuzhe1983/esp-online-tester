#!/usr/bin/env python3
"""Offline previews for exact U138/U037/K115 source plans; never executes firmware."""
import argparse
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import shutil
import cv2
import numpy as np
from PIL import Image, ImageDraw
import trimesh

ROOT = Path(__file__).resolve().parents[1]
IDS = ['m5stack-u138', 'm5stack-u037', 'm5stack-k115']


def digest(data):
    return hashlib.sha256(data).hexdigest()


def write(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')


def build(identity, destination):
    folder = ROOT / 'web/platform/assets' / identity
    plan = json.loads((folder / 'model-plan.json').read_text())
    assert plan['id'] == identity
    out = destination / identity
    if out.resolve() == folder.resolve():
        raise ValueError('Rebuild into an independent output directory')
    if out.exists() and any(out.iterdir()):
        raise ValueError('Output already contains evidence; choose a fresh output directory')
    out.mkdir(parents=True, exist_ok=True)
    for name, expected in plan['retainedHashes'].items():
        assert Path(name).name == name
        data = (folder / name).read_bytes()
        assert digest(data) == expected, name
        shutil.copyfile(folder / name, out / name)
    module_spec = importlib.util.spec_from_file_location('preview_primitives', ROOT / 'tools/build-lilygo-expansion-models.py')
    primitives = importlib.util.module_from_spec(module_spec)
    module_spec.loader.exec_module(primitives)
    parts, buffers, offset = [], [], 0

    def add(record, values):
        nonlocal offset
        values = np.asarray(values, dtype='<f4')
        assert values.ndim == 2 and values.shape[1] == 6 and len(values) % 3 == 0 and np.isfinite(values).all()
        parts.append({**record, 'byteOffset': offset, 'vertexCount': len(values),
                      'boundsMm': [values[:, :3].min(0).tolist(), values[:, :3].max(0).tolist()]})
        buffers.append(values.tobytes())
        offset += values.nbytes

    if plan.get('cad'):
        cad = plan['cad']
        # process=False preserves original STL triangles, including open shells.
        mesh = trimesh.load(folder / cad['file'], process=False)
        matrix = np.asarray(cad['matrixRows'])
        rotation = matrix[:3, :3]
        assert np.allclose(rotation.T @ rotation, np.eye(3)) and np.isclose(np.linalg.det(rotation), 1)
        for part in cad['parts']:
            indices = np.concatenate([np.arange(a, b) for a, b in part['sourceFaceRanges']])
            assert len(np.unique(indices)) == len(indices) and indices.min() >= 0 and indices.max() < len(mesh.faces)
            triangles = mesh.triangles[indices] @ rotation.T + matrix[:3, 3]
            normals = mesh.face_normals[indices] @ rotation.T
            add(copy.deepcopy(part), np.concatenate([triangles.reshape(-1, 3), np.repeat(normals, 3, axis=0)], axis=1))
    for part in plan.get('parts', []):
        record = {k: copy.deepcopy(v) for k, v in part.items()
                  if k not in ['kind', 'sizeMm', 'radiusMm', 'openingMm', 'innerRadiusMm', 'centerMm', 'rotation', 'outlineMm']}
        add(record, primitives.primitive(part))
    for texture in plan.get('photoDerivations', []):
        image = Image.open(folder / texture['sourceFile']).convert('RGBA')
        factor = 4
        mask = Image.new('L', (image.width * factor, image.height * factor), 0)
        draw = ImageDraw.Draw(mask)
        for polygon in texture.get('includePolygons', []):
            draw.polygon([(round(x * factor), round(y * factor)) for x, y in polygon], fill=255)
        for rectangle in texture.get('includeRoundedRects', []):
            draw.rounded_rectangle([round(v * factor) for v in rectangle['xyxy']], radius=rectangle.get('radiusPx', 0) * factor, fill=255)
        for polygon in texture.get('excludePolygons', []):
            draw.polygon([(round(x * factor), round(y * factor)) for x, y in polygon], fill=0)
        image.putalpha(mask.resize(image.size, Image.Resampling.LANCZOS))
        if texture.get('perspectiveQuad'):
            quad = np.asarray(texture['perspectiveQuad'], dtype=np.float32)
            width, height = texture['outputPixels']
            matrix = cv2.getPerspectiveTransform(quad, np.asarray([[0, 0], [width-1, 0], [width-1, height-1], [0, height-1]], dtype=np.float32))
            image = Image.fromarray(cv2.warpPerspective(np.asarray(image), matrix, (width, height), flags=cv2.INTER_LANCZOS4))
        else:
            image = image.crop(texture['cropXYXY'])
        image.save(out / texture['file'], optimize=True)
    geometry = b''.join(buffers)
    (out / 'geometry.bin').write_bytes(geometry)
    model = {k: copy.deepcopy(plan[k]) for k in ['id', 'dimensions', 'presentation', 'derivedParts', 'referencePhotos',
                                                'photoDerivations', 'provenance', 'limitations', 'reviewViews', 'omittedComponents'] if k in plan}
    model.update(schemaVersion='1', unit='mm', binary='geometry.bin', strideFloats=6, parts=parts)
    if plan.get('cad'):
        model['sourceTransform'] = {'matrixRows': plan['cad']['matrixRows'], 'scale': 1,
                                    'note': 'Native STL triangle subsets, one rigid transform. No scaling or mesh hole repair.'}
    model['artifacts'] = {path.name: digest(path.read_bytes()) for path in sorted(out.iterdir()) if path.is_file() and path.name != 'model.json'}
    write(out / 'model.json', model)
    return {'productId': identity, 'modelSha256': digest((out / 'model.json').read_bytes()),
            'geometrySha256': digest(geometry), 'triangles': sum(p['vertexCount'] // 3 for p in parts),
            'scope': plan['representationScope'], 'firmwareExecuted': False, 'hardwareValidated': False}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('ids', nargs='*')
    args = parser.parse_args()
    rows = []
    for identity in args.ids or IDS:
        if identity not in IDS:
            raise ValueError('Unsupported product')
        rows.append(build(identity, args.output))
    print(json.dumps(rows, ensure_ascii=False, indent=2))
