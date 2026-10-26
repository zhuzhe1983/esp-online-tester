#!/usr/bin/env python3
"""Rebuild the StopWatch asset from pinned manufacturer CAD and public photographs.
Requires numpy, trimesh, pillow. Usage: python tools/build-stopwatch-model.py --sources DIR
Photo regions are measured from the supplied official image, never AI generated.
"""
import argparse, hashlib, json, shutil, datetime, subprocess
from pathlib import Path
import numpy as np
import trimesh
from PIL import Image, ImageDraw

parser = argparse.ArgumentParser()
parser.add_argument('--sources', type=Path, default=Path('output/source/stopwatch-knowledge'))
parser.add_argument('--output', type=Path, default=Path('web/platform/assets/m5stack-stopwatch'))
parser.add_argument('--fetch', action='store_true', help='Fetch the pinned manufacturer source files, checking their SHA-256.')
a = parser.parse_args(); a.output.mkdir(parents=True, exist_ok=True)
def sha(path): return hashlib.sha256(path.read_bytes()).hexdigest()
expected_photos={'06':'0ece84a39f3ff6e696db65118c380ed0c4af011d2574d3c2e832e10fe12ee7fc','07':'679e0cd5397e6877ef676e8ae8710fe1149f053d2ce30dad90626f0bc51ff9cd','08':'4d73b0b82c3f98a77bfe049a034729d3d7e6f644c229da187e2fe781b401a1a9','09':'dbc9d90f243436904fb9da02f7052dd0f9431c7c1d5effc271ea63866ab554e8','10':'9f4880cfd68cfb8fa41a978f353b077f1f07afcfcffe07ea4d184950c51662f8'}
if a.fetch:
    a.sources.mkdir(parents=True,exist_ok=True); downloaded=[]
    urls={'official-cad-stl.stl':'https://raw.githubusercontent.com/m5stack/M5_Hardware/e29c139cbc7c80b5ccc5557e5ab8025a7a19a109/Products/C152_StopWatch/Structures/StopWatch.stl',**{f'photo-{n}.webp':f'https://m5stack-doc.oss-cn-shenzhen.aliyuncs.com/1242/C152_stopwatch_mainpictures_{n}.webp' for n in expected_photos}}
    for name,url in urls.items():
        resolved=subprocess.run(['curl','--fail','--location','--silent','--show-error','--max-time','30','--output',str(a.sources/name),'--write-out','%{url_effective}',url],check=True,text=True,capture_output=True).stdout
        downloaded.append({'file':name,'url':url,'resolvedUrl':resolved,'fetchedAt':datetime.datetime.now(datetime.timezone.utc).isoformat().replace('+00:00','Z'),'sha256':sha(a.sources/name),'bytes':(a.sources/name).stat().st_size})
    (a.sources/'model-download-receipts.json').write_text(json.dumps(downloaded,indent=2)+'\n')
for n,expected in expected_photos.items():
    assert sha(a.sources/f'photo-{n}.webp')==expected, f'Photo {n} changed; review before rebuilding.'
source = a.sources / 'official-cad-stl.stl'
assert sha(source) == '48b292c76f929acb75868cba47ccda86800fbff7cfff172161df64ff87f220d5', 'Source CAD changed; review before rebuilding.'
parts = trimesh.load(source).split(only_watertight=False)
assert len(parts) == 8
# 0,2,5,7 already form one assembled shell + three buttons; other four are translated duplicates.
selection = [(0,'shell', '#080a0b'),(2,'power-button','#c83223'),(5,'button-b','#087fc7'),(7,'button-a','#ffc62f')]
records=[]; buffers=[]; offset=0
rotation=np.array([[0,0,-1],[0,1,0],[1,0,0]],dtype=float)
for index,name,color in selection:
    p=parts[index]; vertices=p.vertices@rotation.T+np.array([0,0,-7.75]); normals=p.face_normals@rotation.T
    corners=[]
    for fi,face in enumerate(p.faces):
        for vi in face:
            adjacent=p.vertex_faces[vi]; adjacent=adjacent[adjacent>=0]
            adjacent=adjacent[(normals[adjacent]@normals[fi])>np.cos(np.deg2rad(38))]
            normal=(normals[adjacent]*p.area_faces[adjacent,None]).sum(axis=0); normal/=np.linalg.norm(normal)
            corners.append([*vertices[vi],*normal])
    data=np.asarray(corners,dtype='<f4').tobytes(); buffers.append(data)
    records.append({'name':name,'sourceComponent':index,'byteOffset':offset,'vertexCount':len(corners),'color':color,'boundsMm':[vertices.min(axis=0).tolist(),vertices.max(axis=0).tolist()]}); offset+=len(data)
