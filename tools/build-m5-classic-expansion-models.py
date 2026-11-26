#!/usr/bin/env python3
"""Rebuild five exact-SKU classic previews from retained, hashed source inputs.

Family CAD is explicitly identified as shared/approximate, never relabelled as
an exact-SKU engineering assembly. No network or firmware downloads occur.
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
from m5_texture_projection import projected_texture

ROOT = Path(__file__).resolve().parents[1]
IDS = ['m5stack-k002', 'm5stack-k001-v26', 'm5stack-k001-b', 'm5stack-c001-cnc', 'm5stack-k049']
def sha(data):
    return hashlib.sha256(data).hexdigest()
def write(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')
def build(product_id, output):
    folder = ROOT / 'web/platform/assets' / product_id
    plan = json.loads((folder / 'model-plan.json').read_text())
    if plan.get('controlSurfaceRecipe'):
        spec = importlib.util.spec_from_file_location('control_surface_rebuild', ROOT / 'tools/rebuild-m5-control-models.py')
        builder = importlib.util.module_from_spec(spec); spec.loader.exec_module(builder)
        result = builder.build(product_id, ROOT, ROOT, output)
        return {'id': product_id, 'modelSha256': result['modelSha256']}
    if plan['id'] != product_id:
        raise ValueError('Product identity mismatch')
    out = output / product_id
    if out.resolve() == folder.resolve():
        raise ValueError('Use a separate output directory')
    out.mkdir(parents=True, exist_ok=True)
    for name, digest in plan['retainedHashes'].items():
        if Path(name).name != name or sha((folder / name).read_bytes()) != digest:
            raise ValueError('Unsafe or changed retained source: ' + name)
        shutil.copyfile(folder / name, out / name)
    family = plan['familyGeometry']
    binary = gzip.decompress((folder / family['file']).read_bytes())
    if sha(binary) != family['expandedSha256']:
        raise ValueError('Family geometry digest changed')
    parts = copy.deepcopy(plan['nativeParts'])
    arrays, offset = [binary], len(binary)
    for part in parts:
        begin, count = part['byteOffset'], part['vertexCount']
        if begin < 0 or begin % 4 or count < 3 or count % 3 or begin + count * 24 > len(binary):
            raise ValueError('Invalid retained part range')
    spec = importlib.util.spec_from_file_location('classic_preview_primitives', ROOT / 'tools/build-lilygo-expansion-models.py')
    primitives = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(primitives)
    for definition in plan.get('proceduralParts', []):
        values = primitives.primitive(definition).astype('<f4')
        record = {k: copy.deepcopy(v) for k, v in definition.items() if k not in ['kind', 'sizeMm', 'radiusMm', 'centerMm', 'rotation']}
        record.update(byteOffset=offset, vertexCount=len(values), boundsMm=[values[:, :3].min(0).tolist(), values[:, :3].max(0).tolist()])
        parts.append(record)
        arrays.append(values.tobytes())
        offset += values.nbytes
    texture_records=[]
    for texture in plan.get('photoTextures', []):
        if texture.get('perspectiveQuad'):
            texture_records.append(projected_texture(folder/texture['sourceFile'],texture,out/texture['file']))
            continue
        texture_records.append(texture)
        image = Image.open(folder / texture['sourceFile']).convert('RGBA')
        factor = 4
        mask = Image.new('L', (image.width * factor, image.height * factor), 0)
        draw = ImageDraw.Draw(mask)
        for rectangle in texture['includeRoundedRects']:
            draw.rounded_rectangle([round(v * factor) for v in rectangle['xyxy']], radius=rectangle.get('radiusPx', 0) * factor, fill=255)
        for rectangle in texture.get('excludeRects', []):
            draw.rectangle([round(v * factor) for v in rectangle], fill=0)
        image.putalpha(mask.resize(image.size, Image.Resampling.LANCZOS))
        image.crop(texture['cropXYXY']).save(out / texture['file'], optimize=True)
    raw = b''.join(arrays)
    (out / 'geometry.bin.gz').write_bytes(gzip.compress(raw, mtime=0))
    model = dict(schemaVersion='1', id=product_id, unit='mm', binary='geometry.bin.gz', binaryCompression='gzip', strideFloats=6, parts=parts,
                 sourceTransform=plan['sourceTransform'], dimensions=plan['dimensions'], presentation=plan['presentation'], derivedParts=plan['derivedParts'],
                 derivedSurfaces=[], referencePhotos=plan['referencePhotos'], textures=texture_records, provenance=plan['provenance'], limitations=plan['limitations'])
    model['artifacts'] = {p.name: sha(p.read_bytes()) for p in sorted(out.iterdir()) if p.is_file()}
    write(out / 'model.json', model)
    values = np.frombuffer(raw, dtype='<f4').reshape(-1, 6)
    if not np.isfinite(values).all():
        raise ValueError('Non-finite geometry')
    bounds = [values[:, :3].min(0).tolist(), values[:, :3].max(0).tolist()]
    write(out / 'rebuild-receipt.json', dict(productId=product_id, modelSha256=sha((out / 'model.json').read_bytes()), geometrySha256=sha(raw), nativeAndProceduralBoundsMm=bounds,
                                          scope='Native family meshes plus explicit exterior approximations; not an exact-SKU dimension or photo-overlay acceptance. Derived runtime parts, input overlays and UNKNOWN text are excluded from these bounds.'))
    return {'id': product_id, 'modelSha256': sha((out / 'model.json').read_bytes())}
if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('product_ids', nargs='*')
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    for product_id in args.product_ids or IDS:
        if product_id not in IDS:
            raise ValueError('Unexpected product')
        print(json.dumps(build(product_id, args.output)))
