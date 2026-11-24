#!/usr/bin/env python3
"""Rebuild the source-described Claw 1.54 preview, without network/CAD dependencies.

The nominal 1.54-inch screen anchors a single-photo front-plane estimate.
Hidden thickness, internals and debug-button positions are explicitly uncalibrated.
This file generates geometry, never a firmware screenshot or a fake PCB layout.
"""
import argparse
import hashlib
import json
import math
import shutil
import struct
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / 'web/platform/assets/cloudzao-claw-154'


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def rounded(width, height, radius, x=0, y=0):
    points = []
    for cx, cy, angle in [(width/2-radius, height/2-radius, 0),
                          (-width/2+radius, height/2-radius, 90),
                          (-width/2+radius, -height/2+radius, 180),
                          (width/2-radius, -height/2+radius, 270)]:
        for i in range(9):
            a = math.radians(angle+i*90/8)
            points.append((x+cx+radius*math.cos(a), y+cy+radius*math.sin(a)))
    return points


def triangle(data, a, b, c):
    u = [b[i]-a[i] for i in range(3)]
    v = [c[i]-a[i] for i in range(3)]
    normal = [u[1]*v[2]-u[2]*v[1], u[2]*v[0]-u[0]*v[2], u[0]*v[1]-u[1]*v[0]]
    length = math.sqrt(sum(n*n for n in normal))
    if length < 1e-10:
        return
    normal = [n/length for n in normal]
    for point in (a, b, c):
        data.extend([*point, *normal])


def quad(data, a, b, c, d):
    triangle(data, a, b, c)
    triangle(data, a, c, d)


def sides(data, outline, z0, z1, inside=False, aperture=False):
    for a, b in zip(outline, outline[1:]+outline[:1]):
        spans = [(a, b, z0, z1)]
        if aperture and abs(a[1]-b[1]) < 1e-9 and a[1] < 0 and min(a[0], b[0]) < -4.7 and max(a[0], b[0]) > 4.7:
            lo, hi = sorted([a, b])
            spans = [(lo, (-4.7, lo[1]), z0, z1), ((4.7, lo[1]), hi, z0, z1),
                     ((-4.7, lo[1]), (4.7, lo[1]), z0, -1.55),
                     ((-4.7, lo[1]), (4.7, lo[1]), 1.35, z1)]
        for start, end, low, high in spans:
            vertices = [(*start, low), (*end, low), (*end, high), (*start, high)]
            if inside:
                vertices.reverse()
            quad(data, *vertices)


def ring(outer, inner, z0, z1, aperture=False):
    data = []
    sides(data, outer, z0, z1, aperture=aperture)
    sides(data, inner, z0, z1, inside=True, aperture=aperture)
    for i, j in [(i, (i+1) % len(outer)) for i in range(len(outer))]:
        quad(data, (*outer[i], z1), (*outer[j], z1), (*inner[j], z1), (*inner[i], z1))
        quad(data, (*inner[i], z0), (*inner[j], z0), (*outer[j], z0), (*outer[i], z0))
    return data


def solid(width, height, depth, radius, center):
    x, y, z = center
    outline = rounded(width, height, radius, x, y)
    data = []
    sides(data, outline, z-depth/2, z+depth/2)
    for a, b in zip(outline, outline[1:]+outline[:1]):
        triangle(data, (x, y, z+depth/2), (*a, z+depth/2), (*b, z+depth/2))
        triangle(data, (x, y, z-depth/2), (*b, z-depth/2), (*a, z-depth/2))
    return data


