#!/usr/bin/env python3
"""Offline photo-surface repairs; original geometry is copied, never regenerated.
Every warp/mask/placement is explicit in the SKU's surface-repairs.json.
"""
import argparse,copy,hashlib,json,pathlib,shutil
import cv2
import numpy as np
from PIL import Image,ImageDraw
ROOT=pathlib.Path(__file__).resolve().parents[1]
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def write(path,value):path.write_text(json.dumps(value,ensure_ascii=False,indent=2)+'\n')
def apply(model,spec):
 model=copy.deepcopy(model)
 if spec.get('omitRawParts'):model['parts']=[p for p in model['parts']if p['name']not in spec['omitRawParts']]
 for change in spec.get('derivedParts',[]):
  current=next((p for p in model.get('derivedParts',[])if p['id']==change['id']),None)
  if current is None:model.setdefault('derivedParts',[]).append(copy.deepcopy(change))
  else:current.update(copy.deepcopy(change))
 for name,extra in spec.get('inspectionGroupParts',{}).items():
  group=next(g for g in model['presentation']['inspectionGroups']if g['id']==name)
  group['parts']=list(dict.fromkeys(group['parts']+extra))
 model['limitations']=[spec.get('limitationsReplacements',{}).get(item,item)for item in model.get('limitations',[])]
 for item in spec.get('limitations',[]):
  if item not in model.setdefault('limitations',[]):model['limitations'].append(item)
 model['surfaceRepairs']='surface-repairs.json'
 return model

def build(identity,out,repo):
 src=repo/'web/platform/assets'/identity;spec=json.loads((src/'surface-repairs.json').read_text());assert spec['id']==identity
 dest=out/identity
 if dest.resolve()==src.resolve()or dest.exists()and any(dest.iterdir()):raise ValueError('Choose an empty output outside source assets')
 dest.mkdir(parents=True,exist_ok=True)
 model=json.loads((src/'model.json').read_text());plan=json.loads((src/'model-plan.json').read_text());binary=src/model['binary'];assert sha(binary)==spec['originalGeometrySha256'],'Original geometry changed'
 # Copy retained original/source data; no downloading or CAD export is performed.
 for file in src.iterdir():
  if file.is_file()and file.name not in ['model.json','model-plan.json',*[t['file']for t in spec['textures']]]:shutil.copyfile(file,dest/file.name)
 receipts=[]
 for t in spec['textures']:
  image=src/t['sourceFile'];assert sha(image)==t['sourceSha256'],'Source image changed: '+t['sourceFile']
  rgb=np.asarray(Image.open(image).convert('RGBA'));w,h=t['outputPixels'];quad=np.float32(t['sourceQuadPixels']);target=np.float32([[0,0],[w-1,0],[w-1,h-1],[0,h-1]])
  matrix=cv2.getPerspectiveTransform(quad,target);pixels=cv2.warpPerspective(rgb,matrix,(w,h),flags=cv2.INTER_LANCZOS4,borderMode=cv2.BORDER_CONSTANT);im=Image.fromarray(pixels);factor=4
  mask=Image.new('L',(w*factor,h*factor),0);draw=ImageDraw.Draw(mask)
  if not t.get('includePolygonsOutput'):draw.rectangle((0,0,w*factor,h*factor),fill=255)
  for poly in t.get('includePolygonsOutput',[]):draw.polygon([(round(x*factor),round(y*factor))for x,y in poly],fill=255)
  for poly in t.get('excludePolygonsOutput',[]):draw.polygon([(round(x*factor),round(y*factor))for x,y in poly],fill=0)
  for cx,cy,r in t.get('excludeCirclesOutput',[]):draw.ellipse(tuple(round(v*factor)for v in[cx-r,cy-r,cx+r,cy+r]),fill=0)
  alpha=np.array(mask.resize((w,h),Image.Resampling.LANCZOS))
  for region in t.get('excludeNearWhiteEdgeOutput',[]):
   x0,y0,x1,y1=region['xyxy'];area=pixels[y0:y1,x0:x1,:3];alpha[y0:y1,x0:x1][np.all(area>=region['minChannel'],axis=2)]=0
  im.putalpha(Image.fromarray(alpha));im.save(dest/t['file'],optimize=True)
  receipts.append({'file':t['file'],'sha256':sha(dest/t['file']),'sourceFile':t['sourceFile'],'sourceSha256':t['sourceSha256'],'sourceQuadPixels':t['sourceQuadPixels'],'outputPixels':t['outputPixels'],'sourceToOutputMatrix':matrix.tolist(),'alphaNonzeroPixels':int(np.count_nonzero(np.asarray(im)[:,:,3]))})
 model=apply(model,spec);plan=apply(plan,spec)
 model.setdefault('artifacts',{}).update({t['file']:sha(dest/t['file'])for t in spec['textures']});model['artifacts']['surface-repairs.json']=sha(src/'surface-repairs.json')
 model['provenance']['surfaceRepairGenerator']='tools/rebuild-m5-unit-photo-surfaces.py';plan.setdefault('provenance',{})['surfaceRepairGenerator']='tools/rebuild-m5-unit-photo-surfaces.py'
 write(dest/'model.json',model);write(dest/'model-plan.json',plan)
 receipt={'id':identity,'scope':'Rebuilds the new exact-source photo surfaces and declarative placements, copies unchanged prior approximation geometry. Not new CAD, browser GPU or physical validation.','originalGeometrySha256':sha(binary),'rebuiltGeometrySha256':sha(dest/model['binary']),'modelSha256':sha(dest/'model.json'),'planSha256':sha(dest/'model-plan.json'),'textures':receipts,'inspectionNotes':spec.get('inspectionNotes',[])};write(dest/'surface-rebuild-receipt.json',receipt);return receipt
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--repo',type=pathlib.Path,default=ROOT);p.add_argument('--output',type=pathlib.Path,required=True);p.add_argument('ids',nargs='+');a=p.parse_args()
 for identity in a.ids:print(json.dumps(build(identity,a.output,a.repo),ensure_ascii=False))
