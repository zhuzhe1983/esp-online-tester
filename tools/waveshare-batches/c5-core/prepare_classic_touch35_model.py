#!/usr/bin/env python3
"""Dimension/photo exterior for the classic ESP32 Touch LCD3.5, not the C5 PCB."""
import argparse
import gzip
import hashlib
import json
import math
from pathlib import Path
from PIL import Image

ROOT = Path(__file__).resolve().parents[3]
RIGHTS = 'Original Waveshare sources retain manufacturer rights; public availability is not a separate redistribution licence. Geometry is an explicitly approximate reconstruction.'


def save(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')


def rounded(w, h, r=0, n=12):
    if not r:
        return [[-w/2, -h/2], [w/2, -h/2], [w/2, h/2], [-w/2, h/2]]
    points = []
    for x, y, start in [(w/2-r, h/2-r, 0), (-w/2+r, h/2-r, 90), (-w/2+r, -h/2+r, 180), (w/2-r, -h/2+r, 270)]:
        for i in range(n+1):
            angle = math.radians(start+i*90/n)
            points.append([x+r*math.cos(angle), y+r*math.sin(angle)])
    return points


def open_ring(w, h, r, wall, gap):
    outer, inner = rounded(w,h,r), rounded(w-2*wall,h-2*wall,r-wall)
    # Both sampled contours have13 points per corner. Leave the USB-facing
    # central bottom wall open; the depth of its actual window is unmeasured.
    return [[gap/2,-h/2]]+outer[39:]+outer[:39]+[[-gap/2,-h/2],[-gap/2,-h/2+wall]]+inner[38::-1]+inner[:38:-1]+[[gap/2,-h/2+wall]]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source-root', type=Path, required=True)
    a = parser.parse_args()
    name = 'waveshare-esp32-touch-lcd-3-5'
    out = ROOT / 'web/platform/assets' / name
    out.mkdir(parents=True, exist_ok=True)
    sources = []

    def retain(source_id, path, filename, role='reference-only'):
        body = path.read_bytes()
        receipt = json.loads((path.parent / ('tree.receipt.json' if path.name == 'tree.json' else path.name + '.receipt.json')).read_text())
        digest = hashlib.sha256(body).hexdigest()
        assert len(body) == receipt['bytes'] and digest == receipt['sha256']
        compressed = filename.endswith('.gz')
        stored = gzip.compress(body, mtime=0) if compressed else body
        (out / filename).write_bytes(stored)
        row = {'id': source_id, 'url': receipt['requestedUrl'], 'finalUrl': receipt['finalUrl'],
               'fetchedAt': receipt['fetchedAt'], 'bytes': len(body), 'sha256': digest,
               'file': filename, 'role': role, 'rights': RIGHTS,
               'contentType': receipt.get('contentType', 'application/octet-stream')}
        if compressed:
            row['retained'] = {'encoding': 'gzip', 'bytes': len(stored), 'sha256': hashlib.sha256(stored).hexdigest()}
        elif path.suffix == '.jpg':
            row['imagePixels'] = list(Image.open(path).size)
        sources.append(row)

    photo_root = a.source_root / 'esp32-touch-lcd-3.5'
    source_root = a.source_root / 'ESP32-Touch-LCD-3.5'
    for sid, original in [('dimension-photo', 'ESP32-Touch-LCD-3.5-details-size.jpg'),
                          ('assembled-photo', 'ESP32-Touch-LCD-3.5-details-1.jpg'),
                          ('pcb-photo', 'ESP32-Touch-LCD-3.5-details-inter-1.jpg')]:
        retain(sid, photo_root / original, sid + '.jpg', 'photo-source')
    retain('overview', photo_root / 'overview.html', 'overview.html.gz')
    retain('repository-tree', source_root / 'tree.json', 'repository-tree.json.gz')
    retain('repository-readme', source_root / 'README.md', 'README.md.gz')
    retain('schematic-v1-2', source_root / 'hardware__schematic__ESP32-Touch-LCD-3.5-Schematic-V1.2.pdf', 'schematic-v1-2.pdf.gz')

    # Display portrait +Z. All dimensions below are in millimetres. The
    # published 95.11 x 63.67 landscape drawing is rigidly rotated into portrait.
    w, h = 63.67, 95.11
    aa = [48.96, 73.44]
    front, back = rounded(w, h, 7.30), rounded(59.50, 83.20, 4.8)
    def plate(pid, outline, depth, z, color, holes=None, role=None, basis=None):
        value = {'id': pid, 'kind': 'plate', 'color': color,
                 'geometry': {'outlineMm': outline, 'holesMm': holes or [], 'depthMm': depth, 'centerMm': [0, 0, z]},
                 'sourceIds': ['dimension-photo'], 'basis': basis or 'Published XY envelope; wall thickness and inner radius are explicit visual estimates.'}
        if role:
            value['role'] = role
        return value

    # Open ring and separate rear floor preserve an actual cavity. The three
    # edge controls and USB mouth remain outside this conservative inner wall;
    # no claim is made about measured wall thickness, clip or screw internals.
    derived = [
        plate('front-bezel', front, 2.09, -1.045, '#16191a', [rounded(*aa)], 'shell',
              'Outer 63.67 x 95.11, corner R7.30, front lip2.09 and screen48.96 x73.44 from the original size drawing.'),
        plate('back-shell-wall', open_ring(59.50,83.20,4.8,1.5,10), 10.31, -7.245, '#26282a', [], 'shell',
              'Published rear envelope;1.5 wall and10 wide full-depth USB gap are conservative visual estimates. Actual window height/hidden lip remain unknown; USB mouth is not sealed by a solid ring.'),
        plate('back-shell-floor', back, 1.1, -12.95, '#26282a', [[[x+24.0,y] for x,y in rounded(7.0,43.0)]], 'shell',
              'Case depth13.50 and feet total14.10 from size drawing; floor1.10 is unmeasured. Rear header aperture is photo positioned at the case edge, separate from the paper label.'),
    ]
    # The observed blue C-shaped PCB face establishes its topology, not its
    # dimensions. The envelope is an explicitly estimated fit inside the case.
    x0, y0, x1, y1 = 180, 97, 778, 517
    def board_xy(x, y):
        lx = (x-x0)/(x1-x0)*80-40
        ly = 28-(y-y0)/(y1-y0)*56
        return [-ly, -lx]  # rear photo is viewed from -Z; horizontal mirror
    outline = [board_xy(x, y) for x, y in [(180,97),(778,97),(778,235),(532,235),(532,445),(778,445),(778,517),(180,517)]]
    derived.append({'id': 'pcb-main', 'kind': 'pcb-placeholder', 'color': '#174c72',
                    'geometry': {'shape': 'polygon', 'sizeMm': [56,80,1.6], 'outlineMm': outline,
                                 'holesMm': [], 'depthMm': 1.6, 'centerMm': [0,0,-7.5],
                                 'labelSide': 'front', 'labelSizeMm': [34,5], 'labelOffsetMm': [0,27]},
                    'sourceIds': ['pcb-photo'], 'basis': 'Blue C-shaped silhouette is photographed. Envelope56 x80, thickness1.6 and position are estimated; layout, unseen front and component heights UNKNOWN.'})
    iw, ih = Image.open(photo_root / 'ESP32-Touch-LCD-3.5-details-inter-1.jpg').size
    # x_pixel = midpoint_x - Y * pixel_width /80; y_pixel = midpoint_y + X * pixel_height /56.
    derived.append({'id':'pcb-rear-photo','kind':'photo-surface','texture':'pcb-photo.jpg',
                    'geometry': {'outlineMm': outline, 'centerMm':[0,0,-8.312], 'side':'back',
                                 'uvFromMm':[0,-(x1-x0)/80/iw,(x0+x1)/2/iw,-(y1-y0)/56/ih,0,1-(y0+y1)/2/ih]},
                    'sourceIds':['pcb-photo'],'basis':'Unchanged exact classic-board photo clipped to its observed C-shaped edge; annotations outside the board are excluded. No depth or PCB layout claim.'})
    # A separately supported real paper label. This rectangular crop stops
    # before the physical header, feet and case rim; no shell pixels are decal.
    dw, dh = Image.open(photo_root / 'ESP32-Touch-LCD-3.5-details-size.jpg').size
    label_w, label_h = (991-787)/338*63.67, (648-322)/508*95.11
    label_center = [0.8, -0.75, -13.512]
    derived.append({'id':'rear-paper-label','kind':'photo-surface','texture':'dimension-photo.jpg',
                    'geometry':{'outlineMm':rounded(label_w,label_h),'centerMm':label_center,'side':'back',
                                'uvFromMm':[0,-(648-322)/label_h/dw,(322+648)/2/dw,-(991-787)/label_w/dh,0,1-(787+991)/2/dh]},
                    'sourceIds':['dimension-photo'],'basis':'Independent original paper-label pixels [322,787,648,991]; approximate placement on rear floor, without rim, feet, aperture or invented printing.'})

    parts = []
    for x in [-24.25,24.25]:
        for y in [-36,36]:
            parts.append({'name':f'foot-{x}-{y}','role':'component','color':'#202223',
                          'geometry':{'shape':'cylinder','radiusMm':3.5,'depthMm':.6,'centerMm':[x,y,-13.8]},
                          'sourceIds':['dimension-photo'],'basis':'Four visible feet and72 x48.50 centre spacing come from original rear view. Radius3.5 is photo estimated;0.60 equals annotated14.10 overall minus13.50 shell depth. No hidden screw or support length inferred.'})
    for pid, y, control in [('power-key',25.1,None),('reset-key',16.5,None),('boot-key',7.9,'boot')]:
        p = {'name':pid,'role':'control' if control else 'component','color':'#16191a',
             'geometry':{'shape':'box','sizeMm':[1.6,4.8,2.0],'centerMm':[-30.2,y,-8.5]},
             'sourceIds':['dimension-photo','pcb-photo'],'basis':'Side-view location is photo registered; cap dimensions and insertion depth are estimates. Only BOOT is a modeled input.'}
        if control:
            p['controlId'] = control
        parts.append(p)
    parts.append({'name':'usb-shell','role':'connector','color':'#a7adae',
                  'geometry':{'shape':'open-usb-shell','sizeMm':[8.95,6.4,3.2],'centerMm':[0,0,0],'wallMm':.3},
                  'matrixRows':[[1,0,0,0],[0,1,0,-38.5],[0,0,1,-8.1],[0,0,0,1]],
                  'sourceIds':['dimension-photo','pcb-photo'],'basis':'Open approximate USB envelope; mouth/depth/height are visual estimates, no verified mating anchor or electrical cable connection.'})

    limits = [
        'No exact official CAD, KiCad PCB, embedded footprint STEP or Gerber exists in the inspected pinned product repository tree; this is a bounded dimension/photo reconstruction.',
        'Outer dimensions, front lip and display active area are manufacturer annotations. Internal wall, back radius, PCB thickness/position, keys, USB and hidden details are explicit estimates.',
        'The C-shaped blue board belongs to the classic ESP32 Touch LCD3.5. No C5, S3 or P4 PCB has been borrowed.',
        'Only the observed rear PCB face is photographed. The opposite face and actual component heights remain UNKNOWN; a photograph is not an electrical layout.',
        'The live framebuffer occupies an open front aperture; no factory screenshot or decorative artwork covers it.',
        'Battery/no-battery SKUs share the documented case. The battery, speaker cavity, internal fasteners and interconnects are not reconstructed.',
        'Physical PWR and RESET caps are visual references. BOOT is the sole clickable cap; no simulated power-off is inferred from a GPIO animation.',
        'No browser WebGL, physical fitting, exact optical material, connector mating, or real hardware validation is asserted.'
    ]
    plan = {'id':name,'method':'dimension-photo-reconstruction','sources':sources,'parts':parts,'derivedParts':derived,
            'presentation':{'screen':{'shape':'rectangle','sizeMm':aa,'positionMm':[0,0,.012],
                                      'basis':'Original annotated active area48.96 x73.44;320 x480 pixel ratio exact.'},
                            'inspectionGroups':[{'id':'front-optics','parts':['front-bezel','live-display'],'offsetMm':[40,0,18],'sourceIds':['dimension-photo'],'notes':'Debug removal exposes UNKNOWN front PCB; not a claimed assembly opening motion.'},
                                                {'id':'rear-case','parts':['back-shell-wall','back-shell-floor','rear-paper-label'],'offsetMm':[-40,0,-18],'sourceIds':['dimension-photo'],'notes':'Debug separation reveals original clipped PCB reference face.'}]},
            'sourceSearch':{'status':'no-official-cad-located','inspected':['overview','repository-tree','repository-readme'],
                            'repositoryCommit':'51f354d2aeb2245fd7b748c9f3ea93cea1c527ae','scope':'Complete nontruncated pinned tree, product overview and linked hardware directory.'},
            'dimensionTargets':[{'part':'front-bezel','sizeMm':[w,h,2.09],'toleranceMm':.001,'sourceId':'dimension-photo'},
                                {'part':'screen','sizeMm':aa+[None],'toleranceMm':.001,'sourceId':'dimension-photo'}],
            'validation':{'controls':[{'id':'boot','type':'button'}],'controlViewNormals':{'boot':[-1,0,0]},'ledIndices':[],
                          'shellNames':['front-bezel','back-shell-wall','back-shell-floor'],'display':{'width':320,'height':480},
                          'unknownPcbInspection':[{'label':'pcb-main-unknown-front'}]},
            'referencePhotos':[{'sourceId':s['id'],'file':s['file'],'views':'Original manufacturer image, unchanged.'} for s in sources if 'imagePixels' in s],
            'photoRegistration':{'pcbRearPixelBounds':[x0,y0,x1,y1],'pcbOutlineIncludesCutout':True,
                                 'pcbEnvelopeMm':[56,80],'pcbEnvelopeBasis':'Estimated inside published case; not a PCB dimension annotation',
                                 'paperPixelBounds':[322,787,648,991],'sourceImagesUnchanged':True,
                                 'review':{'file':'dimension-photo.jpg',
                                           'pixelFromModelXY':[[0,508/95.11,361],[338/63.67,0,276]],
                                           'includeParts':['front-bezel'],
                                           'basis':'Original annotated front envelope pixels107..615,107..445; rigid portrait/landscape registration. Hidden geometry and side control locations are not validated by this projection.'}},
            'limitations':limits,'materialBasis':'Original case photo and clipped PCB/label image; neutral non-calibrated material elsewhere.',
            'assemblyNotes':'Native millimetre reconstruction; front+Z, portrait. Source landscape drawing rigidly rotated, no sibling CAD scaling.'}
    save(out/'model-plan.json',plan)
    save(out/'source-manifest.json',{'schemaVersion':1,'id':name,'sources':sources,'limitations':limits,'search':plan['sourceSearch']})
    (out/'SOURCE-RIGHTS.txt').write_text(RIGHTS+'\n')
    print(name)


if __name__ == '__main__':
    main()
