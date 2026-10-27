#!/usr/bin/env python3
"""Export the official C153 assembly without scaling or geometry invention.
Requires numpy, trimesh, Pillow. Downloads are explicit (--fetch), pinned and checked.
"""
import argparse, hashlib, json, shutil, subprocess, datetime
from pathlib import Path
import numpy as np
import trimesh
from PIL import Image

p=argparse.ArgumentParser();p.add_argument('--sources',type=Path,default=Path('output/source/papermono-knowledge'));p.add_argument('--output',type=Path,default=Path('web/platform/assets/m5stack-papermono'));p.add_argument('--fetch',action='store_true');a=p.parse_args();a.output.mkdir(parents=True,exist_ok=True)
def sha(f):return hashlib.sha256(f.read_bytes()).hexdigest()
if a.fetch:
    manifest=json.loads((a.output/'model.json').read_text());a.sources.mkdir(parents=True,exist_ok=True);receipts=[]
    for s in manifest['provenance']['sources']:
        target=a.sources/s['archiveName'];r=subprocess.run(['curl','--fail','--location','--silent','--show-error','--max-time','45','--output',str(target),'--write-out','%{url_effective}',s['url']],check=True,capture_output=True,text=True)
        assert sha(target)==s['sha256'],f"Source changed: {s['id']}"
        receipts.append({**s,'resolvedUrl':r.stdout,'fetchedAt':datetime.datetime.now(datetime.timezone.utc).isoformat(timespec='seconds').replace('+00:00','Z')})
    (a.sources/'sources.json').write_text(json.dumps(receipts,indent=2)+'\n')
sources=[s for s in json.loads((a.sources/'sources.json').read_text()) if s.get('status')=='fetched']
by_id={s['id']:s for s in sources}
for s in sources:assert sha(a.sources/s['archiveName'])==s['sha256']
source=a.sources/'official-cad-stl.stl';parts=trimesh.load(source).split(only_watertight=False);assert len(parts)==8
# Component 0 is the main shell, with 2/3/4 already seated in its actual side openings.
# Components 1/5/6/7 are translated printing-layout duplicates, not a second case half.
translation=np.array([-30.7616885304451,-50.5,4.0]);records=[];buffers=[];offset=0
for index,name,color in [(0,'shell','#888d8e'),(2,'button-a','#424747'),(4,'button-b','#424747'),(3,'power-button','#c74732')]:
    mesh=parts[index];vertices=mesh.vertices+translation;normals=mesh.face_normals;corners=[]
    for fi,face in enumerate(mesh.faces):
        for vi in face:
            adjacent=mesh.vertex_faces[vi];adjacent=adjacent[adjacent>=0];adjacent=adjacent[(normals[adjacent]@normals[fi])>np.cos(np.deg2rad(38))]
            normal=(normals[adjacent]*mesh.area_faces[adjacent,None]).sum(axis=0);normal/=np.linalg.norm(normal);corners.append([*vertices[vi],*normal])
    data=np.asarray(corners,dtype='<f4').tobytes();buffers.append(data);records.append({'name':name,'sourceComponent':index,'byteOffset':offset,'vertexCount':len(corners),'color':color,'boundsMm':[vertices.min(axis=0).tolist(),vertices.max(axis=0).tolist()]});offset+=len(data)
