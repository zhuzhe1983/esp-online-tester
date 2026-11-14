#!/usr/bin/env python3
"""Verify fixed-source geometry; optional photo landmarks require numpy/scipy."""
import argparse,hashlib,json,math,pathlib,struct,importlib.util
parser=argparse.ArgumentParser();parser.add_argument("--source",type=pathlib.Path);parser.add_argument("--photo-alignment",action="store_true");parser.add_argument("--output",type=pathlib.Path);args=parser.parse_args()
R=pathlib.Path(__file__).resolve().parents[1];E=args.output or R/'docs/platform/evidence/mosaico-model-official';E.mkdir(parents=True,exist_ok=True)
asset=R/'web/platform/assets/espressif-esp-mosaico';metadata=json.loads((asset/'model.json').read_text());data=(asset/'geometry.bin').read_bytes()
assert hashlib.sha256(data).hexdigest()==metadata['artifacts']['geometry.bin']
parts={p['name']:p for p in metadata['parts']};shell=parts['shell'];cad=[p for p in metadata['parts'] if p['basis']=='manufacturer-cad']
assert sum(p['vertexCount']//3 for p in cad)==23732
assert [sum(p['vertexCount']//3 for p in cad if p['sourceObjectId']==oid) for oid in [2,4,6]]==[19916,2976,840]
assert metadata['sourceGeometry']['presentationMatrixRows']==[[-1,0,0,0],[0,-1,0,0],[0,0,1,-5.550025463104248],[0,0,0,1]]
cover=parts['front-cover'];cover_size=[cover['boundsMm'][1][i]-cover['boundsMm'][0][i] for i in range(3)]
assert all(abs(a-b)<1e-6 for a,b in zip(cover_size,[43.3,43.3,.7]))
report=dict(schemaVersion=1,artifacts={'model.json':hashlib.sha256((asset/'model.json').read_bytes()).hexdigest(),'geometry.bin':hashlib.sha256(data).hexdigest()},manufacturerTriangles=23732,shellMeasuredMm=[shell['boundsMm'][1][i]-shell['boundsMm'][0][i] for i in range(3)],coverIndependentTargetMm=[43.3,43.3,.7],coverMeasuredMm=cover_size,displayIndependentTargetMm=[38.39,38.39],displayMeasuredMm=metadata['presentation']['screen']['sizeMm'],rawCadCompared=False,wholeProductDimensionAcceptance='unverified; manufacturer file supplies no electronics or complete-product dimension drawing')
if args.source:
 spec=importlib.util.spec_from_file_location('mosaico_builder',R/'tools/build-mosaico-model.py');builder=importlib.util.module_from_spec(spec);spec.loader.exec_module(builder)
 source=builder.source_bytes(args.source,False);meshes=builder.manufacturer_meshes(source);maximum=0;compared=0
 for mesh in meshes:
  expected={}
  for face in mesh['faces']:
   points=[mesh['vertices'][i] for i in face];x=sum(p[0] for p in points)/3;oid=mesh['objectId']
   name='shell' if oid==2 else ('button-boot' if x < -4.5 else 'button-power' if x > 4.5 else 'button-link') if oid==4 else 'button-ai'
   expected.setdefault(name,[]).extend(points)
  for name,points in expected.items():
   p=parts[name];assert p['vertexCount']==len(points)
   for i,want in enumerate(points):
    got=struct.unpack_from('<fff',data,p['byteOffset']+i*24);maximum=max(maximum,max(abs(a-b) for a,b in zip(want,got)));compared+=1
 assert maximum<.000002
 report.update(rawCadCompared=True,sourceSha256=hashlib.sha256(source).hexdigest(),sourceVertexInstancesCompared=compared,sourceVsFloat32MaxErrorMm=maximum)
(E/'geometry-verification.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))
if not args.photo_alignment:raise SystemExit(0)
import numpy as np
from scipy.optimize import least_squares
from scipy.spatial.transform import Rotation
# Virtual sharp corners extrapolated from straight glass edges, in original PNG pixels.
views={
 'front':dict(width=647,height=688,glass=[[20,117],[352,198],[352,667],[20,507]],shell=[[17,105],[365,191],[365,681],[17,514]]),
 'isometric':dict(width=585,height=642,glass=[[85,140],[495,25],[493,486],[19,575]],shell=[[76,132],[500,13],[505,504],[13,586]])}
world=np.array([[-21.65,21.65,6.25],[21.65,21.65,6.25],[21.65,-21.65,6.25],[-21.65,-21.65,6.25]])
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
 shell=np.array([[-22.4480019,22.4500008,5.54997015],[22.4500008,22.4500008,5.54997015],[22.4500008,-22.4480019,5.54997015],[-22.4480019,-22.4480019,5.54997015]])
 pred=project(q,shell);error=np.linalg.norm(pred-np.array(v['shell']),axis=1)
 result={**v,'focalPx':q[6],'matrixRows':mat.tolist(),'calibration':'Camera fitted only to official 43.3mm cover-glass square. Four manually extrapolated corners +/-5px; straight-edge source image perspective is imperfect. Model geometry never stretched.','calibrationRmsPx':float(np.sqrt(np.mean(best.fun**2))),'shellPredicted':pred.tolist(),'shellCornerErrorPx':error.tolist(),'shellCornerMeanErrorPx':float(error.mean()),'status':'partial; independent photo landmarks against manufacturer-shell outer bounds; not a full photo or whole-product dimension acceptance'};out[name]=result
 pts=lambda x:' '.join(','.join(f'{a:.2f}' for a in p) for p in x)
 svg=f'<svg xmlns="http://www.w3.org/2000/svg" width="{v["width"]}" height="{v["height"]}" viewBox="0 0 {v["width"]} {v["height"]}"><rect width="100%" height="100%" fill="#22292c"/><polyline points="{pts(v["shell"]+[v["shell"][0]])}" fill="none" stroke="#45d5d9" stroke-width="3"/><polyline points="{pts(pred.tolist()+[pred[0].tolist()])}" fill="none" stroke="#ee874e" stroke-width="3"/><polyline points="{pts(v["glass"]+[v["glass"][0]])}" fill="none" stroke="#bbbbbb" stroke-width="1"/><text x="10" y="25" fill="white" font-family="sans-serif" font-size="15">Cyan: source shell landmarks; orange: model projection</text><text x="10" y="46" fill="white" font-family="sans-serif" font-size="14">Mean corner deviation: {error.mean():.1f}px; approximate only</text></svg>'
 (E/f'{name}-outline-overlay.svg').write_text(svg)
json.dump(out,open(E/'alignment.json','w'),indent=2);print(json.dumps({k:{'fit':v['calibrationRmsPx'],'shell':v['shellCornerMeanErrorPx']} for k,v in out.items()}))
