#!/usr/bin/env python3
"""Partition K151 original light-guide optical surfaces, with no offset geometry.
Run --write to update the source-backed plan, then the generic model rebuilder.
"""
import argparse,copy,hashlib,json
from pathlib import Path
import numpy as np,trimesh
ROOT=Path(__file__).resolve().parents[1];ASSETS=ROOT/'web/platform/assets/m5stack-k151'
SOURCES={'left':('fbd57f39a7','05313220a4c3734dd0c412a9de04349e4f9b55024ff3a382ea0d85c1d297d2a3'),'right':('c71853d138','ba640a41fde8b197a20579814fd8ca94eba44e7b4cbcae3096d6a1e556dd7db2')}
def ranges(indices):
 result=[]
 for i in indices:
  i=int(i)
  if result and result[-1][1]==i:result[-1][1]=i+1
  else:result.append([i,i+1])
 return result

def prepare(plan):
 result=copy.deepcopy(plan);result['parts']=[p for p in result['parts']if not p['name'].startswith('rgb-window-')];result['derivedParts']=[p for p in result['derivedParts']if not p['id'].startswith('rgb-window-')];report={}
 for side,(suffix,expected)in SOURCES.items():
  path=ASSETS/f'source-manufacturer-cad-{suffix}.stl';raw=path.read_bytes();actual=hashlib.sha256(raw).hexdigest()
  if actual!=expected:raise ValueError(f'{side} source hash changed: {actual}')
  mesh=trimesh.load(path).split(only_watertight=False)[0]
  # The front crown is the source CAD's outermost curved patch. Its adjacent
  # mounting faces start beyond nativeX1.5mm; no visibility ray selects geometry.
  selected=(mesh.face_normals[:,0]<-.5)&(mesh.triangles[:,:,0].max(axis=1)<.6)
  if selected.sum()!=99 or abs(mesh.area_faces[selected].sum()-59.841491690158996)>1e-8:raise ValueError('Original optical crown changed')
  body=next(p for p in result['parts']if p['name']==f'light-guide-{side}');body['sourceFaceRanges']=ranges(np.where(~selected)[0]);body['matrixRows'][0][3]=450.10448897060345 if side=='left'else 501.3555110293966;body['matrixRows'][2][3]=34.925511029396544;body['notes']='Original manufacturer light-guide faces excluding the99-face optical crown, partitioned below without caps, surface offset, scaling or lost area. Entire guide assembly moved0.212132mm inward along45degree normal: left(+0.15,-0.15,0),right(-0.15,-0.15,0). Source outer-corner/crown radius5mm fits put optical face about0.05mm inside the shell envelope; mounting fit remains estimated with retained section residuals.'
  for index in range(6):
   native_index=5-index if side=='left'else index;lo=-15.5+native_index*31/6;hi=-15.5+(native_index+1)*31/6
   item={key:copy.deepcopy(body[key])for key in ['sourceId','sourceComponent','matrixRows']};item.update(name=f'rgb-window-{side}-{index}',role='component',color='#bdceca',ledIndex=index+(0 if side=='left'else 6),sourceFaceRanges=ranges(np.where(selected)[0]),sourcePlaneClip={'axis':1,'minMm':lo,'maxMm':hi},notes='Six equal length visual zones partition the actual original CAD optical crown, including its curved section and rounded ends. Linear triangle clipping only: no overlay, cap, surface offset or added volume. LED chain-to-space order and optical diffusion remain uncalibrated.')
   result['parts'].append(item)
  for item in result['derivedParts']:
   if item['id'].startswith(f'led-package-{side}-'):
    index=int(item['id'].rsplit('-',1)[1]);item['geometry']['centerMm']=[-19.2 if side=='left'else 19.2,23.9,-8.9468+index*5];item['notes']='WS2812C_4020 package-class envelope from official schematic; placement is an explicit board-space estimate.4×0.8×2mm body stays within the43×33mm UNKNOWN touch-board outline; underside rests at board-topY23.5. The UNKNOWN board is itself an estimated envelope; fitting inside it does not establish the real PCB layout. Physical chain order, PCB footprint and optical coupling are not verified. No duplicated ledIndex.'
  report[side]={'sourceSha256':actual,'sourceTriangleCount':len(mesh.faces),'opticalTriangleCount':int(selected.sum()),'opticalAreaMm2':float(mesh.area_faces[selected].sum()),'opticalNativeBoundsMm':[mesh.triangles[selected].reshape(-1,3).min(0).tolist(),mesh.triangles[selected].reshape(-1,3).max(0).tolist()],'bodyTriangleCount':int((~selected).sum()),'sourceFaceCoverage':'body and optical sourceFaceRanges are disjoint and union to every original source triangle'}
 return result,report

def main():
 p=argparse.ArgumentParser();p.add_argument('--write',action='store_true');a=p.parse_args();plan=json.loads((ASSETS/'model-plan.json').read_text());result,report=prepare(plan)
 if a.write:(ASSETS/'model-plan.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n')
 elif result!=plan:raise SystemExit('RGB surface plan differs; run --write to regenerate')
 print(json.dumps(report,indent=2))
if __name__=='__main__':main()
