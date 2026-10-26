#!/usr/bin/env python3
"""Rebuild exact U032-V11 shell CAD plus explicitly photo-derived labels.
Requires tools/requirements-model.txt. No inferred PCB components or geometry scaling.
"""
import argparse,datetime,hashlib,json,shutil,subprocess
from pathlib import Path
import numpy as np
import trimesh
from PIL import Image
p=argparse.ArgumentParser();p.add_argument('--sources',type=Path,default=Path('output/source/gps-u032-v11'));p.add_argument('--output',type=Path,default=Path('web/platform/assets/m5stack-gps-u032-v11'));p.add_argument('--fetch',action='store_true');a=p.parse_args();a.output.mkdir(parents=True,exist_ok=True)
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
knowledge=json.loads(Path('web/platform/knowledge/m5stack-gps-u032-v11.json').read_text())
needed={'official-cad','cad-license','dimension-drawing','photo-5','photo-6','photo-7','photo-8'}
sources=[s for s in knowledge['sources'] if s['id'] in needed]
if a.fetch:
 a.sources.mkdir(parents=True,exist_ok=True);receipts=[]
 for s in sources:
  f=a.sources/s['archiveName'];resolved=subprocess.run(['curl','--fail','--location','--silent','--show-error','--max-time','45','--output',str(f),'--write-out','%{url_effective}',s['url']],check=True,text=True,capture_output=True).stdout
  receipts.append({'file':f.name,'url':s['url'],'resolvedUrl':resolved,'fetchedAt':datetime.datetime.now(datetime.timezone.utc).isoformat().replace('+00:00','Z'),'sha256':sha(f),'bytes':f.stat().st_size})
 (a.sources/'rebuild-receipts.json').write_text(json.dumps(receipts,indent=2)+'\n')
for s in sources:assert sha(a.sources/s['archiveName'])==s['sha256'],s['id']+' changed: review source before rebuilding'
parts=trimesh.load(a.sources/'source.stl').split(only_watertight=False);assert len(parts)==4
# 0 + 3 are already assembled at x=-40; 1 + 2 are translated printing duplicates.
shift=np.array([40,0,-1.2]);selection=[(0,'shell-front'),(3,'shell-back')];records=[];buffers=[];offset=0
for index,name in selection:
 part=parts[index];vertices=part.vertices+shift;normals=part.face_normals;corners=[]
 for fi,face in enumerate(part.faces):
  for vi in face:
   adjacent=part.vertex_faces[vi];adjacent=adjacent[adjacent>=0];adjacent=adjacent[(normals[adjacent]@normals[fi])>np.cos(np.deg2rad(38))]
   n=(normals[adjacent]*part.area_faces[adjacent,None]).sum(axis=0);n/=np.linalg.norm(n);corners.append([*vertices[vi],*n])
 data=np.asarray(corners,dtype='<f4').tobytes();buffers.append(data);records.append({'name':name,'sourceComponent':index,'byteOffset':offset,'vertexCount':len(corners),'color':'#c5c5c1','boundsMm':[vertices.min(axis=0).tolist(),vertices.max(axis=0).tolist()]});offset+=len(data)
(a.output/'geometry.bin').write_bytes(b''.join(buffers));shutil.copyfile(a.sources/'source.stl',a.output/'source.stl');shutil.copyfile(a.sources/'license.txt',a.output/'LICENSE-M5Stack.txt')
textures=[]
for side,num,crop,size,center in [('front',7,[298,159,499,473],[18,28],[0,7.353,3.815]),('back',8,[311,164,491,478],[16.2,28.2],[0,7,-3.815])]:
 original=Image.open(a.sources/f'photo-{num}.webp').convert('RGB');assert original.size==(800,800)
 original.crop(crop).save(a.output/f'{side}-label.webp',lossless=True)
 textures.append({'name':side+'-label','file':side+'-label.webp','sourceId':f'photo-{num}','sourceSha256':sha(a.sources/f'photo-{num}.webp'),'cropXYXY':crop,'outputPixels':[crop[2]-crop[0],crop[3]-crop[1]],'dimensionsMm':size,'centerMm':center,'transform':'Axis-aligned crop, no warp. u increases left-to-right in corresponding face view; v increases bottom-to-top.','uncertainty':'Near-frontal public photo; no lens calibration. Label area follows source CAD recess; photo brightness retained, not material calibration.'})
