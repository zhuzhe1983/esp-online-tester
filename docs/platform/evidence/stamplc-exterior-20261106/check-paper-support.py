"""Run from repository root with tools/requirements-model.txt installed."""
import json,hashlib,numpy as np
from PIL import Image
from scipy.spatial.transform import Rotation
from pathlib import Path
A=Path('web/platform/assets/m5stack-k141');m=json.load(open(A/'model.json'));raw=(A/m['binary']).read_bytes()
results=[]
for label in m['derivedParts']:
 if label['id'] not in ['right-gpio-paper','right-gpio-paper-rim','left-bus-paper','input-side-paper','relay-side-paper']:continue
 g=label['geometry'];im=np.array(Image.open(A/label['texture']).convert('RGBA'));h,w=im.shape[:2];sx,sy=g['sizeMm'];rot=Rotation.from_euler('XYZ',g['rotation']).as_matrix();c=np.array(g['centerMm']);depth=np.full((h,w),-np.inf)
 for part in m['parts']:
  if part['name'] not in ['shell-front','shell-back']:continue
  tri=np.frombuffer(raw,dtype='<f4',offset=part['byteOffset'],count=part['vertexCount']*6).reshape(-1,3,6)[:,:,:3];tri=(tri-c)@rot
  for t in tri:
   xy=np.column_stack(((t[:,0]+sx/2)/sx*w-.5,(sy/2-t[:,1])/sy*h-.5));a,b,q=xy;den=(b[1]-q[1])*(a[0]-q[0])+(q[0]-b[0])*(a[1]-q[1]);
   if abs(den)<1e-9:continue
   x0,x1=max(0,int(np.floor(xy[:,0].min()))),min(w-1,int(np.ceil(xy[:,0].max())));y0,y1=max(0,int(np.floor(xy[:,1].min()))),min(h-1,int(np.ceil(xy[:,1].max())))
   if x1<x0 or y1<y0:continue
   yy,xx=np.mgrid[y0:y1+1,x0:x1+1];u=((b[1]-q[1])*(xx-q[0])+(q[0]-b[0])*(yy-q[1]))/den;v=((q[1]-a[1])*(xx-q[0])+(a[0]-q[0])*(yy-q[1]))/den;f=1-u-v;z=u*t[0,2]+v*t[1,2]+f*t[2,2];patch=depth[y0:y1+1,x0:x1+1];np.maximum(patch,np.where((u>=-1e-8)&(v>=-1e-8)&(f>=-1e-8),z,-np.inf),out=patch)
 act=im[:,:,3]>=128;z=depth[act]
 row={'id':label['id'],'alphaPixels':int(act.sum()),'intersectionsAbovePaper':int((z>.002).sum()),'unsupportedPixels':int((z<-.05).sum()),'supportOffsetRangeMm':[-float(z.max()),-float(z.min())]}
 results.append(row)
 assert row['intersectionsAbovePaper']==0 and row['unsupportedPixels']==0,row
print(json.dumps({'modelSha256':hashlib.sha256((A/'model.json').read_bytes()).hexdigest(),'method':'Per-alpha-pixel ray raster against original shell-front/shell-back triangles, after transforming to each paper plane. Depth measured along actual outward normal; no same-model source silhouette.','checks':results,'limitations':['Registration is photo-estimated, not manufacturing metrology. Original-pixel right label is split at the X=35.8/36.0 native CAD step.']},indent=2))
