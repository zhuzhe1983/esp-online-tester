#!/usr/bin/env python3
"""Check every opaque ATOM label texel against assembled physical triangles.
Input comes from verify-m5-label-surfaces.mjs; render magenta only on geometric penetration.
The positive gap report is a model-space measurement, not a hardware fit claim.
"""
import pathlib,json,numpy as np
from PIL import Image
import sys,hashlib
support=pathlib.Path(sys.argv[1]);rows=[]
for f in support.glob('m5stack-*.json'):
 m=json.load(open(f));d=m['label']; root=pathlib.Path(m['root']).parent;im=np.array(Image.open(root/d['texture']).convert('RGBA'));H,W=im.shape[:2];alpha=im[:,:,3]>=128
 w,h=d['geometry']['sizeMm'];tri=np.fromfile(f.with_suffix('.bin'),dtype='<f4').reshape(-1,3,3).astype(np.float64)
 zbuf=np.full((H,W),-np.inf);partbuf=np.full((H,W),-1,dtype=np.int32)
 for ti,t in enumerate(tri):
  px=(t[:,0]/w+.5)*W-.5;py=(.5-t[:,1]/h)*H-.5;z=t[:,2]
  left=max(0,int(np.ceil(px.min()-1e-7)));right=min(W-1,int(np.floor(px.max()+1e-7)));top=max(0,int(np.ceil(py.min()-1e-7)));bottom=min(H-1,int(np.floor(py.max()+1e-7)))
  if right<left or bottom<top:continue
  denom=(py[1]-py[2])*(px[0]-px[2])+(px[2]-px[1])*(py[0]-py[2])
  if abs(denom)<1e-10:continue
  xx,yy=np.meshgrid(np.arange(left,right+1),np.arange(top,bottom+1));a=((py[1]-py[2])*(xx-px[2])+(px[2]-px[1])*(yy-py[2]))/denom;b=((py[2]-py[0])*(xx-px[2])+(px[0]-px[2])*(yy-py[2]))/denom;c=1-a-b
  inside=(a>=-1e-7)&(b>=-1e-7)&(c>=-1e-7);depth=a*z[0]+b*z[1]+c*z[2];view=zbuf[top:bottom+1,left:right+1];update=inside&(depth>view);view[update]=depth[update];partbuf[top:bottom+1,left:right+1][update]=ti
 gaps=-zbuf[alpha];finite=np.isfinite(gaps);q=np.quantile(gaps[finite],[0,.01,.5,.99,1]);intrusions=int((gaps<-.005).sum());missed=int((~finite).sum());bad=zbuf>.005
 rgb=im.copy();rgb[:,:,3]=np.where(alpha,255,0);rgb[alpha&bad,:3]=[255,0,120];Image.fromarray(rgb).save(support/(m['id']+'-support.png'))
 ids,counts=np.unique(partbuf[alpha],return_counts=True);parts=[]
 for index,count in zip(ids,counts):
  if index<0:continue
  n=next(x['name']for x in m['ranges']if x['startTriangle']<=index<x['endTriangle']);parts.append((n,int(count)))
 from collections import Counter
 c=Counter()
 for n,v in parts:c[n]+=v
 row={'id':m['id'],'label':d['id'],'texturePixels':[W,H],'opaquePixels':int(alpha.sum()),'alphaThreshold':128,'missedPixels':missed,'intrudingPixels':intrusions,'gapMmQuantiles':dict(zip(['min','p01','median','p99','max'],q.tolist())),'supportParts':dict(c),'method':'Every source-texture pixel with alpha>=128 is checked at its center against assembled physical triangles transformed into label coordinates. Barycentric orthographic ray/surface intersection; max local-Z gives nearest outward support, without a one-sided-normal assumption. No textured label, UNKNOWN text, live framebuffer or invisible input plane supplies support.'};rows.append(row);print(json.dumps(row))
json.dump(rows,open(support/'results.json','w'),indent=2)
