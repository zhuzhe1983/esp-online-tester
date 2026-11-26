#!/usr/bin/env python3
"""Rebuild the five November 2026 M5Stack preview additions from retained inputs.

python tools/build-m5-missing-models.py --output /tmp/m5-previews [product-id ...]
No downloads. Sources, CAD selection, rigid transforms and photo masks are declared
in each model-plan.json. Approximate geometry does not imply a board runtime.
"""
import argparse
import copy
import gzip
import hashlib
import importlib.util
import json
from pathlib import Path
import shutil

import numpy as np
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT / 'web/platform/assets'
IDS = ['m5stack-k159', 'm5stack-k162', 'm5stack-k016-p-se', 'm5stack-c156', 'm5stack-a172']


def sha(data):
    return hashlib.sha256(data).hexdigest()


def write(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')


def build(product_id, output):
    folder = ASSETS / product_id
    plan = json.loads((folder / 'model-plan.json').read_text())
    if plan['id'] != product_id:
        raise ValueError('Mismatched product identity')
    out = output / product_id
    if out.resolve() == folder.resolve():
        raise ValueError('Rebuild into a separate directory')
    out.mkdir(parents=True, exist_ok=True)
    for name, expected in plan['retainedHashes'].items():
        if sha((folder / name).read_bytes()) != expected:
            raise ValueError('Retained source changed: ' + name)
        shutil.copyfile(folder / name, out / name)
    records, buffers, offset = [], [], 0
    component_receipts = []
    # Retain an explicitly identified, already assembled family mesh without
    # silently replaying the raw CAD's exploded/local seating transforms.
    for snapshot in plan.get('meshSnapshots', []):
        raw = gzip.decompress((folder / snapshot['file']).read_bytes())
        if sha(raw) != snapshot['expandedSha256']:
            raise ValueError('Mesh snapshot digest changed')
        translation = np.asarray(snapshot['translationMm'], dtype=float)
        if translation.shape != (3,) or not np.isfinite(translation).all():
            raise ValueError('Invalid snapshot translation')
        for definition in snapshot['parts']:
            start, count = definition['sourceByteOffset'], definition['vertexCount']
            if start < 0 or start % 4 or count < 3 or count % 3 or start + count * 24 > len(raw):
                raise ValueError('Invalid snapshot part range')
            values = np.frombuffer(raw, dtype='<f4', count=count * 6, offset=start).reshape(-1, 6).copy()
            values[:, :3] += translation
            if not np.isfinite(values).all():
                raise ValueError('Non-finite snapshot geometry')
            record = {k: copy.deepcopy(v) for k, v in definition.items() if k != 'sourceByteOffset'}
            records.append(record | {'byteOffset': offset, 'boundsMm': [values[:, :3].min(0).tolist(), values[:, :3].max(0).tolist()]})
            buffers.append(values.tobytes())
            offset += values.nbytes
    if plan.get('cad'):
        import trimesh
        cad = plan['cad']
        source = folder / cad['file']
        components = trimesh.load(source).split(only_watertight=False)
        matrix = np.array(cad['matrixRows'], dtype=float)
        rotation = matrix[:3, :3]
        if not np.allclose(rotation.T @ rotation, np.eye(3), atol=1e-8) or not np.isclose(np.linalg.det(rotation), 1):
            raise ValueError('CAD transforms must be rigid and preserve millimetres')
        for index, mesh in enumerate(components):
            component_receipts.append({'index': index, 'triangles': len(mesh.faces), 'boundsMm': mesh.bounds.tolist(), 'selected': any(p['sourceComponent'] == index for p in cad['parts'])})
        for definition in cad['parts']:
            mesh = components[definition['sourceComponent']]
            vertices = mesh.vertices @ rotation.T + matrix[:3, 3]
            values = np.concatenate([vertices[mesh.faces].reshape(-1, 3), np.repeat(mesh.face_normals @ rotation.T, 3, axis=0)], axis=1)
            record = copy.deepcopy(definition)
            values = values.astype('<f4')
            records.append(record | {'byteOffset': offset, 'vertexCount': len(values), 'boundsMm': [values[:, :3].min(0).tolist(), values[:, :3].max(0).tolist()]})
            buffers.append(values.tobytes())
            offset += values.nbytes
    spec = importlib.util.spec_from_file_location('preview_primitives', ROOT / 'tools/build-lilygo-expansion-models.py')
    primitives = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(primitives)
    for definition in plan.get('parts', []):
        values = primitives.primitive(definition).astype('<f4')
        record = {k: v for k, v in definition.items() if k not in ['kind', 'sizeMm', 'radiusMm', 'centerMm', 'rotation', 'outlineMm', 'openingMm', 'innerRadiusMm']}
        records.append(record | {'byteOffset': offset, 'vertexCount': len(values), 'boundsMm': [values[:, :3].min(0).tolist(), values[:, :3].max(0).tolist()]})
        buffers.append(values.tobytes())
        offset += values.nbytes
    for texture in plan.get('photoTextures', []):
        source = Image.open(folder / texture['sourceFile']).convert('RGBA')
        crop = texture['cropPixels']
        image = source.crop(crop)
        mask = Image.new('L', image.size, 0)
        draw = ImageDraw.Draw(mask)
        for polygon in texture['includePolygonsPixels']:
            draw.polygon([(x - crop[0], y - crop[1]) for x, y in polygon], fill=255)
        for polygon in texture.get('excludePolygonsPixels', []):
            draw.polygon([(x - crop[0], y - crop[1]) for x, y in polygon], fill=0)
        for x, y, radius in texture.get('excludeCirclesPixels', []):
            draw.ellipse((x-radius-crop[0], y-radius-crop[1], x+radius-crop[0], y+radius-crop[1]), fill=0)
        image.putalpha(mask)
        image.save(out / texture['file'], optimize=True)
    binary = b''.join(buffers)
    (out / 'geometry.bin.gz').write_bytes(gzip.compress(binary, mtime=0))
    artifacts = {name: sha((out / name).read_bytes()) for name in plan['retainedHashes']}
    artifacts['geometry.bin.gz'] = sha((out / 'geometry.bin.gz').read_bytes())
    for texture in plan.get('photoTextures', []):
        artifacts[texture['file']] = sha((out / texture['file']).read_bytes())
    model = {k: copy.deepcopy(plan[k]) for k in ['id', 'dimensions', 'presentation', 'derivedParts', 'referencePhotos', 'provenance', 'limitations', 'representationScope'] if k in plan}
    model.update(schemaVersion='1', unit='mm', strideFloats=6, binary='geometry.bin.gz', binaryCompression='gzip', parts=records, artifacts=artifacts)
    if plan.get('cad'):
        model['sourceTransform'] = {'matrixRows': plan['cad']['matrixRows'], 'scale': 1, 'note': plan['cad']['notes']}
    write(out / 'model.json', model)
    bounds = np.array([record['boundsMm'] for record in records])
    receipt = {'schemaVersion': '1', 'productId': product_id, 'modelSha256': sha((out / 'model.json').read_bytes()), 'geometrySha256': sha(binary), 'storedGeometrySha256': artifacts['geometry.bin.gz'], 'triangleCount': sum(p['vertexCount']//3 for p in records), 'cadComponents': component_receipts, 'meshBoundsMm': [bounds[:, 0].min(0).tolist(), bounds[:, 1].max(0).tolist()], 'measurementScope': 'Native mesh and added mesh primitives only; dynamic derivedParts must be measured in the viewer separately.', 'photoComparison': 'pending-browser-comparison', 'runtimeValidation': False, 'physicalValidation': False}
    write(out / 'build-check.json', receipt)
    return receipt


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('ids', nargs='*')
    args = parser.parse_args()
    for product_id in args.ids or IDS:
        print(json.dumps(build(product_id, args.output), ensure_ascii=False))