for n in [5,6,7,8]:shutil.copyfile(a.sources/f'photo-{n}.webp',a.output/f'reference-photo-{n}.webp')
meta={'schemaVersion':'1','id':'m5stack-gps-u032-v11','unit':'mm','binary':'geometry.bin','strideFloats':6,'parts':records,'sourceTransform':{'matrixRows':[[1,0,0,40],[0,1,0,0],[0,0,1,-1.2],[0,0,0,1]],'scale':1,'note':'One common translation only; original assembly retained. Front +Z, connector -Y.'},'omittedComponents':[{'index':1,'duplicateOf':0,'translationMm':[84.384063720703125,0,0]},{'index':2,'duplicateOf':3,'translationMm':[43.16054916381836,0,0]}],'dimensions':{'width':24,'height':48,'depth':8,'mountingHoleDiameter':5,'mountingHoleSpacing':16,'mountingHoleCenterFromBottom':12,'sourceId':'dimension-drawing'},'textures':textures,'connectors':{'grove':{'type':'HY2.0-4P','anchorMm':[0,-24,1.4],'outwardNormal':[0,-1,0],'pinOrder':['gnd','vcc','signal1','signal2'],'allowConnections':True,'openingMm':[10,5.2],'sourceIds':['dimension-drawing','photo-6'],'basis':'Opening from drawing and CAD. Mating plane is the case opening; insertion depth 3 mm is estimated; detailed insert and pins are photo-informed estimates, not connector manufacturing CAD.'}},'derivedParts':[{'id':'pcb-main','kind':'pcb-placeholder','label':'UNKNOWN','basis':'estimated','dimensionsMm':[20,28,1],'centerMm':[0,6,0],'sourceIds':[],'notes':'Empty neutral rectangular board constrained to case space. Outline, thickness, location and all components unknown; not an electronics model.'},{'id':'grove-insert','basis':'photo-measurement','sourceIds':['photo-6','dimension-drawing'],'notes':'Cyan housing and four visible contacts only. Hidden dimensions are estimates.'},{'id':'rear-fastener','basis':'photo-measurement','sourceIds':['photo-8'],'notes':'Visible screw represented at CAD hole center; head and socket approximate.'}],'provenance':{'repository':'https://github.com/m5stack/M5_Hardware','commit':'e29c139cbc7c80b5ccc5557e5ab8025a7a19a109','sourceUrl':next(s['url'] for s in sources if s['id']=='official-cad'),'sourceSha256':sha(a.sources/'source.stl'),'sourceLicense':'MIT; Copyright (c) 2021 M5Stack; see LICENSE-M5Stack.txt','photoRights':'Product photographs © M5Stack. Reference photos and derived label crops are retained for requested product visualization and verification. No separate open photo license verified; not relicensed under project MIT.','sources':sources},'limitations':['Only U032-V11 v1.1. Do not apply to U032/AT6558.','Shell only is manufacturer CAD; connectors, fastener and photo labels are separate visual additions.','PCB UNKNOWN is an estimated blank placeholder; no components, traces or manufacturing claims.','Official linked schematic title conflicts with v1.1 identity; not used for PCB layout.','UART simulation remains logical NMEA input, independent of external appearance.']}
files=['source.stl','geometry.bin','LICENSE-M5Stack.txt','front-label.webp','back-label.webp']+[f'reference-photo-{n}.webp' for n in [5,6,7,8]]
meta['artifacts']={name:sha(a.output/name) for name in files};(a.output/'model.json').write_text(json.dumps(meta,ensure_ascii=False,indent=2)+'\n');print(json.dumps({'triangles':sum(p['vertexCount']//3 for p in records),'bytes':offset,'sha256':meta['artifacts']},indent=2))
