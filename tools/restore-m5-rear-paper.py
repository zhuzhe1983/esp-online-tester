#!/usr/bin/env python3
"""Rebuild independently masked paper from retained exact-product photographs.

Original manufacturer CAD and photograph RGB are never modified. The explicit
photo ROI is intersected with the outward CAD face, leaving ports and fasteners
open. All dimensions/registration are photo estimates, not manufacturing data.
"""
import argparse
import hashlib
import json
import math
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageFilter

ROOT = Path(__file__).resolve().parents[1]
SPECS = {
    'm5stack-c145': {
        'photo': 'product-photo-531bf55037', 'crop': [347, 562, 858, 1023],
        'size': [74, 66.4], 'center': [0, -28, -5.925],
        'polygon': [[366, 565], [830, 565], [854, 585], [854, 1019],
                    [716, 1019], [716, 1008], [489, 1008], [489, 1019],
                    [365, 1019], [365, 902], [348, 902], [348, 816],
                    [365, 816]],
        'ellipses': [[428, 796, 470, 838], [728, 796, 770, 838],
                     [334, 946, 378, 990], [823, 946, 867, 990]],
        'rects': [[566, 579, 635, 612], [654, 587, 701, 610],
                  [388, 988, 463, 1038]],
        'group': 'exterior-enclosure',
        'note': 'Tab5 rear lower circuit/pinout paper only; battery lid, header opening, screw holes, sockets and shell remain separate CAD. The photographed screen film is not a framebuffer.',
    },
    'm5stack-k124-b': {
        'photo': 'product-photo-f76707f6d0', 'crop': [276, 337, 516, 483],
        'size': [39.8, 24], 'center': [0, -2, -14.125], 'radius': 6,
        'bridgeCadRecesses': True,
        'group': 'rear-enclosure',
        'note': 'Station-Bat central rear pinout paper between the two mounting clips. Manufacturer photograph retains the printed optional-model table; it does not establish unmodeled circuitry.',
    },
    'm5stack-k123': {
        'photo': 'product-photo-4025ea219b', 'crop': [276, 337, 516, 483],
        'size': [39.8, 24], 'center': [0, 0, -14.125], 'radius': 6,
        'bridgeCadRecesses': True,
        'group': 'rear-enclosure',
        'note': 'Station-485 official gallery supplies the same shared Station rear paper photograph as Station-Bat. Retain its optional-model printing as supplied; no SKU-specific battery internals are inferred. Its CAD assembly is registered 2 mm higher in Y.',
    },
    'm5stack-c125': {
        'photo': 'product-photo-8948e53bb3', 'crop': [333, 286, 469, 522],
        'size': [8.8, 15.2], 'center': [0, 0, -4.575], 'radius': 3,
        'group': 'rear',
        'note': 'NanoC6 white rear identity/pinout paper within the recessed bottom-shell face. Grove and USB-C openings and end ventilation slots remain CAD geometry.',
    },
}


def digest(data):
    return hashlib.sha256(data).hexdigest()


def nearest_back_face(model, raw, width, height, size, center):
    """Orthographic -Z ray against all imported assembled CAD triangles."""
    zbuf = np.full((height, width), np.inf)
    w, h = size
    cx, cy, _ = center
    for part in model['parts']:
        vertices = np.frombuffer(raw, dtype='<f4', count=part['vertexCount']*6,
                                 offset=part['byteOffset']).reshape(-1, 6)[:, :3]
        for triangle in vertices.reshape(-1, 3, 3):
            px = (.5-(triangle[:, 0]-cx)/w)*width-.5
            py = (.5-(triangle[:, 1]-cy)/h)*height-.5
            left = max(0, math.ceil(float(px.min())-1e-7))
            right = min(width-1, math.floor(float(px.max())+1e-7))
            top = max(0, math.ceil(float(py.min())-1e-7))
            bottom = min(height-1, math.floor(float(py.max())+1e-7))
            if right < left or bottom < top:
                continue
            denominator = (py[1]-py[2])*(px[0]-px[2])+(px[2]-px[1])*(py[0]-py[2])
            if abs(denominator) < 1e-10:
                continue
            xx, yy = np.meshgrid(np.arange(left, right+1), np.arange(top, bottom+1))
            a = ((py[1]-py[2])*(xx-px[2])+(px[2]-px[1])*(yy-py[2]))/denominator
            b = ((py[2]-py[0])*(xx-px[2])+(px[0]-px[2])*(yy-py[2]))/denominator
            c = 1-a-b
            inside = (a >= -1e-7) & (b >= -1e-7) & (c >= -1e-7)
            z = a*triangle[0, 2]+b*triangle[1, 2]+c*triangle[2, 2]
            view = zbuf[top:bottom+1, left:right+1]
            np.minimum(view, np.where(inside, z, np.inf), out=view)
    return zbuf


