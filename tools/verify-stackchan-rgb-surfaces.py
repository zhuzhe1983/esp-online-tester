#!/usr/bin/env python3
"""Prove exported RGB vertices stay on original CAD triangles and conserve area.
Requires tools/requirements-model.txt; no browser, downloaded source or renderer mock.
"""
import argparse,hashlib,json
from pathlib import Path
import numpy as np,trimesh
ROOT=Path(__file__).resolve().parents[1];A=ROOT/'web/platform/assets/m5stack-k151'
def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def triangles(meta,binary):return np.frombuffer(binary,dtype='<f4',count=meta['vertexCount']*6,offset=meta['byteOffset']).reshape(-1,3,6)[:,:,:3].astype(float)
def area(t):return np.linalg.norm(np.cross(t[:,1]-t[:,0],t[:,2]-t[:,0]),axis=1).sum()/2
def volume(t):return np.einsum('ij,ij->i',t[:,0],np.cross(t[:,1],t[:,2])).sum()/6
def surface_distance(points,t):
 a=t[:,0];v=t[:,1]-a;w=t[:,2]-a;n=np.cross(v,w);n/=np.linalg.norm(n,axis=1)[:,None];vv=np.einsum('ij,ij->i',v,v);vw=np.einsum('ij,ij->i',v,w);ww=np.einsum('ij,ij->i',w,w);den=vv*ww-vw*vw;maximum=0
 for p in points:
  q=p-a;qv=np.einsum('ij,ij->i',q,v);qw=np.einsum('ij,ij->i',q,w);u=(ww*qv-vw*qw)/den;s=(vv*qw-vw*qv)/den;inside=(u>=-2e-5)&(s>=-2e-5)&(u+s<=1+2e-5)
  if not inside.any():raise ValueError('Exported point is outside every original crown triangle')
  maximum=max(maximum,float(np.abs(np.einsum('ij,ij->i',q,n))[inside].min()))
 return maximum
p=argparse.ArgumentParser();p.add_argument('--output',required=True);args=p.parse_args();m=json.loads((A/'model.json').read_text());binary=(A/'geometry.bin').read_bytes();M=np.array(m['sourceTransform']['matrixRows']);report={'geometrySha256':digest(A/'geometry.bin'),'modelSha256':digest(A/'model.json'),'method':'Inverse rigid transformation of exported vertices, triangle centroids and edge midpoints; barycentric containment and plane distance against original STL crown triangles. Source/cut surfaces retain area and signed volume without caps. Float32 export tolerance1e-5mm.','sides':{}}
for side in ['left','right']:
 body=next(q for q in m['parts']if q['name']==f'light-guide-{side}');source=A/('source-'+body['sourceId']+'.stl');mesh=trimesh.load(source).split(only_watertight=False)[0];selected=(mesh.face_normals[:,0]<-.5)&(mesh.triangles[:,:,0].max(1)<.6);original=mesh.triangles[selected];effective=M@np.array(body['matrixRows']);inverse=np.linalg.inv(effective);allparts=[body]+[q for q in m['parts']if q['name'].startswith(f'rgb-window-{side}-')];allworld=np.concatenate([triangles(q,binary)for q in allparts]);native=allworld@inverse[:3,:3].T+inverse[:3,3];parts=[];optical_area=0
 for part in allparts[1:]:
  world=triangles(part,binary);native_part=world@inverse[:3,:3].T+inverse[:3,3];samples=np.concatenate([native_part.reshape(-1,3),native_part.mean(axis=1),(native_part+np.roll(native_part,1,axis=1)).reshape(-1,3)/2]);distance=surface_distance(np.unique(samples,axis=0),original);clip=part['sourcePlaneClip'];bounds=[float(native_part[:,:,1].min()),float(native_part[:,:,1].max())];ar=float(area(native_part));optical_area+=ar
  assert distance<1e-5 and bounds[0]>=clip['minMm']-1e-5 and bounds[1]<=clip['maxMm']+1e-5
  parts.append({'name':part['name'],'areaMm2':ar,'nativeLongAxisBoundsMm':bounds,'clipBoundsMm':[clip['minMm'],clip['maxMm']],'maximumDistanceToSourceTriangleMm':distance,'triangles':len(world)})
 body_indices={i for lo,hi in body['sourceFaceRanges']for i in range(lo,hi)};crown_indices=set(np.where(selected)[0]);assert not body_indices&crown_indices and body_indices|crown_indices==set(range(len(mesh.faces)))
 errors={'opticalAreaMm2':abs(optical_area-float(area(original))),'wholeGuideAreaMm2':abs(float(area(native))-mesh.area),'wholeGuideSignedVolumeMm3':abs(float(volume(native))-mesh.volume)}
 assert max(errors.values())<.001
 report['sides'][side]={'sourceSha256':digest(source),'originalOpticalAreaMm2':float(area(original)),'exportedOpticalAreaMm2':optical_area,'sourceFacePartitionComplete':True,'errors':errors,'parts':parts}
report['passed']=True;Path(args.output).write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({'passed':True,'errors':{s:r['errors']for s,r in report['sides'].items()}},indent=2))
