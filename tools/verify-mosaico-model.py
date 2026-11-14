#!/usr/bin/env python3
"""Repeat the limited Mosaico photo-landmark comparison; requires numpy/scipy."""
import numpy as np,json,math,pathlib
from scipy.optimize import least_squares
from scipy.spatial.transform import Rotation
R=pathlib.Path(__file__).resolve().parents[1];E=R/'docs/platform/evidence/mosaico-model';E.mkdir(parents=True,exist_ok=True)
# Virtual sharp corners extrapolated from straight glass edges, in original PNG pixels.
views={
 'front':dict(width=647,height=688,glass=[[20,117],[352,198],[352,667],[20,507]],shell=[[17,105],[365,191],[365,681],[17,514]]),
 'isometric':dict(width=585,height=642,glass=[[85,140],[495,25],[493,486],[19,575]],shell=[[76,132],[500,13],[505,504],[13,586]])}
world=np.array([[-21.65,21.65,6.55],[21.65,21.65,6.55],[21.65,-21.65,6.55],[-21.65,-21.65,6.55]])
out={}
for name,v in views.items():
 target=np.array(v['glass']);wh=np.array([v['width'],v['height']]);
 def project(q,pts):
  p=Rotation.from_rotvec(q[:3]).apply(pts)+q[3:6];return p[:,:2]/p[:,2:]*q[6]+wh/2
 def residual(q):return (project(q,world)-target).ravel()
 best=None
 for rx in [2.7,3.2,3.6]:
  for ry in [-.8,0,.8]:
   q=least_squares(residual,[rx,ry,.1,0,0,150,1300],max_nfev=3000,bounds=([-6,-6,-6,-300,-300,70,250],[6,6,6,300,300,1000,5000]))
   if best is None or np.linalg.norm(q.fun)<np.linalg.norm(best.fun):best=q
 q=best.x;rot=Rotation.from_rotvec(q[:3]).as_matrix();mat=np.eye(4);mat[:3,:3]=np.diag([1,-1,-1])@rot;mat[:3,3]=np.diag([1,-1,-1])@q[3:6]
 shell=np.array([[-22.5,22.5,6.25],[22.5,22.5,6.25],[22.5,-22.5,6.25],[-22.5,-22.5,6.25]])
 pred=project(q,shell);error=np.linalg.norm(pred-np.array(v['shell']),axis=1)
 result={**v,'focalPx':q[6],'matrixRows':mat.tolist(),'calibration':'Camera fitted only to official 43.3mm cover-glass square. Four manually extrapolated corners +/-5px; straight-edge source image perspective is imperfect. Model geometry never stretched.','calibrationRmsPx':float(np.sqrt(np.mean(best.fun**2))),'shellPredicted':pred.tolist(),'shellCornerErrorPx':error.tolist(),'shellCornerMeanErrorPx':float(error.mean()),'status':'partial; silhouette landmarks only, not a manufacturing dimension or full photo acceptance'};out[name]=result
 pts=lambda x:' '.join(','.join(f'{a:.2f}' for a in p) for p in x)
 svg=f'<svg xmlns="http://www.w3.org/2000/svg" width="{v["width"]}" height="{v["height"]}" viewBox="0 0 {v["width"]} {v["height"]}"><rect width="100%" height="100%" fill="#22292c"/><polyline points="{pts(v["shell"]+[v["shell"][0]])}" fill="none" stroke="#45d5d9" stroke-width="3"/><polyline points="{pts(pred.tolist()+[pred[0].tolist()])}" fill="none" stroke="#ee874e" stroke-width="3"/><polyline points="{pts(v["glass"]+[v["glass"][0]])}" fill="none" stroke="#bbbbbb" stroke-width="1"/><text x="10" y="25" fill="white" font-family="sans-serif" font-size="15">Cyan: source shell landmarks; orange: model projection</text><text x="10" y="46" fill="white" font-family="sans-serif" font-size="14">Mean corner deviation: {error.mean():.1f}px; approximate only</text></svg>'
 (E/f'{name}-outline-overlay.svg').write_text(svg)
json.dump(out,open(E/'alignment.json','w'),indent=2);print(json.dumps({k:{'fit':v['calibrationRmsPx'],'shell':v['shellCornerMeanErrorPx']} for k,v in out.items()}))
