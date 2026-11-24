#!/usr/bin/env python3
"""Replace old solid envelopes with source-bounded, explicitly approximate shells.

Run before build-waveshare-models.py. Original photographs are never rewritten.
The printed exterior dimensions are retained; 1.5 mm walls, cap splits and
photo-estimated openings are presentation estimates, not fabrication geometry.
"""
import copy
import hashlib
import json
import math
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
ASSETS = ROOT / 'web/platform/assets'
IDS = [
    'lr1121-oled-1-54', 'touch-lcd-7c-box', 'touch-amoled-1-75c',
    'epaper-1-54g', 'lcd-1-54', 'touch-lcd-1-85b', 'touch-lcd-4b',
    'touch-amoled-1-43c', 'lcd-0-85', 'touch-amoled-2-16',
    'knob-touch-lcd-1-8-blue', 'knob-touch-lcd-1-8-black',
]
NOTE = ('后续空腔修正：原实心外廓已拆为有实际内腔的侧壁、带光学孔的前盖及独立后盖；'
        '外尺寸取原厂标注，1.5mm壁厚/盖厚、未标注接缝与孔位是明确估计，不能用于制造或装配。'
        '照片保持原样，模型开孔去除照片中的端口覆盖；隐藏电路板标识UNKNOWN。'
        '本修正取代早期实心外廓说明，不增加整机或WebGL验收声明。')