def restore(identity):
    spec = SPECS[identity]
    directory = ROOT/'web/platform/assets'/identity
    metadata_file = directory/'model.json'
    metadata = json.loads(metadata_file.read_text())
    source = next(p for p in metadata['referencePhotos'] if p['sourceId'] == spec['photo'])
    source_bytes = (directory/source['file']).read_bytes()
    assert digest(source_bytes) == source['sourceSha256'], 'Retained source photograph changed'
    raw = (directory/metadata['binary']).read_bytes()
    assert digest(raw) == metadata['artifacts'][metadata['binary']], 'Original CAD changed'
    original = Image.open(directory/source['file']).convert('RGB')
    mask = Image.new('L', original.size, 0)
    painter = ImageDraw.Draw(mask)
    if 'polygon' in spec:
        painter.polygon(spec['polygon'], fill=255)
    else:
        painter.rounded_rectangle(spec['crop'], radius=spec['radius'], fill=255)
    for ellipse in spec.get('ellipses', []):
        painter.ellipse(ellipse, fill=0)
    for rectangle in spec.get('rects', []):
        painter.rectangle(rectangle, fill=0)
    image = original.crop(spec['crop']).convert('RGBA')
    alpha = np.array(mask.crop(spec['crop']))
    zbuf = nearest_back_face(metadata, raw, *image.size, spec['size'], spec['center'])
    gap = zbuf-spec['center'][2]
    # The official Station paper is visibly continuous across small holes and
    # grooves in the supplied rear CAD face. Preserve that real thin-sheet span;
    # do not invent holes in printed text or fill the manufacturer's solid mesh.
    support = Image.fromarray(np.uint8((gap >= .015) &
        ((gap <= .12) | spec.get('bridgeCadRecesses', False)))*255)
    # One pixel of edge clearance around source holes or incompatible CAD relief.
    support = np.array(support.filter(ImageFilter.MinFilter(3)))
    alpha = np.minimum(alpha, support)
    image.putalpha(Image.fromarray(alpha))
    texture_file = 'rear-paper-label.png'
    image.save(directory/texture_file, optimize=True)
    output_sha = digest((directory/texture_file).read_bytes())
    label = {
        'id': 'rear-paper-label', 'kind': 'label', 'texture': texture_file,
        'geometry': {'sizeMm': spec['size'], 'centerMm': spec['center'],
                     'rotation': [0, math.pi, 0]},
        'basis': 'image-measurement', 'sourceIds': [spec['photo']],
        'notes': spec['note']+' Registration is estimated from the retained near-frontal photo and native CAD recess. Paper RGB/lighting are uncalibrated; the alpha plane follows its enclosure inspection group and remains independent of shell color.',
    }
    metadata['derivedParts'] = [p for p in metadata['derivedParts'] if p['id'] != label['id']]+[label]
    texture = {
        'file': texture_file, 'sourceId': spec['photo'], 'sourceSha256': digest(source_bytes),
        'cropXYXY': spec['crop'], 'outputPixels': list(image.size),
        'method': 'Original RGB crop without warp or generated lettering; explicit paper ROI intersected with nearest assembled CAD -Z face, gap 0.015–0.12 mm, one-texel mechanical edge clearance. '+('Official continuous Station paper bridges local CAD holes/grooves, supported by the surrounding coplanar rear face; source solid is unchanged. ' if spec.get('bridgeCadRecesses') else '')+'Deterministic tools/restore-m5-rear-paper.py.',
        'alphaMask': {k: spec[k] for k in ['polygon', 'radius', 'ellipses', 'rects'] if k in spec},
        'notes': spec['note'],
    }
    metadata['textures'] = [t for t in metadata['textures'] if t['file'] != texture_file]+[texture]
    metadata['artifacts'][texture_file] = output_sha
    for group in metadata['presentation'].get('inspectionGroups', []):
        if group['id'] == spec['group'] and label['id'] not in group['parts']:
            group['parts'].append(label['id'])
    metadata_file.write_text(json.dumps(metadata, indent=2, ensure_ascii=False)+'\n')
    plan_file = directory/'model-plan.json'
    plan = json.loads(plan_file.read_text())
    plan['derivedParts'] = [p for p in plan.get('derivedParts', []) if p['id'] != label['id']]+[label]
    if 'presentation' in plan:
        plan['presentation'] = metadata['presentation']
    plan['paperRestoration'] = {'texture': texture, 'originalCadSha256': digest(raw),
                                'rebuild': 'python tools/restore-m5-rear-paper.py '+identity}
    plan_file.write_text(json.dumps(plan, indent=2, ensure_ascii=False)+'\n')
    opaque = alpha >= 128
    receipt = {'id': identity, 'sourcePhoto': source['file'], 'sourceSha256': digest(source_bytes),
               'originalCadSha256': digest(raw), 'outputSha256': output_sha,
               'opaqueTexels': int(opaque.sum()), 'missingSupportTexels': int((~np.isfinite(gap[opaque])).sum()),
               'immediateFaceSupportTexels': int((gap[opaque] <= .12).sum()),
               'photoVerifiedPaperSpanTexels': int((gap[opaque] > .12).sum()) if spec.get('bridgeCadRecesses') else 0,
               'finiteGapMmRange': [float(gap[opaque & np.isfinite(gap)].min()),
                                    float(gap[opaque & np.isfinite(gap)].max())],
               'scope': 'Photo-derived paper and CAD texel support only; dimensions approximate, no physical or GPU validation.',
               'localCadRecessPolicy': 'Continuous official Station paper spans native CAD holes/grooves with surrounding face support. No shell triangles are added or removed.' if spec.get('bridgeCadRecesses') else 'Paper alpha excludes incompatible CAD relief and every retained texel has immediate face support.'}
    (directory/'rear-paper-receipt.json').write_text(json.dumps(receipt, indent=2)+'\n')
    print(json.dumps(receipt))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('ids', nargs='+', choices=list(SPECS))
    for identity in parser.parse_args().ids:
        restore(identity)
