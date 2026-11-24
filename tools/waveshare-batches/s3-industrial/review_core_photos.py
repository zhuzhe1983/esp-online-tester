#!/usr/bin/env python3
"""Exact-option reference overlays on unmodified photos; manual planar projection only."""
from pathlib import Path
import argparse,hashlib,json,textwrap
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
import numpy as np
from PIL import Image
from scipy.spatial import ConvexHull,QhullError
ROOT=Path(__file__).resolve().parents[3]
p=argparse.ArgumentParser();p.add_argument('--meshes',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args();a.output.mkdir(parents=True,exist_ok=True)
def hmatrix(model,pixels):
 rows=[];target=[]
 for(x,y),(u,v)in zip(model,pixels):rows.extend([[x,y,1,0,0,0,-u*x,-u*y],[0,0,0,x,y,1,-v*x,-v*y]]);target.extend([u,v])
 return np.append(np.linalg.solve(rows,target),1).reshape(3,3)
regs={
 'dev-kit-n16r8':[[500,920],[217,713],[1063,263],[1350,447]],
 'dev-kit-n16r8-m':[[507,850],[290,657],[1047,263],[1280,443]],
 'dev-kit-n32r16v':[[473,913],[240,700],[1020,283],[1273,463]],
 'dev-kit-n32r16v-m':[[487,897],[227,687],[1047,233],[1327,407]],
 'dev-kit-n16r8-u':[[573,507],[683,610],[260,867],[150,750]],
 'dev-kit-n16r8-um':[[583,453],[700,563],[250,813],[130,697]],
 'zero-n8r8':[[80,27],[109,48],[64,79],[34,60]],
 'zero-n8r8-m':[[533,74],[730,216],[415,392],[220,238]],
}
records=[]
for suffix,pixels in regs.items():
 mid='waveshare-esp32-s3-'+suffix;d=ROOT/'web/platform/assets'/mid;plan=json.loads((d/'model-plan.json').read_text());src=next(x for x in plan['sources']if x['id']=='exact-variant-photo');im=Image.open(d/src['file']);rec=json.loads((a.meshes/(mid+'.json')).read_text());zero=suffix.startswith('zero');corners=[[-9,11.75],[9,11.75],[9,-11.75],[-9,-11.75]]if zero else[[-11,29.5],[11,29.5],[11,-29.5],[-11,-29.5]];matrix=hmatrix(corners,pixels)
 note='Exact option photo. Four manually selected '+('PCB corners'if zero else'mounting-hole centers')+' register only the PCB plane. Components project their actual loader XY footprint; photographed elevated tops/parallax and uncertain height are not a dimensional fit. Unknown small parts, silkscreen and reverse face are not invented.'
 if suffix=='zero-n8r8':note+=' The original bare-option thumbnail is low resolution; no fine-component alignment claim.'
 if suffix.endswith(('-u','-um')):note+=' Coax/antenna remain source reference only; U-end notch is separately checked by actual mesh rays.'
 fig,axes=plt.subplots(1,2,figsize=(12,8));projected=[]
 for ax in axes:ax.imshow(im);ax.axis('off')
 axes[0].set_title('Unmodified exact-SKU reference');axes[1].set_title('Loader footprints / manual board-plane registration')
 for mesh in rec['meshes']:
  if mesh.get('unknown')or mesh['name'].startswith('header-'):continue
  pts=np.unique(np.asarray(mesh['position']).reshape(-1,3)[:,:2],axis=0)
  if len(pts)<3:continue
  try:points=pts[ConvexHull(pts).vertices]
  except QhullError:continue
  pixel=np.column_stack([points,np.ones(len(points))])@matrix.T;pixel=pixel[:,:2]/pixel[:,2:3];pixel=np.vstack([pixel,pixel[0]]);axes[1].plot(pixel[:,0],pixel[:,1],linewidth=.9,alpha=.9);projected.append(mesh['name']or'PCB-body')
 axes[1].scatter(*np.asarray(pixels).T,s=16,color='#d53e4f');fig.suptitle(mid,fontsize=11,y=.98);fig.text(.5,.94,textwrap.fill(note,125),ha='center',va='top',fontsize=9);fig.tight_layout(rect=(0,0,1,.84));name=mid+'-exact-photo-overlay.png';fig.savefig(a.output/name,dpi=120);plt.close(fig)
 records.append({'model':mid,'modelSha256':hashlib.sha256((d/'model.json').read_bytes()).hexdigest(),'sourceId':src['id'],'sourceSha256':src['sha256'],'modelPointsMm':corners,'sourcePixels':pixels,'homogeneousPixelFromModelXY':matrix.tolist(),'projectedParts':projected,'overlay':name,'notes':note,'scope':'Manual unmodified-photo review only; depth/material/physical fit and execution not certified.'})
(a.output/'exact-photo-registration.json').write_text(json.dumps({'records':records},indent=2)+'\n');print('8 exact-option overlays written.')
