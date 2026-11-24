#!/usr/bin/env python3
"""Create four new source-bounded Nano/Pico model plans, not old hash recovery.

Normally runs entirely from the sources retained in the four asset directories.
The optional --source-cache bootstraps those sources from recorded HTTP captures.
Run the shared tools/build-waveshare-models.py afterwards to export geometry.
"""
import argparse
import copy
import gzip
import hashlib
from html.parser import HTMLParser
import json
import math
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[5]
ASSETS = ROOT / 'web/platform/assets'
EVIDENCE = Path(__file__).resolve().parent
RIGHTS = ('Manufacturer photographs, diagrams and documents retain their original rights. '
          'Public availability is not an MIT redistribution licence. Original source bytes '
          'are retained unchanged (HTML may be losslessly gzip encoded). Reconstruction '
          'code is repository code; this does not relicense referenced manufacturer assets.')


def sha(data): return hashlib.sha256(data).hexdigest()


def save(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')


class Links(HTMLParser):
    def __init__(self): super().__init__(); self.links = []
    def handle_starttag(self, tag, attrs):
        if tag == 'a':
            href = dict(attrs).get('href')
            if href: self.links.append(href)


def sources(family, pid, cache):
    folder = ASSETS / pid
    folder.mkdir(parents=True, exist_ok=True)
    if cache is None:
        records = json.loads((folder / 'source-manifest.json').read_text())['sources']
        for item in records:
            data = (folder / item['file']).read_bytes()
            retained = item.get('retained', item)
            assert len(data) == retained['bytes'] and sha(data) == retained['sha256']
            raw = gzip.decompress(data) if item.get('retained', {}).get('encoding') == 'gzip' else data
            assert len(raw) == item['bytes'] and sha(raw) == item['sha256']
        return records
    records = []
    captures = []
    for filename in ['source-fetch.json', 'gallery-fetch.json']:
        captures.extend(json.loads((cache / filename).read_text())['sources'])
    selected = [s for s in captures if s.get('family') == family and s.get('httpStatus') == 200
                and 'Sensor-Kit' not in s['file']]
    for n, original in enumerate(selected):
        source_path = cache / original['file']
        data = source_path.read_bytes()
        assert len(data) == original['bytes'] and sha(data) == original['sha256']
        name = source_path.name
        sid = f'photo-{n:02d}'
        if name == 'docs-overview.html': sid = 'overview'
        elif name == 'store.html': sid = 'store'
        elif 'details-size-' in name: sid = 'dimension-photo'
        elif 'details-intro-' in name: sid = 'component-photo'
        elif '-M-' in name: sid = 'header-photo'
        elif name == {'nano': 'esp32-s3-nano-3.jpg', 'pico': 'esp32-s3-pico-3_5.jpg'}[family]: sid = 'front-photo'
        item = {k: v for k, v in original.items() if k not in ('family', 'file')}
        item.update(id=sid, file=name, requestedUrl=original['url'], status='fetched', rights=RIGHTS)
        if name.endswith('.html'):
            retained = gzip.compress(data, compresslevel=9, mtime=0)
            item['file'] += '.gz'
            item['retained'] = {'encoding': 'gzip', 'bytes': len(retained), 'sha256': sha(retained)}
        else:
            retained = data
            with Image.open(source_path) as image: item['imagePixels'] = list(image.size)
        (folder / item['file']).write_bytes(retained)
        records.append(item)
    # Keep the actual earlier retrieval time; this is not a new HTTP request.
    audit = json.loads((ROOT / f'docs/evidence/waveshare-s3-core-2026-11-23/sources-esp32-s3-{family}.json').read_text())
    original = next(s for s in audit['sources'] if s['url'].endswith('/Resources-And-Documents'))
    data = (ROOT / f'docs/research/waveshare-s3-core-sources/{family}-resources.html.gz').read_bytes()
    raw = gzip.decompress(data)
    assert len(raw) == original['bytes'] and sha(raw) == original['sha256']
    item = {k: v for k, v in original.items() if k != 'path'}
    item.update(id='resources-historical', file='resources-historical.html.gz', rights=RIGHTS,
                retained={'encoding': 'gzip', 'bytes': len(data), 'sha256': sha(data)})
    (folder / item['file']).write_bytes(data)
    records.append(item)
    return records


def circle(x, y, radius, count=32):
    return [[x + radius * math.cos(i * 2 * math.pi / count), y + radius * math.sin(i * 2 * math.pi / count)] for i in range(count)]


def rectangle(x, y, w, h, angle=0):
    c, s = math.cos(angle), math.sin(angle)
    return [[x + c*a - s*b, y + s*a + c*b] for a, b in [(-w/2,-h/2),(w/2,-h/2),(w/2,h/2),(-w/2,h/2)]]


def outline(w, h, rows, usb_notch=None):
    result = [[-w/2,-h/2],[w/2,-h/2]]
    radius = .60  # Visible castellated edge only; not an official drill dimension.
    for y in sorted(rows):
        for i in range(9):
            angle = -math.pi/2 + i*math.pi/8
            result.append([w/2-radius*math.cos(angle), y+radius*math.sin(angle)])
    result.append([w/2,h/2])
    if usb_notch:
        half, inner_y = usb_notch
        result.extend([[half,h/2],[half,inner_y],[-half,inner_y],[-half,h/2]])
    result.append([-w/2,h/2])
    for y in sorted(rows, reverse=True):
        for i in range(9):
            angle = math.pi/2 - i*math.pi/8
            result.append([-w/2+radius*math.cos(angle), y+radius*math.sin(angle)])
    return result


def build_plan(family, header, source_cache):
    pid = f'waveshare-esp32-s3-{family}' + ('-m' if header else '')
    folder = ASSETS / pid
    records = sources(family, pid, source_cache)
    by_id = {s['id']: s for s in records}
    nano = family == 'nano'
    width, height = (17.78,43.18) if nano else (21.,51.)
    pins, spacing, hole_radius = (15,15.24,.52) if nano else (20,17.78,.5)
    rows = [(pins-1)*2.54/2 - 2.54*i for i in range(pins)]
    holes = [circle(x,y,hole_radius) for x in [-spacing/2,spacing/2] for y in rows]
    if nano: holes += [circle(x,y,.83) for x in [-7.62,7.62] for y in [-20.32,20.32]]
    photo = by_id['front-photo']
    iw, ih = photo['imagePixels']
    # Independently picked PCB substrate edges in the unmodified top photograph.
    left, top, right, bottom = (627,117,1042,1121) if nano else (584,96,1015,1127)
    sx, sy = (right-left)/width, (bottom-top)/height
    cx, cy = (left+right)/2, (top+bottom)/2
    uv = [sx/iw,0,cx/iw,0,sy/ih,1-cy/ih]
    parts, derived, masks = [], [], []
    def photo_surface(name, polygon, z, holes_mm=()):
        derived.append({'id':name,'kind':'photo-surface','texture':photo['file'],
            'geometry':{'outlineMm':polygon,'holesMm':list(holes_mm),'sizeMm':[width,height,.001],
                        'centerMm':[0,0,z],'uvFromMm':uv,'side':'front'},
            'basis':'Unmodified manufacturer front photograph projected only onto this face. Photo pixels do not establish hidden structure or measured component height.',
            'sourceIds':['front-photo','dimension-photo']})
    def body(name, x, y, w, h, z, color, angle=0, texture=True, control=None, led=None):
        c, s = math.cos(angle), math.sin(angle)
        item={'name':name,'role':'component','color':color,
              'geometry':{'shape':'box','sizeMm':[w,h,z],'centerMm':[0,0,z/2]},
              'matrixRows':[[c,-s,0,x],[s,c,0,y],[0,0,1,0],[0,0,0,1]],
              'basis':'Visible footprint registered to the official front/oblique photographs; height and simplified package geometry are explicit estimates, not manufacturer CAD.',
              'sourceIds':['front-photo','component-photo']}
        if control: item['controlId']=control
        if led is not None: item['ledIndex']=led
        parts.append(item)
        footprint=rectangle(x,y,w,h,angle)
        masks.append(footprint)
        if texture and not control and led is None: photo_surface(name+'-top-photo',footprint,z+.015)
        return item
    usb_width = 9.0 if nano else 8.95
    usb_y, usb_depth = (18.95,8.1) if nano else (22.8,7.7)
    parts.append({'name':'usb-open-envelope','role':'connector','color':'#bec4c7',
        'geometry':{'shape':'open-usb-shell','sizeMm':[usb_width,usb_depth,3.2],
                    'centerMm':[0,usb_y,1.6],'wallMm':.25},
        'basis':('Pico 8.95mm metal width is explicitly labeled on the official drawing. ' if not nano else 'Nano visible metal width is a photo estimate. ')+
                'Open metal envelope only; depth, height and wall thickness estimated. Contacts, tongue, screw holes and mating fit remain UNKNOWN.',
        'sourceIds':['dimension-photo','front-photo']})
    if nano:
        body('reset-physical',1.0,9.05,4.8,3.6,1.15,'#b7bbc0',texture=False)
        body('reset-cap',1.0,9.05,3.3,2.5,1.85,'#ebe7c9',texture=False)
        masks.pop()  # Nested cap is already excluded by its carrier footprint.
        body('flash-envelope',.8,2.3,5.4,5.1,1.2,'#202329')
        body('soc-envelope',0,-8.8,7.0,7.0,1.0,'#24282c',math.pi/4)
        body('antenna-envelope',0,-20.3,6.8,1.9,1.25,'#e8e5dc')
        body('rgb-led-window',4.55,-1.3,1.25,1.25,.7,'#cbc5b2',texture=False,led=0)
        body('sck-led-window',-5.45,20.2,1.2,1.8,.6,'#d7d9bc',texture=False,led=1)
        body('power-led-window',5.65,20.2,1.2,1.8,.6,'#d7d9bc',texture=False)
    else:
        body('boot-physical',-5.2,-1.25,3.4,4.5,1.05,'#b7bbc0',texture=False)
        body('boot-cap',-5.2,-1.25,2.5,3.4,1.9,'#ece9ce',texture=False,control='boot')
        masks.pop()
        body('reset-physical',5.05,-1.25,3.4,4.5,1.05,'#b7bbc0',texture=False)
        body('reset-cap',5.05,-1.25,2.5,3.4,1.9,'#ece9ce',texture=False)
        masks.pop()
        body('flash-envelope',-.1,-.65,5.4,5.2,1.2,'#22262a')
        body('soc-envelope',-.1,-11.95,7.0,7.0,1.0,'#25292d',math.pi/4)
        body('usb-hub-envelope',-3.7,15.1,4.0,3.8,.85,'#272a2d')
        body('usb-uart-envelope',.8,9.25,3.3,3.3,.8,'#25292d')
        body('power-controller-envelope',5.0,15.0,3.0,3.0,.8,'#272a2d')
        body('antenna-envelope',.85,-24.0,7.0,1.9,1.25,'#e8e6df')
        body('rgb-led-window',5.6,-8.2,2.0,1.8,.8,'#d7d9bc',texture=False,led=0)
        body('power-led-window',-6.7,24.0,1.3,1.6,.6,'#d7d9bc',texture=False)
    # Copper drill rings and silkscreen remain photo appearance; no invented
    # hidden traces, vias, solder joints or component internals are introduced.
    pcb_outline=outline(width,height,rows)
    derived.insert(0,{'id':'pcb-main','kind':'pcb-placeholder','color':'#20242a' if nano else '#205c94',
        'geometry':{'shape':'polygon','outlineMm':pcb_outline,'holesMm':holes,
                    'sizeMm':[width,height,1.6],'centerMm':[0,0,-.8],
                    'labelSide':'back','labelSizeMm':[11.5 if nano else 14.,3.]},
        'basis':'Official board XY and documented spacing; estimated 1.6mm thickness, 0.60mm edge castellations and any unmeasured drills. Front photo is sourced appearance; rear layout, stackup and component heights remain UNKNOWN.',
        'sourceIds':['dimension-photo','front-photo','resources-historical']})
    photo_surface('pcb-front-photo',outline(width,height,rows,(usb_width/2+.05,usb_y-usb_depth/2-.05)),.02,holes+masks)
    if header:
        for row_index,x in enumerate([-spacing/2,spacing/2]):
            parts.append({'name':f'header-body-{row_index}','role':'header','color':'#20232b' if nano else '#eddf49',
                'geometry':{'shape':'box','sizeMm':[2.1,(pins-1)*2.54+2.1,2.5],'centerMm':[x,0,-2.85]},
                'basis':'Two soldered rows confirmed by exact -M photograph. Body cross-section and height are unmeasured estimates.',
                'sourceIds':['header-photo','dimension-photo']})
            for n,y in enumerate(rows):
                parts.append({'name':f'header-pin-{row_index}-{n}','role':'header','color':'#c7c5b6',
                    'geometry':{'shape':'box','sizeMm':[.64,.64,8.5],'centerMm':[x,y,-3.0]},
                    'basis':'Photographed row count and documented 2.54mm pitch; estimated 0.64mm square pins and 8.5mm total length. No fitted connector or hardware-fit claim.',
                    'sourceIds':['header-photo','dimension-photo','overview','store']})
    resource_data=gzip.decompress((folder/by_id['resources-historical']['file']).read_bytes()).decode()
    links=Links();links.feed(resource_data)
    cad_links=[u for u in links.links if any(ext in u.lower() for ext in ['.step','.stp','.kicad_mod','.kicad_pcb','.gerber','.stl','.3mf'])]
    assert not cad_links, 'A native model link requires inspection before using a photo reconstruction'
    plan={'schemaVersion':1,'id':pid,'method':'dimension-photo-reconstruction','sources':records,
        'matrixRows':[[1,0,0,0],[0,1,0,0],[0,0,1,0],[0,0,0,1]],'sourceTopologyCounts':{},
        'parts':parts,'derivedParts':derived,'presentation':{},
        'dimensions':{'pcb':{'publishedXYMm':[width,height],'thicknessMm':None,'illustrativeThicknessMm':1.6,'sourceIds':['dimension-photo']},
            'preSolderedHeaderRows':[pins,pins] if header else [],
            'headerPads':{'pinsPerRow':pins,'rowSpacingMm':spacing,'pitchMm':2.54,'holeDiameterMm':None if nano else 1.0,
                          'illustrativeHoleDiameterMm':hole_radius*2,'sourceIds':['dimension-photo','header-photo','overview','store']},
            **({'mountingHoles':{'count':4,'centerSpacingMm':[15.24,40.64],'radiusMm':.83,'sourceIds':['dimension-photo']}} if nano else {})},
        'dimensionTargets':[{'part':'pcb-main','sizeMm':[width,height,None],'toleranceMm':.001,'sourceId':'dimension-photo'}],
        'validation':{'controls':[] if nano else [{'id':'boot','type':'button'}],'ledIndices':[0,1] if nano else [0],
                      'visibleUnknownLabels':['pcb-main-unknown-back'],'shellNames':[]},
        'sourceSearch':{'status':'no-matching-CAD-in-recorded-scope','inspected':['overview','store','resources-historical'],
                        'mechanicalFileLinks':cad_links,'result':'The retained canonical Resources HTML links were parsed for STEP/STL/KiCad/Gerber/3MF. None was found in that page; linked software archives and unvisited external locations are not a global absence claim.'},
        'materialBasis':'Original photographs establish black Nano or blue Pico PCB and header color; illumination and color calibration are unknown. Metal/ivory package colors and depths are explicit approximations.',
        'assemblyNotes':'Front +Z, USB toward +Y. New reconstruction from retained current official photos and drawing; no old asset-byte identity or hidden CAD geometry is claimed.',
        'limitations':[
            'New dimension-photo reconstruction; it does not recover the lost old model bytes or inherit the historical batch-04 visual acceptance.',
            'PCB thickness, component heights, detailed passives, drill metallurgy and connector contact geometry are unmeasured; rear layout and stackup remain visibly UNKNOWN.',
            'Visible RESET has sourced physical geometry but no 3D GPIO control mapping; use the platform restart capability. '+('Nano has no invented BOOT button and GPIO0 remains an RGB channel.' if nano else 'Pico BOOT is the only mapped physical button; RESET is not a second GPIO control.'),
            'Photo projection shows the visible front appearance only; small raised passives remain flat photographic detail and are not exact mechanical or electrical geometry.',
            'USB is an open approximate metal envelope; tongue, contacts, hole details, insertion depth and cable mating are unverified.',
            'Header body/pin thickness and length are estimates; row count and documented pitch do not establish a physical-fit tolerance.',
            'Original manufacturer source images remain independent references; no real hardware acceptance is claimed.'
        ],
        'referencePhotos':[{'sourceId':s['id'],'file':s['file'],'views':'Original manufacturer source, unchanged.'} for s in records if s.get('imagePixels')],
        'photoRegistration':{'front':{'file':photo['file'],'pcbPixelBounds':[left,top,right,bottom],
            'nominalSizeMm':[width,height],'pixelFromModelXY':[[sx,0,cx],[0,-sy,cy]],'uvFromMm':uv,
            'basis':'Manual PCB substrate boundary landmarks in the retained high-resolution front photograph; USB overhang excluded. Independent official drawing supplies XY. Height and perspective remain uncalibrated.'},
            'review':{'file':by_id['dimension-photo']['file'],'pixelFromModelXY':
                [[0,-648/43.18,482],[-267/17.78,0,241.5]] if nano else [[201/21,0,479.5],[0,-489/51,334.5]],
                'basis':'Independent manufacturer dimension image; affine plane alignment only, no manufactured tolerance claim.'}}}
    save(folder/'model-plan.json',plan)
    save(folder/'source-manifest.json',{'id':pid,'method':plan['method'],'sources':records})
    (folder/'SOURCE-RIGHTS.txt').write_text(RIGHTS+'\n')
    save(EVIDENCE/(pid+'-source-search.json'),{'sources':[{'id':s['id'],'sha256':s['sha256'],'fetchedAt':s['fetchedAt']} for s in records],
        'canonicalResourcesLinks':sorted(set(links.links)),'matchingMechanicalLinks':cad_links,'scope':plan['sourceSearch']['result']})
    print(json.dumps({'id':pid,'status':'new-plan-ready','parts':len(parts),'sources':len(records),'method':plan['method']}),flush=True)


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source-cache',type=Path)
    args=parser.parse_args()
    for family in ['nano','pico']:
        for header in [False,True]: build_plan(family,header,args.source_cache)
