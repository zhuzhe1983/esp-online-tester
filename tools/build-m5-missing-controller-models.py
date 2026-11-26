#!/usr/bin/env python3
"""Rebuild five source-described M5 controller previews from retained inputs.
No networking, guessed PCB placement or firmware images. Explicit model plans
record all dimensions, CAD selection/transforms and photo UV crops.
"""
import argparse,hashlib,importlib.util,json,math,shutil
from pathlib import Path
import cv2
import numpy as np
from PIL import Image,ImageDraw
import trimesh
ROOT=Path(__file__).resolve().parents[1]
spec=importlib.util.spec_from_file_location('cad_export',ROOT/'tools/build-waveshare-models.py');cad=importlib.util.module_from_spec(spec);spec.loader.exec_module(cad)
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def write(p,data):p.write_text(json.dumps(data,ensure_ascii=False,indent=2)+'\n')
def build(id,out):
 src=ROOT/'web/platform/assets'/id;plan=json.loads((src/'model-plan.json').read_text());dest=out/id;dest.mkdir(parents=True,exist_ok=True)
 assert src.resolve()!=dest.resolve(),'Use an independent output directory.'
 artifacts={};sources={}
 for s in plan['sources']:
  p=src/s['file'];assert sha(p)==s['sha256'] and p.stat().st_size==s['bytes'],p
  shutil.copyfile(p,dest/p.name);artifacts[p.name]=s['sha256'];sources[s['id']]=s
 blobs=[];parts=[];cache={};checks=[]
 for part in plan['parts']:
  if part['kind']=='cad-stl':
   filename=part['sourceFile']
   if filename not in cache:cache[filename]=trimesh.load(src/filename,force='mesh').split(only_watertight=False)
   arrays=[];matrix=np.asarray(part['matrixRows'],dtype=float);rot=matrix[:3,:3];assert np.allclose(rot.T@rot,np.eye(3)) and np.isclose(np.linalg.det(rot),1)
   for c in part['components']:
    m=cache[filename][c];vertices=m.vertices@rot.T+matrix[:3,3];normals=m.face_normals@rot.T
    arrays.append(np.concatenate([vertices[m.faces].reshape(-1,3),np.repeat(normals,3,axis=0)],axis=1))
   a=np.concatenate(arrays)
  else:a=cad.tessellate(cad.procedural(part['geometry']),.04,.15)
  arr=a.astype('<f4');blob=arr.tobytes();record={k:v for k,v in part.items() if k not in ['kind','geometry','matrixRows','components']};record.update(byteOffset=sum(map(len,blobs)),vertexCount=len(arr),boundsMm=[arr[:,:3].min(0).tolist(),arr[:,:3].max(0).tolist()]);
  if part['kind']=='cad-stl':record.update(sourceComponents=part['components'],sourceMatrixRows=part['matrixRows'])
  parts.append(record);blobs.append(blob)
  checks.append({'part':record['name'],'basis':part['basis'],'floatExportErrorMm':float(np.max(np.abs(a[:,:3]-arr[:,:3]))),'triangles':len(arr)//3})
 (dest/'geometry.bin').write_bytes(b''.join(blobs));artifacts['geometry.bin']=sha(dest/'geometry.bin')
 textures=[]
 for texture in plan.get('texturePlans',[]):
  img=Image.open(src/texture['sourceFile']).convert('RGBA');size=texture['sizePx'];quad=np.asarray(texture['quadPx'],dtype=np.float32);target=np.array([[0,0],[size[0]-1,0],[size[0]-1,size[1]-1],[0,size[1]-1]],np.float32)
  matrix=cv2.getPerspectiveTransform(quad,target);warped=Image.fromarray(cv2.warpPerspective(np.asarray(img),matrix,tuple(size),flags=cv2.INTER_CUBIC,borderMode=cv2.BORDER_CONSTANT))
  mask=Image.new('L',tuple(size),0);draw=ImageDraw.Draw(mask);draw.rounded_rectangle((0,0,size[0]-1,size[1]-1),radius=texture.get('cornerRadiusPx',0),fill=255)
  for poly in texture.get('excludePolygonsPx',[]):draw.polygon([tuple(p) for p in poly],fill=0)
  warped.putalpha(mask);warped.save(dest/texture['file']);artifacts[texture['file']]=sha(dest/texture['file'])
  textures.append({**texture,'perspectiveMatrix':matrix.tolist(),'sha256':artifacts[texture['file']],'notes':'Exact source photo crop; printed paper only, no shell tint or static display content.'})
 metadata={'schemaVersion':'1','id':id,'unit':'mm','binary':'geometry.bin','strideFloats':6,'parts':parts,'sourceTransform':{'scale':1,'note':plan['assemblyNotes']},'dimensions':plan['dimensions'],'presentation':plan['presentation'],'derivedParts':plan.get('derivedParts',[]),'referencePhotos':plan['referencePhotos'],'textures':textures,'provenance':{'kind':plan['basis'],'sources':plan['sources'],'license':plan['license'],'rebuild':'python tools/build-m5-missing-controller-models.py '+id+' --output /new/scratch/directory'},'limitations':plan['limitations'],'omittedComponents':plan.get('omittedComponents',[]),'artifacts':artifacts}
 write(dest/'model.json',metadata)
 b=np.array([p['boundsMm'] for p in parts]);lo=b[:,0].min(0);hi=b[:,1].max(0)
 write(dest/'build-check.json',{'productId':id,'metadataSha256':sha(dest/'model.json'),'geometrySha256':sha(dest/'geometry.bin'),'parts':checks,'geometryBoundsMm':[lo.tolist(),hi.tolist()],'geometryExtentsMm':(hi-lo).tolist(),'sourceCoordinateScale':1,'nativeCadCoordinatesPreserved':plan['basis']=='manufacturer-cad','manufacturerEnvelopeAcceptance':'not-verified','photoModelManufacturingAccuracy':'not-verified','limits':plan['limitations']})
 print(json.dumps({'productId':id,'parts':len(parts),'geometryBytes':len(b''.join(blobs)),'extentsMm':(hi-lo).tolist()}))
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('ids',nargs='*');p.add_argument('--output',type=Path,required=True);a=p.parse_args()
 for id in a.ids or ['m5stack-k140','m5stack-k016-p2','m5stack-k117','m5stack-k126','m5stack-u195']:build(id,a.output)
if __name__=='__main__':main()
