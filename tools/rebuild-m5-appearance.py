#!/usr/bin/env python3
"""Reapply source-declared appearance metadata without tessellating native CAD.

Run against retained model.json/model-plan.json and their hash-pinned artifacts.
The output must be an independent directory; source files are never overwritten.
"""
import argparse
import hashlib
import json
from pathlib import Path
import shutil


def sha(data):
    return hashlib.sha256(data).hexdigest()


def photo_texture(source, output, specification, source_record, artifacts):
    """Reproduce only explicitly measured source-pixel masks/perspective crops."""
    import cv2
    import numpy as np
    from PIL import Image, ImageDraw
    name = specification['file']
    if Path(name).name != name:
        raise ValueError('Unsafe texture filename')
    original = next(source / file for file, value in artifacts.items()
                    if value == source_record['sha256'])
    image = Image.open(original).convert('RGBA')
    mask = Image.new('L', (image.width * 4, image.height * 4), 0)
    draw = ImageDraw.Draw(mask)
    for polygon in specification['alphaMask']['includePolygons']:
        draw.polygon([(round(x * 4), round(y * 4)) for x, y in polygon], fill=255)
    for polygon in specification['alphaMask'].get('excludePolygons', []):
        draw.polygon([(round(x * 4), round(y * 4)) for x, y in polygon], fill=0)
    image.putalpha(mask.resize(image.size, Image.Resampling.LANCZOS))
    quad = np.asarray(specification['perspectiveQuad'], dtype=np.float32)
    width, height = specification['rectifiedPixels']
    if quad.shape != (4, 2) or not np.isfinite(quad).all() or not all(1 <= n <= 4096 for n in [width, height]):
        raise ValueError('Invalid source-photo perspective declaration')
    target = np.asarray([[0, 0], [width - 1, 0], [width - 1, height - 1], [0, height - 1]], dtype=np.float32)
    transform = cv2.getPerspectiveTransform(quad, target)
    image = Image.fromarray(cv2.warpPerspective(np.asarray(image), transform, (width, height),
                           flags=cv2.INTER_LANCZOS4, borderMode=cv2.BORDER_CONSTANT,
                           borderValue=(0, 0, 0, 0)))
    image.save(output / name)
    return {**specification, 'sourceSha256': source_record['sha256'],
            'perspectiveMatrix': transform.tolist(), 'outputPixels': [width, height],
            'method': 'Explicit original-photo polygon mask at 4x and four-corner Lanczos4 rectification. No repainted/generated pixels. Same-photo appearance is not independent geometry evidence.'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('product_id')
    parser.add_argument('--repo', type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    source = args.repo.resolve() / 'web/platform/assets' / args.product_id
    output = args.output.resolve() / args.product_id
    if output == source or output.is_relative_to(source):
        raise ValueError('Output must be independent from the retained product assets')
    if output.exists():
        raise ValueError('Choose a new output directory to preserve prior rebuilds')
    plan = json.loads((source / 'model-plan.json').read_text())
    metadata = json.loads((source / 'model.json').read_text())
    recipe = plan['appearanceRebuild']
    if recipe['nativeGeometrySha256'] != metadata['artifacts']['geometry.bin']:
        raise ValueError('Native geometry identity changed')
    for name, expected in metadata['artifacts'].items():
        if Path(name).name != name or sha((source / name).read_bytes()) != expected:
            raise ValueError('Retained artifact changed: ' + name)
    sources = {item['id']: item for item in metadata['provenance']['sources']}
    for sid in recipe['sourceIds']:
        source_record = sources[sid]
        if source_record['status'] != 'fetched':
            raise ValueError('Appearance source was not fetched: ' + sid)
        if not any(value == source_record['sha256'] for value in metadata['artifacts'].values()):
            raise ValueError('Appearance source pixels are not retained: ' + sid)
    for field in ['derivedParts', 'presentation', 'limitations']:
        metadata[field] = plan[field]
    for name, appearance in recipe.get('nativePartAppearance', {}).items():
        if set(appearance) - {'color', 'role', 'texture'}:
            raise ValueError('Only native appearance fields may change')
        matches = [part for part in metadata['parts'] if part['name'] == name]
        if len(matches) != 1:
            raise ValueError('Native appearance part is missing or duplicated: ' + name)
        matches[0].update(appearance)
    metadata['appearanceRebuild'] = recipe
    output.mkdir(parents=True)
    for name in metadata['artifacts']:
        shutil.copyfile(source / name, output / name)
    for texture in recipe.get('generatedTextures', []):
        record = photo_texture(source, output, texture, sources[texture['sourceId']], metadata['artifacts'])
        metadata['artifacts'][texture['file']] = sha((output / texture['file']).read_bytes())
        metadata['textures'] = [old for old in metadata.get('textures', []) if old['file'] != texture['file']] + [record]
    for name, data in [('model-plan.json', plan), ('model.json', metadata)]:
        (output / name).write_text(json.dumps(data, ensure_ascii=False, indent=2) + '\n')
    print(json.dumps({'id': args.product_id, 'geometryUnchanged': True,
                      'geometrySha256': metadata['artifacts']['geometry.bin'],
                      'modelSha256': sha((output / 'model.json').read_bytes()),
                      'scope': 'Retained artifact hashes and deterministic appearance metadata only; no CAD retessellation or optical/physical acceptance.'}))


if __name__ == '__main__':
    main()