def save(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')


def circle(radius, center=(0, 0)):
    return [[center[0] + radius * math.cos(i * math.tau / 128),
             center[1] + radius * math.sin(i * math.tau / 128)] for i in range(128)]


def rect(width, height, center=(0, 0), radius=0):
    if not radius:
        return [[center[0] + x * width / 2, center[1] + y * height / 2]
                for x, y in [(-1, -1), (1, -1), (1, 1), (-1, 1)]]
    return [[center[0] + x * (width / 2 - radius) + radius * math.cos(a + i * math.pi / 24),
             center[1] + y * (height / 2 - radius) + radius * math.sin(a + i * math.pi / 24)]
            for x, y, a in [(1, 1, 0), (-1, 1, math.pi / 2),
                            (-1, -1, math.pi), (1, -1, math.pi * 1.5)]
            for i in range(13)]


def box(size, center):
    return {'shape': 'box', 'sizeMm': size, 'centerMm': center}


def plate(name, outline, holes, depth, z, color, cuts=(), encoder=None):
    part = {'name': name, 'role': 'shell', 'color': color,
            'geometry': {'shape': 'plate', 'outlineMm': outline, 'holesMm': holes,
                         'depthMm': depth, 'centerZMm': z, 'subtract': list(cuts)},
            'basis': 'Printed manufacturer outer dimensions; hollow presentation only. '
                     'Wall/cap thickness 1.5mm and unannotated split/opening dimensions are estimates, '
                     'not a measured enclosure interior, connector mating interface or manufacturing CAD.'}
    if encoder:
        part['encoderId'] = encoder
    return part


def repair(suffix):
    product_id = 'waveshare-esp32-s3-' + suffix
    directory = ASSETS / product_id
    path = directory / 'model-plan.json'
    plan = json.loads(path.read_text())
    previous = plan.get('enclosureReconstruction')
    if previous:
        original = previous['originalEnvelope']
        photos = copy.deepcopy(previous['originalPhotoParts'])
    else:
        candidates = [p for p in plan['parts'] if p.get('role') == 'shell'
                      and p.get('geometry', {}).get('shape') in ('box', 'cylinder')]
        assert len(candidates) == 1
        original = copy.deepcopy(candidates[0])
        photos = copy.deepcopy([p for p in plan['parts'] if p is not candidates[0]])
    geometry = original['geometry']
    rounded = geometry['shape'] == 'cylinder'
    w, h, d = ([geometry['radiusMm'] * 2] * 2 + [geometry['depthMm']]
               if rounded else geometry['sizeMm'])
    radius = {'touch-lcd-7c-box': 9.08, 'epaper-1-54g': 4.5,
              'lcd-1-54': 5.8, 'lcd-0-85': 5.8, 'touch-amoled-2-16': 5.8}.get(suffix, 0)
    outer = circle(w / 2) if rounded else rect(w, h, radius=radius)
    inner = circle(w / 2 - 1.5) if rounded else rect(w - 3, h - 3, radius=max(0, radius - 1.5))
    screen = plan['presentation']['screen']
    sw, sh = screen['sizeMm']
    sx, sy = screen['positionMm'][:2]
    aperture = circle(sw / 2, (sx, sy)) if screen['shape'] == 'circle' else rect(sw, sh, (sx, sy), screen.get('cornerRadiusMm', 0))
    front = [p for p in photos if not p.get('geometry', {}).get('back')]
    back = [p for p in photos if p.get('geometry', {}).get('back')]
    corrected_bounds = {'lcd-0-85': [123, 150, 379, 410], 'epaper-1-54g': [167, 83, 496, 526]}
    if suffix in corrected_bounds:
        left, top, right, bottom = corrected_bounds[suffix]
        texture = front[0]['texture']
        source = next(s for s in plan['sources'] if s['file'] == texture['file'])
        px, py = source['imagePixels']
        scale_x, scale_y = (right - left) / w, (bottom - top) / h
        center_x, center_y = (left + right) / 2, (top + bottom) / 2
        texture['boundsMm'] = [-center_x / scale_x, (center_y - py) / scale_y,
                               (px - center_x) / scale_x, center_y / scale_y]
        plan.setdefault('photoRegistration', {})['correctedFrontBoundsPixels'] = corrected_bounds[suffix]
        plan['photoRegistration']['correctionBasis'] = 'Visual comparison of unchanged dimension image: prior source-to-model translation exceeded its stated tolerance. Re-register printed outer contour; approximately +/-1.5mm uncertainty remains.'
    # Preserve the larger source optical opening, where logical pixels are
    # aspect-fitted inside it (e.g. LR1121 OLED 37.5x20 mm opening).
    holes = copy.deepcopy(front[0]['geometry'].get('holesMm', [aperture]))
    if screen.get('cornerRadiusMm'):
        holes = [aperture]
        front[0]['geometry']['holesMm'] = holes
    assert holes
    side_cuts, rear_holes, observations = [], [], []
    if suffix == 'touch-lcd-4b':
        side_cuts = [box([5, 10, 4], [w / 2, y, .7]) for y in [1.4, -13.9]]
        rear_holes = [rect(20.7, 4.6, (6.8, 28.9)), rect(8.9, 6.3, (-22.3, -25.5))]
        observations += ['Retained dimension image shows two right-side USB mouths and rear expansion/battery mouths. '
                         'Their registered positions and clearances are photo estimates (about +/-1.5mm), with no socket interior invented.']
    elif suffix in ('lcd-1-54', 'lcd-0-85', 'touch-amoled-2-16'):
        side_cuts = [box([10, 5, 4], [0, -h / 2, d / 2 - 8.6]),
                     box([5, 13, 2], [-w / 2, 0, -d / 2 + 8.3])]
        observations += ['Dimension images identify side TF slot and lower USB-C. Slot and USB widths are approximate; '
                         'the annotated 8.3/8.6mm depth references are retained where printed. Button bores and all socket internals remain unknown.']
    elif suffix == 'lr1121-oled-1-54':
        side_cuts = [box([9, 5, 4], [0, -h / 2, 0]),
                     box([8, 5, 5], [-14, -h / 2, -1]), box([8, 5, 5], [14, -h / 2, -1])]
        observations += ['Official bottom view documents USB-C and two PIN OUT mouths at 14mm/28mm spacing. '
                         'Mouth sizes/depth and microphone grille details remain photo estimates or omitted. RF mating geometry is not invented.']
    elif suffix == 'touch-lcd-7c-box':
        side_cuts = [box([6, 11, 3], [w / 2, -9, -4])]
        rear_holes = [rect(58, 12, (0, -44))]
        observations += ['Official side view shows TF and rear image shows the green terminal opening. '
                         'Approximate clearance openings do not establish connector depth or the stepped rear moulding.']
    elif suffix == 'epaper-1-54g':
        side_cuts = [box([5, 13, 2], [w / 2, 10, 0])]
        rear_holes = [rect(6, 16.5, (12.9, -2.15))]
        observations += ['Right-side TF slot from dimension photograph; rear header opening uses annotated 6x16.5mm '
                         'and 4mm right/16.1mm bottom margins. Socket contacts and switch moulding are not reconstructed.']
    elif suffix == 'touch-amoled-1-43c':
        side_cuts = [box([10, 6, 4], [0, -h / 2, 0])]
        observations += ['Retained side image shows exposed USB at the lower perimeter; this opening is an uncalibrated '
                         'photo estimate. Exact side angles, microphone holes, button caps and seals remain unknown.']
    elif suffix in ('touch-lcd-1-85b', 'touch-amoled-1-75c'):
        observations += ['Retained side photograph shows closed button/USB-cover surfaces, not an open socket mouth. '
                         'Their exact travel, gasket, microscopic holes and internal mount are omitted rather than fabricated.']
    knob = suffix.startswith('knob-')
    if knob:
        side_cuts = [box([10, 9, 3.5], [-8, -30, -6.1])]
        observations += ['Front/side photographs show a colored upper rotating ring and fixed black lower base. '
                         'The -3mm split and USB angle/clearance are estimates, not an annotated mechanical assembly. '
                         'Only the upper side ring/front annulus expose the existing encoder input.']
    color = original['color']
    parts = []
    if knob:
        parts += [plate('case-side-ring', outer, [inner], d / 2 + 1.5, (d / 2 - 4.5) / 2, color,
                        encoder=original.get('encoderId')),
                  plate('case-fixed-base-wall', outer, [inner], d / 2 - 4.5, -(d / 2 + 1.5) / 2,
                        '#24282d', side_cuts)]
    else:
        parts += [plate('case-side-ring', outer, [inner], d - 3, 0, color, side_cuts)]
    parts += [plate('case-front-annulus', outer, holes, 1.5, d / 2 - .75, color,
                    encoder=original.get('encoderId')),
              plate('case-rear-cap', outer, rear_holes, 1.5, -d / 2 + .75,
                    '#24282d' if knob else color)]
    for part in photos:
        part['role'] = 'component'  # A source photograph is never recolored by shell presets.
        if part in back:
            part['geometry']['outlineMm'] = outer
            part['geometry']['holesMm'] = rear_holes
        else:
            part['geometry']['outlineMm'] = outer
        parts.append(part)
    plan['parts'] = parts
    derived = plan.setdefault('derivedParts', [])
    pcb = next((p for p in derived if p['kind'] == 'pcb-placeholder'), None)
    if pcb is None:
        pcb = {'id': 'pcb-inner-unknown', 'kind': 'pcb-placeholder',
               'geometry': {'sizeMm': [w - 10, h - 10, 1], 'centerMm': [0, 0, 0], 'labelSide': 'front'},
               'color': '#596767', 'basis': 'UNKNOWN hidden PCB: size/thickness are inspection placeholders only; no sourced layout or assembly-fit claim.'}
        if rounded:
            pcb['geometry']['shape'] = 'disc'
        derived.append(pcb)
    pcb['geometry']['labelSide'] = 'front'
    side_names = [p['name'] for p in parts if p['name'] in ('case-side-ring', 'case-fixed-base-wall')]
    source_ids = [s['id'] for s in plan['sources'] if s.get('imagePixels')]
    plan['presentation']['inspectionGroups'] = [
        {'id': 'front-cap-inspection', 'parts': ['case-front-annulus', 'live-display'] + [p['name'] for p in front]
         + ['cover-key:' + b['id'] for b in plan['presentation'].get('buttons', [])],
         'offsetMm': [w + 15, 0, 12], 'sourceIds': source_ids,
         'notes': 'Illustrative front-cap separation reveals UNKNOWN PCB. The displacement is not assembly travel.'},
        {'id': 'rear-cap-inspection', 'parts': ['case-rear-cap'] + [p['name'] for p in back],
         'offsetMm': [-w - 15, 0, -12], 'sourceIds': source_ids,
         'notes': 'Independent rear-cap separation; hollow walls remain in place. Exact internal mounting is unknown.'},
    ]
    plan['validation']['shellNames'] = [p['name'] for p in parts if p['role'] == 'shell']
    plan['validation']['unknownPcbInspection'] = [{'label': pcb['id'] + '-unknown-front'}]
    plan['validation']['hollowEnclosure'] = {
        'wallParts': side_names, 'frontCap': 'case-front-annulus', 'rearCap': 'case-rear-cap',
        'cavityPointMm': [0, 0, 0], 'aperturePointMm': [sx, sy, 0], 'wallMmEstimate': 1.5,
        'sourceIds': source_ids,
    }
    plan['dimensionTargets'] = [t for t in plan.get('dimensionTargets', []) if t.get('part') != original['name']]
    plan['dimensionTargets'] += [{'part': 'case-front-annulus', 'sizeMm': [w, h, 1.5], 'toleranceMm': .001},
                                 {'part': 'case-rear-cap', 'sizeMm': [w, h, 1.5], 'toleranceMm': .001}]
    plan['limitations'] = [n for n in plan['limitations'] if not n.startswith('Hollow repair:')]
    plan['limitations'] += ['Hollow repair: Previous solid-envelope representation is superseded. Exterior printed dimensions retained; '
                            '1.5mm wall/cap thickness, simplified corner/side profiles and unannotated port positions are explicit estimates. '
                            'Only visible openings are approximated; no screw threads, hidden electronics, battery, seals or connector mating model is invented.']
    plan['enclosureReconstruction'] = {'originalEnvelope': original, 'originalPhotoParts': photos,
                                       'sourceObservations': observations, 'wallMmEstimate': 1.5,
                                       'scope': 'Geometry correction, not physical hardware or browser acceptance.'}
    textured_front = next((p for p in front if p.get('texture', {}).get('boundsMm')), None)
    if textured_front:
        texture = textured_front['texture']
        source = next(s for s in plan['sources'] if s['file'] == texture['file'])
        px, py = source['imagePixels']
        left, bottom, right, top = texture['boundsMm']
        plan.setdefault('photoRegistration', {})['review'] = {
            'file': texture['file'],
            'pixelFromModelXY': [[px / (right - left), 0, -left * px / (right - left)],
                                 [0, -py / (top - bottom), top * py / (top - bottom)]],
            'includeParts': ['case-front-annulus'],
            'basis': 'Same explicitly declared source-pixel registration as the production front texture. '
                     'Overlay checks the exterior contour only; no hidden dimensions or independent camera calibration inferred.',
        }
    save(path, plan)
    save(directory / 'hollow-enclosure-review.json', {
        'model': product_id, 'observations': observations, 'wallMmEstimate': 1.5,
        'sources': [{'file': s['file'], 'sha256': s['sha256'], 'url': s['url']} for s in plan['sources'] if s.get('imagePixels')],
        'scope': plan['enclosureReconstruction']['scope'], 'sourceImagesModified': False,
    })
    return product_id


def qualify(records, model_ids):
    affected = set()
    for product in records:
        url = product.get('model', {}).get('url', '')
        if any('/' + identifier + '/' in url for identifier in model_ids):
            if NOTE not in product.setdefault('limitations', []):
                product['limitations'].append(NOTE)
            affected.add(product['id'])
    return affected


if __name__ == '__main__':
    models = [repair(suffix) for suffix in IDS]
    catalog_path = ROOT / 'web/platform/catalog/products.json'
    catalog = json.loads(catalog_path.read_text())
    affected = qualify(catalog['items'], models)
    save(catalog_path, catalog)
    for path in Path(__file__).parent.glob('*.json'):
        fragment = json.loads(path.read_text())
        if not isinstance(fragment, dict) or 'products' not in fragment:
            continue
        if qualify(fragment['products'], models):
            if NOTE not in fragment.setdefault('limitations', []):
                fragment['limitations'].append(NOTE)
            save(path, fragment)
    for product_id in affected:
        path = ROOT / 'web/platform/knowledge' / (product_id + '.json')
        record = json.loads(path.read_text())
        if NOTE not in record['collectionNotes']:
            record['collectionNotes'].append(NOTE)
        save(path, record)
    print(json.dumps({'models': models, 'products': sorted(affected)}, ensure_ascii=False))
