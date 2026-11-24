#!/usr/bin/env python3
"""Prepare exact manufacturer CAD plans; rebuilds subsequently use retained sources only."""
import argparse
import gzip
import hashlib
import json
from pathlib import Path
from PIL import Image

ROOT = Path(__file__).resolve().parents[3]
ASSETS = ROOT / 'web/platform/assets'

def sha(data):
    return hashlib.sha256(data).hexdigest()

def write(path, data):
    path.write_text(json.dumps(data, ensure_ascii=False, indent=2) + '\n')

def color(linear):
    return '#' + ''.join(f'{round(255 * (12.92 * c if c <= .0031308 else 1.055 * c ** (1 / 2.4) - .055)):02x}' for c in linear)

def prepare(cache, family, model, cad, drawing, photos, config):
    folder = cache / family
    output = ASSETS / model
    output.mkdir(parents=True, exist_ok=True)
    sources = []
    containers = {}

    def retain(path, name, sid, role, compressed=False):
        receipt = json.loads(Path(str(path) + '.receipt.json').read_text())
        original = path.read_bytes()
        assert sha(original) == receipt['sha256'] and len(original) == receipt['bytes']
        parent = receipt.get('sourceArchive', receipt)
        record = {'id': sid, 'file': name, 'role': role,
                  'url': parent['requestedUrl'], 'resolvedUrl': parent['finalUrl'],
                  'fetchedAt': parent['fetchedAt'], 'bytes': len(original), 'sha256': sha(original),
                  'mediaType': 'model/step' if role == 'cad-source' else 'application/pdf' if path.suffix == '.pdf' else parent.get('contentType', 'application/octet-stream')}
        if 'sourceArchive' in receipt:
            key = parent['sha256']
            if key not in containers:
                candidates = [p for p in folder.glob('*.zip') if p.stat().st_size == parent['bytes']]
                container = next(p for p in candidates if sha(p.read_bytes()) == key)
                containers[key] = 'container-' + key[:12]
                retain(container, containers[key] + '.zip', containers[key], 'manufacturer-source-container')
            record.update(origin='zip-member', containerSourceId=containers[key],
                          archive={'format': 'zip', 'entry': receipt['archiveMember'],
                                   'containerSha256': key, 'containerBytes': parent['bytes']})
        data = gzip.compress(original, mtime=0) if compressed else original
        (output / name).write_bytes(data)
        if compressed:
            record['retained'] = {'encoding': 'gzip', 'bytes': len(data), 'sha256': sha(data)}
        if path.suffix in ('.webp', '.jpg', '.png'):
            record['imagePixels'] = list(Image.open(path).size)
        sources.append(record)
        return record

    cad_path = folder / 'mechanical' / cad
    inspection = json.loads(Path(str(cad_path) + '.inspection.json').read_text())
    retain(cad_path, 'source.stp.gz', 'official-cad', 'cad-source', True)
    retain(folder / drawing, 'mechanical-drawing.pdf', 'mechanical-drawing', 'dimension-reference')
    for index, photo in enumerate(photos):
        retain(folder / photo, f'reference-{index}{Path(photo).suffix}', f'photo-{index}', 'manufacturer-photograph')
    special = config['parts']
    omitted = config.get('omittedIndices', [])
    consumed = {i for p in special for i in p['indices']} | set(omitted)
    parts = []
    for p in special:
        record = {k: v for k, v in p.items() if k != 'indices'}
        record.update(sourceFile='source.stp.gz', sourceComponents=p['indices'])
        record.setdefault('basis', 'Original manufacturer source component(s), unscaled and in original assembly coordinates.')
        parts.append(record)
    groups = {}
    for component in inspection['components']:
        if component['index'] in consumed:
            continue
        material = color(component['sourceColorLinear']) if 'sourceColorLinear' in component else '#888888'
        groups.setdefault(material, []).append(component['index'])
    for index, (material, indices) in enumerate(sorted(groups.items())):
        parts.append({'name': f'native-components-{index:02d}', 'role': 'component',
                      'sourceFile': 'source.stp.gz', 'sourceComponents': indices, 'color': material,
                      'basis': 'Native STEP source color converted from linear to sRGB; absent color uses neutral grey. All solids and free shells/faces retained.'})
    n = inspection['nativeComponents']
    plan = {'schemaVersion': 1, 'id': model, 'method': 'official-cad', 'sources': sources,
            'sourceTopologyCounts': {'source.stp.gz': n},
            'sourceCoverage': {'source.stp.gz': {'includedIndices': [i for i in range(n) if i not in omitted], 'omittedIndices': omitted}},
            'parts': parts, 'derivedParts': [],
            'materialBasis': 'Native CAD colors where available; optical black follows the retained exact-family photograph. Grey means source material/color is unknown, not a measured finish.',
            'assemblyNotes': 'Original millimetre coordinates and all source topology retained without scaling. Inspection separation is illustrative, not a mechanical travel or assembly fit claim.',
            'sourceSearch': {'priority': 'Manufacturer native CAD', 'scope': 'Exact official resources and their source archives retained in this model; alternate hardware revision correspondence is stated separately.'},
            'limitations': config['limitations'], 'presentation': config['presentation'],
            'dimensions': config['dimensions'], 'validation': config['validation'],
            'dimensionTargets': config['dimensionTargets'], 'photoRegistration': config['photoRegistration'],
            'tessellation': {'deflectionMm': .05, 'angleRadians': .4}}
    write(output / 'model-plan.json', plan)
    write(output / 'source-manifest.json', {'schemaVersion': 1, 'sources': sources})
    (output / 'SOURCE-RIGHTS.txt').write_text('Manufacturer CAD, drawings and photographs: Waveshare and their respective owners retain rights. Public download access does not establish a general redistribution licence. Originals are unchanged; no manufacturer firmware is redistributed. Generated model plans and verification scripts follow this repository licence.\n')
    print(model, n, 'native components', len(parts), 'draw groups', flush=True)

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cache', type=Path, required=True)
    args = parser.parse_args()
    prepare(args.cache, 'ESP32-S3-LCD-1.69', 'waveshare-esp32-s3-lcd-1-69',
            'ESP32-S3-Lcd-1_69_3D.stp', 'ESP32-S3LCD-1_69.pdf',
            ['images/ESP32-S3-LCD-1.69-details-size-15ed68e1d045a9474bad27533c1aa320.webp'], {
        'parts': [
            {'name': 'native-optical-assembly', 'role': 'panel', 'indices': [0], 'color': '#222222', 'basis': 'Exact source LCD/FPC topology; black optical appearance from retained manufacturer photo, no static image over the framebuffer.'},
            {'name': 'native-pcb', 'role': 'pcb', 'indices': [5], 'color': '#173961'},
            {'name': 'native-reset', 'role': 'component', 'indices': [269], 'color': '#fffff2'},
            {'name': 'native-boot', 'role': 'component', 'indices': [270], 'color': '#fffff2', 'controlId': 'boot'},
            {'name': 'native-power', 'role': 'component', 'indices': [271], 'color': '#fffff2', 'controlId': 'power'},
            {'name': 'native-usb-shell', 'role': 'component', 'indices': [60], 'color': '#c9c9c9'}],
        'limitations': ['Manufacturer mechanical CAD is unversioned. It establishes this family assembly, not independent V1/V2 layout equivalence; electrical revisions have separate runtime profiles.', 'PCB and LCD source bounds are retained exactly, including FPC and connector protrusions. No synthetic case or hidden electronics added. Physical pixel orientation, switch actuation travel and connector mating depth are not hardware calibrated.', 'RESET is retained as native geometry; BOOT and digital PWR have sourced GPIO interactions. Analog power latch, buzzer sound and battery charging are not simulated.'],
        'presentation': {'screen': {'shape': 'rounded-rectangle', 'sizeMm': [27.972, 32.634], 'cornerRadiusMm': 5, 'positionMm': [0, 0, 1.58], 'physicalBasis': 'active-area', 'activeAreaKnown': True, 'basis': 'Official LCD drawing labels AA27.97x32.63; manufacturer dimensioned photo states27.972x32.634 and four diameter10mm corners. Glass lies above native Z1.56 at Z1.58.'}, 'inspectionGroups': [{'id': 'optical-inspection', 'parts': ['native-optical-assembly', 'live-display'], 'offsetMm': [45, 0, 15], 'sourceIds': ['official-cad', 'mechanical-drawing'], 'notes': 'LCD and live framebuffer move together; original coordinates restore.'}]},
        'dimensions': {'widthMm': 30, 'heightMm': 37.1, 'depthMm': 7.3, 'basis': 'PCB width/height from original drawing; native whole assembly Z envelope7.3mm, not an exterior enclosure specification.'},
        'validation': {'display': {'width': 240, 'height': 280}, 'controls': [{'id': 'boot', 'type': 'button'}, {'id': 'power', 'type': 'button'}], 'ledIndices': [], 'shellNames': [], 'controlViewNormals': {'boot': [1, 0, 0], 'power': [1, 0, 0]}},
        'dimensionTargets': [{'part': 'native-pcb', 'sizeMm': [30.0208, 37.1419, 1], 'toleranceMm': .05}],
        'photoRegistration': {'review': {'file': 'reference-0.webp', 'pixelFromModelXY': [[-430/30, 0, 458.5], [0, -532/37.1, 1026.1]], 'includeParts': ['native-pcb', 'native-boot', 'native-power', 'native-reset', 'native-usb-shell'], 'basis': 'Rear view flips source X. Dimensioned PCB photo approximatelyx243..673,y776..1308 with native PCB centerY-1.18956. Registration is photographic comparison, not height/mating acceptance.'}}
    })
    for curved in [False, True]:
        model = 'waveshare-esp32-s3-touch-lcd-2-1' + ('b' if curved else '')
        cad = 'ESP32-S3-Touch-LCD-2.1B_3d-20260523.stp' if curved else 'ESP32-S3-Touch-LCD-2_1_asm.stp'
        drawing = 'mechanical/' + ('ESP32-S3-Touch-LCD-2.1B-═Γ╨╬│▀┤τ═╝-20260523.pdf' if curved else 'ESP32-S3-Touch-LCD-2.1 尺寸图.pdf')
        pcb, boot = (2, 505) if curved else (15, 6)
        outer, optical, z = (71.8, 53.48, 4.72) if curved else (75, 53.28, 3.42)
        prepare(args.cache, 'ESP32-S3-Touch-LCD-2.1', model, cad, drawing,
                ['images/ESP32-S3-Touch-LCD-2.1-product2-966ae1dc22d40fe1a65507af72f2b876.webp', 'images/ESP32-S3-Touch-LCD-2.1-product3-5f3a60f01d37c961cc26b890c4eddfc4.webp'] + ([] if curved else ['images/flat-active-area-original.jpg']), {
            'parts': [{'name': 'native-lcd-back', 'role': 'component', 'indices': [0], 'color': '#555555'}, {'name': 'native-cover-glass', 'role': 'panel', 'indices': [1], 'color': '#111111'}, {'name': 'native-pcb', 'role': 'pcb', 'indices': [pcb], 'color': '#888888' if curved else '#173961'}, {'name': 'native-boot', 'role': 'component', 'indices': [boot], 'color': '#cccccc', 'controlId': 'boot'}],
            'limitations': ['Separate exact flat and curved manufacturer CAD files are used; all native solids/free shells/faces and original millimetre transforms remain intact.', 'Curved cover geometry comes from the actual2.1B STEP. Its flat framebuffer projection is an explicitly uncalibrated optical approximation, not curved lens refraction or per-pixel surface calibration.' if curved else 'Flat source CAD visible window is53.88mm; the retained exact official dimensioned photograph labels53.28mm display area. The live framebuffer uses53.28mm and does not fill the larger VA. Native CAD lacks the upper rear square IC body visible in the family photo; source-component completeness does not certify a complete physical BOM.', 'No inferred external case, antenna behaviour, battery chemistry, power-switch electronics or connector mating calibration. Uncolored native components remain neutral grey. Photographic PCB comparison is a visible mechanical reference, not proof of every electrical revision.'],
            'presentation': {'screen': {'shape': 'circle', 'sizeMm': [optical, optical], 'positionMm': [0, 0, z], 'physicalBasis': 'active-area', 'activeAreaKnown': True, 'basis': 'Exact manufacturer sources: curved AA53.48/VA54.08; flat display53.28 from the dimensioned photograph, separately from CAD VA53.88. Native glass top+0.02mm plane; physical calibration remains unverified.'}, 'inspectionGroups': [{'id': 'optical-inspection', 'parts': ['native-lcd-back', 'native-cover-glass', 'live-display'], 'offsetMm': [90, 0, 20], 'sourceIds': ['official-cad', 'mechanical-drawing'], 'notes': 'LCD, glass and live screen share one illustrative rigid separation.'}]},
            'dimensions': {'widthMm': outer, 'heightMm': outer, 'depthMm': 10.8 if curved else 9.5, 'basis': 'Exact native cover and assembled source envelope, without scale fitting.'},
            'validation': {'display': {'width': 480, 'height': 480}, 'controls': [{'id': 'boot', 'type': 'button'}], 'ledIndices': [], 'shellNames': [], 'controlViewNormals': {'boot': [1, 0, 0]}, 'screenAspectRelativeTolerance': .001, 'screenAspectSourceId': 'mechanical-drawing' if curved else 'photo-2', 'screenAspectBasis': 'The manufacturer dimensioned photograph labels53.28mm display area for the flat cover; the curved drawing labels53.48mm AA. Neither registration is hardware calibrated.'},
            'dimensionTargets': [{'part': 'native-cover-glass', 'sizeMm': [outer, outer, 3.5 if curved else 2.2], 'toleranceMm': .05}],
            'photoRegistration': {'review': {'file': 'reference-1.webp', 'pixelFromModelXY': [[-8.29, 0, 478.5], [0, -8.29, 469.2]], 'includeParts': ['native-pcb', 'native-boot'], 'basis': 'Rear PCB photo orientation flips X; sourced native PCB outline66mm compared against visible board width. Source photo is flat2.1; curved model PCB silhouette correspondence is a comparison only, not independent revision certification.'}}
        })

if __name__ == '__main__':
    main()
