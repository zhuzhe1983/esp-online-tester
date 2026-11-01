"""Fit manufacturer MainBody and light-guide circular optical surfaces in world XY.
No vertices are changed; rigid reference matrices are recorded in the section receipt.
Requires numpy and trimesh. Pass public asset directory plus section receipt path.
"""
import argparse,json,hashlib
from pathlib import Path
import numpy as np,trimesh
p=argparse.ArgumentParser();p.add_argument('assets');p.add_argument('receipt');p.add_argument('--output',default='/tmp/stackchan-rgb-circle-fit.json');a=p.parse_args();root=Path(a.assets);receipt=json.load(open(a.receipt));out={}
for side,sgn in [('left',-1),('right',1)]:
 out[side]={}
 for item in ['shell',side]:
  q=receipt['sources'][item];path=root/q['file'];assert hashlib.sha256(path.read_bytes()).hexdigest()==q['sha256']
  m=trimesh.load(path,process=True);m.apply_transform(q['worldMatrixRows']);n=m.face_normals;cent=m.triangles_center
  selected=(n[:,0]*sgn>.05)&(n[:,1]>.05)&(abs(n[:,2])<1e-4)&(cent[:,0]*sgn>23.5)&(cent[:,1]>23.5)
  xy=np.unique(np.round(m.triangles[selected,:,:2].reshape(-1,2),7),axis=0)
  c=np.linalg.lstsq(np.c_[2*xy[:,0],2*xy[:,1],np.ones(len(xy))],(xy*xy).sum(axis=1),rcond=None)[0];r=np.sqrt(c[2]+c[0]**2+c[1]**2);res=np.linalg.norm(xy-c[:2],axis=1)-r
  out[side]['shell'if item=='shell'else'guide']={'sourceSha256':q['sha256'],'faceCount':int(selected.sum()),'uniqueXYPoints':len(xy),'centerMm':c[:2].tolist(),'radiusMm':float(r),'maximumAbsoluteRadialResidualMm':float(abs(res).max()),'rmsRadialResidualMm':float(np.sqrt(np.mean(res**2)))}
 sc=np.array(out[side]['shell']['centerMm']);gc=np.array(out[side]['guide']['centerMm']);out[side]['guideCenterAlignmentDeltaMm']=(sc-gc).tolist();out[side]['previousCandidateDeltaMm']=[-sgn*.15,-.15];out[side]['candidateCenterOffsetFromShellMm']=(gc+[-sgn*.15,-.15]-sc).tolist()
result={'method':'Least-squares circle fit to original manufacturer STL face vertices. Faces have outward corner XY normals, abs(nz)<1e-4, abs(x)>23.5 and y>23.5; no mesh editing. Section receipt records exact source SHA256 and world matrices.','measurements':out}
Path(a.output).write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
