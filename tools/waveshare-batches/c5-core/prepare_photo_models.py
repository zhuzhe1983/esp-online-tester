#!/usr/bin/env python3
"""Source-preserving C5 Pico/Mini exteriors; unmeasured dimensions stay explicit."""
import argparse
import datetime
import gzip
import hashlib
import json
import math
from pathlib import Path
from PIL import Image

ROOT = Path(__file__).resolve().parents[3]
RIGHTS = 'Original Waveshare material. No independent asset redistribution licence stated; retained for auditing this hardware adaptation.'


def save(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')


def sha(data):
    return hashlib.sha256(data).hexdigest()


def prepare(source_root, family, slug, photo, bounds, size, boot, led, shield, antenna, usb):
    src = source_root / family
    out = ROOT / 'web/platform/assets' / slug
    out.mkdir(parents=True, exist_ok=True)
    sources = []
    for source_id, name, target, role in [
        ('dimension-photo', photo, 'dimension-photo.webp', 'photo-source'),
        ('official-resources', 'resources.html', 'resources.html.gz', 'reference-only'),
        ('official-repository-tree', 'tree.json', 'tree.json.gz', 'reference-only'),
    ]:
        path = src / name
        data = path.read_bytes()
        receipt_path = src / ('tree.receipt.json' if name == 'tree.json' else name + '.receipt.json')
        receipt = json.loads(receipt_path.read_text())
        assert sha(data) == receipt['sha256'] and len(data) == receipt['bytes']
        retained = gzip.compress(data, mtime=0) if target.endswith('.gz') else data
        (out / target).write_bytes(retained)
        row = {'id': source_id, 'url': receipt.get('requestedUrl', receipt.get('url')),
               'finalUrl': receipt['finalUrl'], 'fetchedAt': datetime.datetime.fromisoformat(receipt['fetchedAt']).isoformat(timespec='seconds').replace('+00:00', 'Z'),
               'bytes': len(data), 'sha256': sha(data), 'file': target, 'role': role, 'rights': RIGHTS}
        if receipt.get('mediaType') or receipt.get('contentType'):
            row['contentType'] = receipt.get('mediaType', receipt.get('contentType'))
        else:
            row['mediaType'] = 'image/webp' if target.endswith('.webp') else 'application/json' if target.endswith('.json.gz') else 'text/html'
            row['mediaTypeBasis'] = 'Retained file format; HTTP Content-Type was not captured.'
        if target.endswith('.gz'):
            row['retained'] = {'encoding': 'gzip', 'bytes': len(retained), 'sha256': sha(retained)}
        else:
            row['imagePixels'] = list(Image.open(path).size)
        sources.append(row)
    # This annotation explicitly locates the Mini RGB package below RESET.
    annotation = next(iter(src.glob('*Ziyuan-*.webp')), None)
    if annotation:
        receipt = json.loads(Path(str(annotation) + '.receipt.json').read_text())
        data = annotation.read_bytes(); assert sha(data) == receipt['sha256']
        (out / 'annotated-front.webp').write_bytes(data)
        sources.append({'id': 'annotated-front', 'url': receipt['requestedUrl'], 'finalUrl': receipt['finalUrl'],
                        'fetchedAt': receipt['fetchedAt'], 'bytes': len(data), 'sha256': sha(data), 'file': 'annotated-front.webp',
                        'imagePixels': list(Image.open(annotation).size), 'role': 'reference-only', 'rights': RIGHTS,
                        'mediaType': 'image/webp', 'mediaTypeBasis': 'Retained file format.'})
    left, top, right, bottom = bounds
    width, height = size
    def xy(x, y):
        return [(x-left)/(right-left)*width-width/2, height/2-(y-top)/(bottom-top)*height]
    def rect(x0,y0,x1,y1):
        return [xy(x0,y1),xy(x1,y1),xy(x1,y0),xy(x0,y0)]
    image_width,image_height=Image.open(src/photo).size
    texture={'file':'dimension-photo.webp','projection':'xy','side':'front',
             'boundsMm':[*xy(0,image_height),*xy(image_width,0)],'minNormalZ':.99}
    def box(name, role, dimensions, center, color, **extra):
        return {'name':name,'role':role,'color':color,'geometry':{'shape':'box','sizeMm':dimensions,'centerMm':center},
                'basis':'XY registered to the original official dimension photograph; package height is an unmeasured visual estimate.',**extra}
    parts=[box('pcb-main','pcb',[width,height,1.6],[0,0,-.8],'#202522')]
    outline=rect(left,top,right,bottom)
    parts.append({'name':'pcb-front-photo','role':'photo-appearance','color':'#ffffff',
                  'geometry':{'shape':'photo-face','outlineMm':outline,'zMm':.012},'texture':texture,
                  'basis':'Unchanged original photo mapped by documented board width/length. No generated text or component labels.'})
    # Antenna is a separate documented module overhang, never a stretched PCB.
    a0,a1,a2,a3=antenna; ax,ay=xy((a0+a2)/2,(a1+a3)/2)
    parts.append(box('module-antenna','component',[(a2-a0)/(right-left)*width,(a3-a1)/(bottom-top)*height,.8],[ax,ay,.4],'#1e2320',texture=texture))
    s0,s1,s2,s3=shield;sx,sy=xy((s0+s2)/2,(s1+s3)/2)
    parts.append(box('rf-shield','component',[(s2-s0)/(right-left)*width,(s3-s1)/(bottom-top)*height,2.0],[sx,sy,1.0],'#aeb2a6',texture=texture))
    bx,by=xy(*boot)
    parts.append(box('boot-key','control',[1.9,2.65,1.0],[bx,by,.5],'#d4d1ac',controlId='boot'))
    lx,ly=xy(*led)
    parts.append(box('rgb-window','led',[1.6,1.6,.8],[lx,ly,.4],'#d4d8ce',ledIndex=0))
    ux,uy,angle=usb
    x,y=xy(ux,uy);c,s=math.cos(angle),math.sin(angle)
    parts.append({'name':'usb-shell','role':'connector','color':'#aeb4b2',
                  'geometry':{'shape':'open-usb-shell','sizeMm':[8.95,7.4,3.2],'centerMm':[0,0,0],'wallMm':.3},
                  'matrixRows':[[c,-s,0,x],[s,c,0,y],[0,0,1,1.6],[0,0,0,1]],
                  'basis':'8.95 mm mouth width is dimensioned; depth, wall and height are explicit visual estimates. Open shell, no invented cable anchor.'})
    limitations=[
        'No official STEP/STL was located in the inspected manufacturer resources and pinned repository tree; this is an approximate exterior reconstruction.',
        'PCB width/length, antenna overhang and USB mouth width come from the dimension photograph. PCB thickness1.6 mm and package/USB heights are unmeasured rendering estimates.',
        'Front silkscreen and package markings are original image pixels, not synthesized artwork. Depth and occluded structures cannot be inferred from those pixels.',
        'The reverse and internal stack are UNKNOWN; no inferred reverse circuit, drilled hole or hidden connector is supplied.',
        'Header-installed and external-antenna SKU variants retain separate identities; this base-PCB exterior does not claim their distinct hardware geometry.',
        'No measured material response, electrical cable connectivity, physical fit or browser WebGL acceptance is asserted.'
    ]
    search={'status':'no-official-cad-located','inspected':['official-resources','official-repository-tree'],
            'result':'Source-preserving dimension-photo exterior; UNKNOWN reverse retained.'}
    plan={'id':slug,'method':'dimension-photo-reconstruction','sources':sources,'parts':parts,
          'dimensions':{'pcb':{'sizeMm':[width,height,None],'nominalVisualThicknessMm':1.6,'thicknessStatus':'UNKNOWN; rendering estimate',
                                'basis':['dimension-photo']},'packageHeights':'Unmeasured explicit estimates'},
          'derivedParts':[{'id':'unknown-reverse','kind':'printed-marking','lines':['UNKNOWN'],'color':'#b1b7bd',
                           'geometry':{'sizeMm':[min(width*.8,24),4],'centerMm':[0,0,-1.612],'rotation':[0,math.pi,0]},
                           'basis':'No exact orthographic reverse or inner geometry was available for registration.'}],
          'presentation':{'buttons':[]},'referencePhotos':[{'sourceId':'dimension-photo','file':'dimension-photo.webp','views':'Official dimensioned front view'}],
          'sourceSearch':search,'photoRegistration':{'sourceId':'dimension-photo','pcbPixelBounds':bounds,'pcbSizeMm':size,
              'transform':'Independent X/Y photo registration to dimension labels; geometry is constructed at native millimetres, not rescaled sibling CAD.',
              'sourceImageUnchanged':True,'bootPixelCenter':boot,'rgbPixelCenter':led,
              'review':{'file':'dimension-photo.webp',
                  'pixelFromModelXY':[[(right-left)/width,0,(left+right)/2],
                                      [0,-(bottom-top)/height,(top+bottom)/2]],
                  'includeParts':['pcb-main','boot-key','rgb-window']}},
          'limitations':limitations,'omittedSourceComponents':[],
          'materialBasis':'Original photo appearance and neutral non-calibrated PBR.',
          'assemblyNotes':'Explicit measured XY and approximate Z exterior; no concealed structure invented.',
          'dimensionTargets':[{'part':'pcb-main','sizeMm':[width,height,1.6],'toleranceMm':.02,'sourceId':'dimension-photo',
                               'scope':'XY dimensioned; Z is the declared rendering estimate, not a measured target.'}],
          'validation':{'controls':[{'id':'boot','type':'button'}],'ledIndices':[0],'shellNames':[]}}
    save(out/'model-plan.json',plan)
    save(out/'source-manifest.json',{'schemaVersion':1,'id':slug,'sources':sources,'search':search,'limitations':limitations})
    (out/'SOURCE-RIGHTS.txt').write_text(RIGHTS+'\nGeometry is an independently documented approximate reconstruction; source photo rights remain with Waveshare.\n')
    print(slug)


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--source-root',type=Path,required=True);a=parser.parse_args()
    prepare(a.source_root,'ESP32-C5-Pico','waveshare-esp32-c5-pico','ESP32-C5-Pico-size-19e1b396795511d234a76685f4b12573.webp',
            [151,103,664,315],[51,21],[359,250],[256,209],[512,132,658,285],[664,129,723,285],[175,208,math.pi/2])
    prepare(a.source_root,'ESP32-C5-MINI-KIT','waveshare-esp32-c5-mini-kit','ESP32-C5-MINI-KIT-board-size-c58c3b71ab8177f993b558f019fde99a.webp',
            [206,183,530,631],[21,28.5],[255,215],[464,280],[244,392,479,615],[244,631,482,721],[364,220,0])