(a.output/'geometry.bin').write_bytes(b''.join(buffers))
shutil.copyfile(source,a.output/'source.stl')
license_source=Path(__file__).resolve().parent.parent/'web/platform/assets/m5stack-stopwatch/LICENSE-M5Stack.txt'
if license_source.resolve() != (a.output/'LICENSE-M5Stack.txt').resolve(): shutil.copyfile(license_source,a.output/'LICENSE-M5Stack.txt')
# Rear is near-frontal. Measured circle center (800,799), radius 488 px.
# Plane is the measured sticker contour, not a disc that would conceal CAD openings.
im=Image.open(a.sources/'photo-07.webp').convert('RGBA')
assert im.size==(1600,1600)
polygon=[(464,620),(464,558),(514,492),(560,456),(586,451),(616,477),(648,532),(674,578),(708,613),(750,637),(798,647),(849,643),(895,622),(930,589),(957,544),(983,495),(1010,461),(1040,451),(1065,456),(1103,490),(1145,550),(1145,620),(1100,620),(1099,975),(1146,981),(1146,1049),(1089,1126),(535,1126),(465,1051),(464,938),(513,936),(514,622)]
mask=Image.new('L',im.size); ImageDraw.Draw(mask).polygon(polygon, fill=255); im.putalpha(mask)
box=(312,311,1288,1287)
im.crop(box).save(a.output/'rear-label.webp',lossless=True)
# Reference photos kept as review evidence, with their copyright/source metadata.
for name in ['photo-06.webp','photo-07.webp','photo-08.webp','photo-09.webp','photo-10.webp']:
    shutil.copyfile(a.sources/name,a.output/('reference-'+name))
knowledge=json.loads(Path('web/platform/knowledge/m5stack-stopwatch.json').read_text())
sources=[s for s in knowledge['sources'] if s['id'] in ['official-cad-stl','photo-06','photo-07','photo-08','photo-09','photo-10','dimension-drawing-pdf']]
metadata={'schemaVersion':'1','id':'m5stack-stopwatch','unit':'mm','binary':'geometry.bin','strideFloats':6,'parts':records,
 'sourceTransform':{'matrixRows':[[0,0,-1,0],[0,1,0,0],[1,0,0,-7.75],[0,0,0,1]],'scale':1,'note':'Rigid orientation only, mm preserved; original +X is front. No inferred assembly translations.'},
 'omittedComponents':[{'index':i,'reason':'Translated duplicate of an already assembled component.'} for i in [1,3,4,6]],
 'dimensions':{'bodyDiameterDrawingMm':51.95,'bodyCadDiameterMm':51.947,'totalThicknessDrawingMm':15.5,'sourceShellDepthMm':15,'addedGlassThicknessMm':.5,'activeDisplayDiameterMm':44.45},
 'texture':{'file':'rear-label.webp','source':'photo-07','sourceSha256':sha(a.sources/'photo-07.webp'),'cropXYXY':list(box),'stickerPolygonPixels':polygon,'mapping':'Rear view: u=(26-worldX)/52; v=(26+worldY)/52. Near-frontal photo; no perspective warp.','uncertainty':'Photo scale uses outside case outline; lens distortion and surface relief not calibrated. Visible label is v1.0.1; user hardware revision remains unspecified.'},
 'provenance':{'repository':'https://github.com/m5stack/M5_Hardware','commit':'e29c139cbc7c80b5ccc5557e5ab8025a7a19a109','sourceUrl':'https://raw.githubusercontent.com/m5stack/M5_Hardware/e29c139cbc7c80b5ccc5557e5ab8025a7a19a109/Products/C152_StopWatch/Structures/StopWatch.stl','sourceSha256':sha(source),'sourceLicense':'MIT; Copyright (c) 2021 M5Stack; see LICENSE-M5Stack.txt','photoRights':'Product photographs © M5Stack; used for requested product reference and a derived sticker texture. No separate open photo license has been verified. Not relicensed under project MIT.','sources':sources},
 'limitations':['Manufacturer CAD covers shell and three button caps, not a full electronics assembly.','Connector inserts, display stack and rear fasteners are derived visual additions, not manufacturer component CAD.','1.75-inch active display diameter converts to 44.45 mm; optical display/window boundary not dimensioned independently.','Rear photo label depicts v1.0.1 and is a visual reference, never a substitute for revision-specific wiring metadata.']}
metadata['artifacts']={n:sha(a.output/n) for n in ['source.stl','geometry.bin','rear-label.webp']}
(a.output/'model.json').write_text(json.dumps(metadata,ensure_ascii=False,indent=2)+'\n')
print(json.dumps({'parts':len(records),'triangles':sum(r['vertexCount']//3 for r in records),'bytes':offset,'sha256':metadata['artifacts']},indent=2))
