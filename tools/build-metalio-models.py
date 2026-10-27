#!/usr/bin/env python3
"""Rebuild the official MakerWorld Metalio CAD, preserving source geometry and scale.
Dependencies: tools/requirements-model.txt. Source mesh extraction reuses the existing CAD reader.
"""
import argparse,hashlib,importlib.util,json,shutil
from pathlib import Path
import numpy as np
ROOT=Path(__file__).resolve().parents[1];ASSETS=ROOT/'web/platform/assets'
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def build(pid,output):
 folder=ASSETS/pid;plan=json.loads((folder/'model-plan.json').read_text());source=folder/plan['sourceFile'];assert sha(source)==plan['sourceSha256'],'Source CAD hash changed'
 spec=importlib.util.spec_from_file_location('cad_reader',ROOT/'tools/onboard-lane-01.py');module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module);meshes=module.load_parts(source)
 assert sorted(item['sourceComponent'] for item in plan['parts'])==list(range(len(meshes))),'Each original component must occur exactly once'
 matrix=np.asarray(plan['matrixRows'],float);rotation=matrix[:3,:3];assert np.allclose(rotation.T@rotation,np.eye(3))and np.isclose(np.linalg.det(rotation),1),'Rigid right-handed transform required'
 output.mkdir(parents=True,exist_ok=True);records=[];buffers=[];offset=0;checks=[]
 for item in plan['parts']:
  mesh=meshes[item['sourceComponent']];v=mesh.vertices@rotation.T+matrix[:3,3];n=mesh.face_normals@rotation.T
  data=np.concatenate([v[mesh.faces],np.repeat(n[:,None,:],3,axis=1)],axis=2).astype('<f4').reshape(-1,6);expected=v[mesh.faces].reshape(-1,3);error=float(np.abs(expected-data[:,:3]).max());assert error<.0001
  records.append({**item,'byteOffset':offset,'vertexCount':len(data),'boundsMm':[v.min(0).tolist(),v.max(0).tolist()]});buffers.append(data.tobytes());offset+=data.nbytes;checks.append({'component':item['sourceComponent'],'faces':len(mesh.faces),'float32ErrorMm':error,'pass':True})
 # Printed cover marks are separate transparent vector art, never a screenshot
 # of firmware or a photograph of the cover material.
 width,height,_=plan['presentation']['bodySizeMm'];shapes=[]
 for name,x,y,w,h,color in plan['coverMarksMm']:
  shapes.append(f'<rect x="{x+width/2-w/2}" y="{height/2-y-h/2}" width="{w}" height="{h}" rx="{min(w,h)/2}" fill="{color}"/>')
 (output/'cover-marks.svg').write_text(f'<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="{round(1024*height/width)}" viewBox="0 0 {width} {height}">'+''.join(shapes)+'</svg>\n')
 (output/'geometry.bin').write_bytes(b''.join(buffers));meta={'schemaVersion':'1','id':pid,'unit':'mm','binary':'geometry.bin','strideFloats':6,'parts':records,'sourceTransform':{'matrixRows':plan['matrixRows'],'scale':1,'note':'Native assembly preserved; +90° rotation about X and centering only. No resizing or individual-component moves.'},'omittedComponents':[],'dimensions':plan['dimensions'],'presentation':plan['presentation'],'derivedParts':plan['derivedParts'],'referencePhotos':[{'file':'reference-front.png','view':'front','sourceId':'burner-photo'}],'provenance':{'kind':'manufacturer-cad','sourceUrl':plan['sourceUrl'],'sourceSha256':plan['sourceSha256'],'sourceLicense':'CC-BY-NC-4.0; 无名科技Nologo; see NOTICE.md','generator':'tools/build-metalio-models.py','tessellation':'STL original triangles; STEP existing OpenCascade reader, 0.05 mm deflection / 0.2 rad angle. Material assignments from manufacturer photos.','sources':plan['sources']},'limitations':plan['limitations'],'artifacts':{}}
 for name in [plan['sourceFile'],'reference-front.png','reference-front-back.png','NOTICE.md']:
  path=folder/name
  if path.exists():
   if path.resolve()!=(output/name).resolve():shutil.copyfile(path,output/name)
   meta['artifacts'][name]=sha(output/name)
 for name in ['geometry.bin','cover-marks.svg']:meta['artifacts'][name]=sha(output/name)
 (output/'model.json').write_text(json.dumps(meta,ensure_ascii=False,indent=2)+'\n');(output/'geometry-checks.json').write_text(json.dumps({'productId':pid,'sourceSha256':plan['sourceSha256'],'components':checks,'scale':1,'sourceGeometryPreserved':True},indent=2)+'\n');print(pid,len(records),'official components',offset,'bytes')
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--output',type=Path,help='Scratch root; default catalog assets');a=p.parse_args()
 for pid in ['metalio-ink4','metalio-ink4-plus']:build(pid,(a.output or ASSETS)/pid)
