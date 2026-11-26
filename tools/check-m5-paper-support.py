#!/usr/bin/env python3
"""Audit every effective-alpha pixel against facing native CAD and paper stock.
This measures geometry separation, not actual adhesive or paper thickness.
"""
import argparse, hashlib, json
from pathlib import Path
import numpy as np
from PIL import Image
ROOT=Path(__file__).resolve().parents[1]
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def audit(base):
 m=json.loads((base/'model.json').read_text());binary=(base/'geometry.bin').read_bytes();rows=[]
 for label in [p for p in m['derivedParts'] if p['kind']=='label']:
  g=label['geometry'];w,h=g['sizeMm'];cx,cy,z=g['centerMm'];rotation=g.get('rotation',[0,0,0]);assert abs(rotation[0])+abs(rotation[2])<1e-8
  sign=-1 if rotation[1]>1 else 1
  im=np.array(Image.open(base/label['texture']).convert('RGBA'));H,W=im.shape[:2];alpha=im[:,:,3]>=128;surfaces=[]
  for role,parts in [('native-case',[p for p in m['parts'] if p.get('sourceComponents')]),('paper-stock',[p for p in m['parts'] if p['name']==label['id']+'-stock'])]:
   if not parts:continue
   support=np.full((H,W),-np.inf)
   for part in parts:
    a=np.frombuffer(binary,dtype='<f4',count=part['vertexCount']*6,offset=part['byteOffset']).reshape(-1,3,6)
    for triangle in a:
     pos=triangle[:,:3];normal=triangle[:,3:].mean(0)
     if sign*normal[2]<1e-6:continue
     xs=(sign*(pos[:,0]-cx)/w+.5)*W-.5;ys=(.5-(pos[:,1]-cy)/h)*H-.5
     x0=max(0,int(np.floor(xs.min())));x1=min(W-1,int(np.ceil(xs.max())));y0=max(0,int(np.floor(ys.min())));y1=min(H-1,int(np.ceil(ys.max())))
     if x0>x1 or y0>y1:continue
     den=(ys[1]-ys[2])*(xs[0]-xs[2])+(xs[2]-xs[1])*(ys[0]-ys[2])
     if abs(den)<1e-10:continue
     yy,xx=np.mgrid[y0:y1+1,x0:x1+1];b0=((ys[1]-ys[2])*(xx-xs[2])+(xs[2]-xs[1])*(yy-ys[2]))/den;b1=((ys[2]-ys[0])*(xx-xs[2])+(xs[0]-xs[2])*(yy-ys[2]))/den;b2=1-b0-b1;inside=(b0>=-1e-7)&(b1>=-1e-7)&(b2>=-1e-7)
     zz=sign*(b0*pos[0,2]+b1*pos[1,2]+b2*pos[2,2]);view=support[y0:y1+1,x0:x1+1];np.maximum(view,np.where(inside,zz,-np.inf),out=view)
   gaps=(sign*z-support)[alpha];valid=np.isfinite(gaps);values,counts=np.unique(np.round(gaps[valid],3),return_counts=True)
   surfaces.append(dict(role=role,partNames=[p['name'] for p in parts],noSupportPixels=int((~valid).sum()),intersectingPixels=int((gaps[valid]<-1e-4).sum()),gapHistogram=[dict(gapMm=float(v),pixels=int(c),fraction=float(c/alpha.sum())) for v,c in zip(values,counts)]))
  rows.append(dict(label=label['id'],texture=label['texture'],textureSha256=sha(base/label['texture']),labelPlaneZMm=z,opaquePixels=int(alpha.sum()),surfaces=surfaces))
 return dict(productId=m['id'],modelSha256=sha(base/'model.json'),geometrySha256=sha(base/'geometry.bin'),results=rows)
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('ids',nargs='+');p.add_argument('--assets',type=Path,default=ROOT/'web/platform/assets');p.add_argument('--output',type=Path,required=True);a=p.parse_args();results=[audit(a.assets/id) for id in a.ids]
 out=dict(schemaVersion=1,method='Exact triangle barycentric depth at every effective alpha pixel center(alpha>=128, matching alphaTest0.5). Front/back rotations honored. Facing native triangles retained at source coordinates, not projected bounding boxes.',interpretation='These distances do not establish physical paper/adhesive thickness.0.2mm recess separation alone does not prove floating paper or justify lowering it across rims. Paper-stock parts where present are explicit estimated0.01mm display sheets with0.005mm image-face separation; no plastic support is invented.',results=results);a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(json.dumps(out,indent=2)+'\n')
 for r in results:
  for label in r['results']:print(r['productId'],label['label'],[(s['role'],s['noSupportPixels'],s['intersectingPixels'],s['gapHistogram'][:5]) for s in label['surfaces']])
if __name__=='__main__':main()
