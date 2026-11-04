#!/usr/bin/env python3
"""Check explicit StampFly harness centerlines against retained native geometry.
Requires tools/requirements-model.txt. Source/photo routing accuracy is separate.
"""
import hashlib,json
from pathlib import Path
import numpy as np,trimesh
from scipy.spatial.distance import cdist
ROOT=Path(__file__).resolve().parents[1]
def dense(points,step=.08):
 out=[]
 for a,b in zip(points,points[1:]):
  n=max(1,int(np.ceil(np.linalg.norm(b-a)/step)));out.extend(a+(b-a)*t/n for t in range(n))
 return np.asarray(out+[points[-1]])
def read(p):return json.loads(p.read_text())
def verify(sku):
 a=ROOT/'web/platform/assets'/sku;plan=read(a/'model-plan.json');meta=read(a/'model.json');paths=[dense(np.asarray(t['pointsMm']))for t in plan['tubes']];radii=[t['radiusMm']for t in plan['tubes']];receipts=[]
 for source_id in ['ecosystem-frame-stl','ecosystem-pcb-stl','ecosystem-battery-stl']:
  source=next(s for s in meta['provenance']['sources']if s['id']==source_id);candidates=[a/name for name,h in meta['artifacts'].items()if h==source['sha256']]
  if not candidates:
   original=ROOT/'web/platform/assets/m5stack-k138';original_meta=read(original/'model.json');candidates=[original/name for name,h in original_meta['artifacts'].items()if h==source['sha256']]
  assert candidates,'Pinned obstacle source unavailable';path=candidates[0];raw=path.read_bytes();assert hashlib.sha256(raw).hexdigest()==source['sha256'];mesh=trimesh.load(path,process=False);mesh.apply_transform(plan['matrixRows'])
  # Spatially exclude remote rotor guards without modifying their triangles.
  bounds=np.concatenate(paths);lo=bounds.min(0)-3;hi=bounds.max(0)+3;tri=mesh.triangles;keep=((tri.max(1)>=lo)&(tri.min(1)<=hi)).all(1);mesh=trimesh.Trimesh(vertices=tri[keep].reshape(-1,3),faces=np.arange(sum(keep)*3).reshape(-1,3),process=False)
  for i,points in enumerate(paths):
   distances=[]
   for j in range(0,len(points),32):distances.extend(trimesh.proximity.closest_point_naive(mesh,points[j:j+32])[1])
   clearance=float(min(distances)-radii[i]-.04) # Half sample spacing conservatively removed.
   assert clearance>0,(sku,source_id,i,clearance)
   receipts.append({'wire':i,'sourceId':source_id,'sourceSha256':source['sha256'],'sampleCount':len(points),'conservativeSurfaceClearanceMm':clearance,'note':'Includes source battery as a conservative obstacle even for battery-removed v1.1 display.'})
 pairs=[]
 for i in range(6):
  for j in range(i+1,6):
   minimum=float(cdist(paths[i],paths[j]).min()-radii[i]-radii[j]-.08);assert minimum>0,(sku,'wires',i,j,minimum);pairs.append({'wires':[i,j],'conservativeClearanceMm':minimum})
 return {'productId':sku,'modelSha256':hashlib.sha256((a/'model.json').read_bytes()).hexdigest(),'geometrySha256':meta['artifacts']['geometry.bin'],'maximumSampleSpacingMm':.08,'nativeObstacles':receipts,'wirePairs':pairs,'passed':True,'scope':'Numerical nonintersection of authored geometry; not validation of unknown hidden real-world routing, actual wire length or manufacturing fit. Plug insertion is checked separately by verify-stampfly-optical-assembly.py.'}
if __name__=='__main__':print(json.dumps([verify(x)for x in ['m5stack-k138','m5stack-k138-v11']],indent=2))