(a.output/'geometry.bin').write_bytes(b''.join(buffers));shutil.copyfile(source,a.output/'source.stl');shutil.copyfile(a.sources/'cad-license.txt',a.output/'LICENSE-M5Stack.txt')
crop=[488,286,1108,1320];Image.open(a.sources/'photo-08.webp').crop(crop).save(a.output/'rear-label.webp',lossless=True)
for n in ['02','06','07','08','09','10','11','12']:shutil.copyfile(a.sources/f'photo-{n}.webp',a.output/f'reference-photo-{n}.webp')
metadata={'schemaVersion':'1','id':'m5stack-papermono','unit':'mm','binary':'geometry.bin','strideFloats':6,'parts':records,
 'sourceTransform':{'matrixRows':[[1,0,0,float(translation[0])],[0,1,0,-50.5],[0,0,1,4],[0,0,0,1]],'scale':1,'note':'Translation only. Source +Z front, +Y top. Native millimetres preserved.'},
 'omittedComponents':[{'index':i,'reason':'Translated printing-layout duplicate of an already assembled shell/button.'} for i in [1,5,6,7]],
 'dimensions':{'bodyWidthMm':61,'overallWidthMm':62,'heightMm':101,'depthMm':8,'frontWindowMm':[50.84,86.4],'windowTopInsetMm':4.89},
 'texture':{'file':'rear-label.webp','source':'photo-08','sourceSha256':by_id['photo-08']['sha256'],'cropXYXY':crop,'planeMm':[58.9,99],'mapping':'Back-facing plane; full cropped rear product label including NFC pad, no generated text.','uncertainty':'Near-frontal photo has small lens/perspective errors and includes lighting. Label appearance is not independent photo evidence.'},
 'derivedParts':[{'id':'pcb-main','kind':'pcb-placeholder','label':'UNKNOWN','basis':'estimated','geometry':{'sizeMm':[56,94,1],'centerMm':[0,0,-.5]},'notes':'Case-space estimate only; no board layout or component placement data. Visible connector inserts are separate photo/CAD-derived visual parts.'},
 {'id':'front-cover','basis':'official-drawing','sizeMm':[59,99],'cornerRadiusMm':2,'centerMm':[0,0,3.8],'notes':'Front cover dimensions from official drawing; depth within CAD rim estimated from front photograph.'},
 {'id':'live-display','basis':'official-drawing','sizeMm':[50.84,86.4],'centerMm':[0,2.41,3.82],'notes':'Drawing front window, not independently measured active LCD pixel boundary; no static promotional pixels overlay the runtime framebuffer.'},
 {'id':'usb-c-insert','basis':'photo-and-cad','notes':'9.03 x 3.25 mm shell opening from drawing. Visible metal rim/tongue approximated from photo09; not a connector manufacturing model.'}],
 'provenance':{'repository':'https://github.com/m5stack/M5_Hardware','commit':'e29c139cbc7c80b5ccc5557e5ab8025a7a19a109','sourceUrl':by_id['official-cad-stl']['url'],'sourceSha256':sha(source),'sourceLicense':'MIT; Copyright (c) 2021 M5Stack; see LICENSE-M5Stack.txt','photoRights':'Product photographs © M5Stack. No separate open photo license verified; used for product identification/reference and a derived product label, not relicensed under project MIT.','sources':[s for s in sources if s['id'] in ['official-cad-stl','cad-license','dimension-drawing','photo-02','photo-06','photo-07','photo-08','photo-09','photo-10','photo-11','photo-12']]},
 'limitations':['Manufacturer CAD covers exterior shell and three button caps, not the electronics assembly.','Blank PCB has UNKNOWN; dimensions and placement are estimated and excluded from 1:1 internal certification.','The shell material/color is a display approximation chosen against photos, not a physical reflectance measurement.','Visual evidence does not certify board execution. Current product-specific runtime support is tracked separately in the catalog.']}
metadata['derivedParts'].append({'id': 'rgb-led-diffuser', 'kind': 'rgb-led', 'ledIndex': 0, 'basis': 'cad-opening-and-photo', 'sourceIds': ['official-cad-stl', 'photo-10'], 'geometry': {'shape': 'capsule', 'sizeMm': [8, 2], 'sideSurfaceXmm': -30.515, 'centerYZmm': [10, 0], 'estimatedInsetDepthMm': 0.35}, 'notes': 'Fits original source side opening Y5.9618..14.0382/Z±1.0384mm. Capsule face8×2mm leaves0.038mm perimeter clearance. Diffuser face15µm outside shell avoids coplanar flicker. Optical thickness/color estimated; no internal RGB package position claimed.'})
metadata['artifacts']={n:sha(a.output/n) for n in ['source.stl','geometry.bin','rear-label.webp']};(a.output/'model.json').write_text(json.dumps(metadata,ensure_ascii=False,indent=2)+'\n');print(json.dumps({'parts':len(records),'triangles':sum(x['vertexCount']//3 for x in records),'artifacts':metadata['artifacts']},indent=2))