def build(output):
    plan = json.loads((SOURCE / 'model-plan.json').read_text())
    reference = SOURCE / plan['reference']['file']
    if digest(reference) != plan['reference']['sha256']:
        raise ValueError('Reference image differs from the fixed source receipt')
    output.mkdir(parents=True, exist_ok=True)
    width, height, depth = plan['bodyEstimateMm']
    side = plan['screen']['sideMm']
    screen_y = plan['screen']['centerYmm']
    records, blobs = [], []
    def part(name, data, **properties):
        blob = struct.pack('<'+'f'*len(data), *data)
        points = [data[i:i+3] for i in range(0, len(data), 6)]
        records.append(dict(name=name, byteOffset=sum(map(len, blobs)), vertexCount=len(data)//6,
                            boundsMm=[[min(p[i] for p in points) for i in range(3)],
                                      [max(p[i] for p in points) for i in range(3)]], **properties))
        blobs.append(blob)
    photo = ['claw154-product-photo']
    part('shell-wall', ring(rounded(width,height,2.3), rounded(width-2,height-2,1.3), -3.65,3.8,True),
         role='shell', color='#d6d8d9', material={'metalness':0.65,'roughness':0.4}, basis='single-photo exterior approximation; thickness unknown', sourceIds=photo)
    part('shell-back', solid(width,height,.6,2.3,[0,0,-3.95]), role='shell', color='#d6d8d9',
         material={'metalness':0.65,'roughness':0.4}, basis='unobserved plain rear closure; no invented ports or fasteners', sourceIds=photo)
    part('front-glass-frame', ring(rounded(width-.5,height-.5,2.1),rounded(side,side,.1,0,screen_y),3.8,4.08),
         role='component',color='#121719',material={'roughness':0.22,'metalness':0.04},basis='single-photo front-plane estimate',sourceIds=photo)
    for id_, x in [('boot',-8),('power',8)]:
        part('debug-'+id_,solid(13,5,1.2,1.6,[x,-31,0]),role='component',color='#315a67',controlId=id_,
             basis='separate debug input; physical button location unknown',sourceIds=['claw154-config'])
    (output/'geometry.bin').write_bytes(b''.join(blobs))
    marks = f'''<svg xmlns="http://www.w3.org/2000/svg" width="720" height="864" viewBox="{-width/2} {-height/2} {width} {height}">
<g transform="scale(1,-1)" fill="none" stroke="#eeeeee" stroke-width=".15" stroke-linejoin="round">
<path d="M-10.3 -15.7 L-7 -13.6 L-7 -17.8 Z"/><circle cx="0" cy="-15.7" r="2.1"/><path d="M7 -13.6 L10.3 -15.7 L7 -17.8 Z"/>
</g></svg>\n'''
    (output/'cover-marks.svg').write_text(marks)
    derived = [
        dict(id='pcb-main',kind='pcb-placeholder',basis='unknown interior; debug spacing only',sourceIds=photo,
             color='#788181',geometry=dict(sizeMm=[30,35,1],centerMm=[0,0,0],labelSide='both',labelSizeMm=[25,6])),
        dict(id='front-navigation-marks',kind='label',basis='symbols visible in official oblique photo; estimated positions',sourceIds=photo,
             texture='cover-marks.svg',geometry=dict(sizeMm=[width,height],centerMm=[0,0,4.10])),
        dict(id='usb-c',kind='connector-visual',connectorType='USB-C',allowConnections=False,basis='USB-C-shaped front visible in photo; nominal face only, no exact connector or mating-depth evidence',sourceIds=photo,
             geometry=dict(centerMm=[0,-height/2-.02,-.1],rotation=[math.pi/2,0,0])),
    ]
    for id_,x,title in [('boot',-8,'BOOT'),('power',8,'PWR')]:
        derived.append(dict(id='debug-label-'+id_,kind='printed-marking',basis='debug layout',sourceIds=['claw154-config'],
                            lines=[title],color='#eef7f4',geometry=dict(sizeMm=[11,3],centerMm=[x,-31,.62])))
    derived.append(dict(id='debug-input-label',kind='printed-marking',basis='debug layout',sourceIds=['claw154-config'],
                        lines=['DEBUG INPUT'],color='#436471',geometry=dict(sizeMm=[30,3],centerMm=[0,-26.4,.62])))
    buttons = [dict(id=id_,shape='circle',sizeMm=[5.4,5.4],positionMm=[x,-15.7,4.12]) for id_,x in [('previous',-8.3),('home',0),('next',8.3)]]
    front_parts = ['front-glass-frame','front-navigation-marks','live-display']+['cover-key:'+x for x in ['previous','home','next']]
    presentation = dict(bodySizeMm=[width,68,depth],screen=dict(sizeMm=[side,side],positionMm=[0,screen_y,4.12]),buttons=buttons,
        inspectionGroups=[dict(id='front-cover',parts=front_parts,offsetMm=[0,0,18],sourceIds=photo,notes='Illustrative inspection offset; front cover/photo geometry is estimated.'),
                          dict(id='rear-cover',parts=['shell-back'],offsetMm=[0,0,-15],sourceIds=photo,notes='Unobserved plain back cover; separation only exposes the UNKNOWN board.'),
                          dict(id='unknown-pcb',parts=['pcb-main'],offsetMm=[0,0,7],sourceIds=photo,notes='UNKNOWN placeholder, not a recovered PCB layout or verified assembly.')])
    for filename in [plan['reference']['file'],'NOTICE.md','upstream-LICENSE.txt']:
        if (SOURCE/filename).resolve() != (output/filename).resolve():
            shutil.copyfile(SOURCE/filename,output/filename)
    metadata = dict(schemaVersion='1',id='cloudzao-claw-154',unit='mm',strideFloats=6,binary='geometry.bin',parts=records,
        dimensions=dict(width=width,height=height,depth=depth,basis='photo-derived front-plane envelope; 8.5 mm depth is uncalibrated display spacing'),
        presentation=presentation,derivedParts=derived,referencePhotos=[dict(sourceId='claw154-product-photo',file=plan['reference']['file'],view='front-right-bottom oblique')],
        provenance=dict(kind='photo-derived',sources=plan['sources'],generator='tools/build-cloudzao-claw154-model.py',license='See NOTICE.md; original generated geometry is MIT, source photo retains upstream rights.'),
        limitations=plan['limitations'],debugPreview=dict(mode='photo-derived',isPhysicalReplica=False,physicalDimensionsVerified=False,liveFramebuffer=True),
        reviewViews=dict(front=dict(position=[0,0,180],span=82),back=dict(position=[0,0,-180],span=82),side=dict(position=[180,0,0],span=82)),
        artifacts={filename:digest(output/filename) for filename in ['geometry.bin','cover-marks.svg',plan['reference']['file'],'NOTICE.md','upstream-LICENSE.txt']})
    (output/'model.json').write_text(json.dumps(metadata,ensure_ascii=False,indent=2)+'\n')
    print(json.dumps(dict(productId=metadata['id'],parts=len(records),geometryBytes=sum(map(len,blobs)),modelSha256=digest(output/'model.json'),physicalReplica=False)))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output',type=Path,required=True,help='New scratch output directory; committed model assets are never overwritten implicitly.')
    build(parser.parse_args().output)
