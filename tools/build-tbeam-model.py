#!/usr/bin/env python3
"""Rebuild the explicitly partial T-Beam V1.2 photo proxy (standard library only)."""
import argparse,json,struct,hashlib
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
SRC=ROOT/'web/platform/assets/lilygo-t-beam-v1-2'
def digest(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def build(out):
 plan=json.loads((SRC/'model-plan.json').read_text());out.mkdir(parents=True,exist_ok=True);data=bytearray();parts=[]
 for source in plan['provenance']['sources']:
  if 'file' in source:assert digest(SRC/source['file'])==source['sha256'],'Source photo changed'
 for p in plan['boxes']:
  center=p['centerMm'];size=p['sizeMm'];lo=[center[i]-size[i]/2 for i in range(3)];hi=[center[i]+size[i]/2 for i in range(3)];offset=len(data)
  for axis in range(3):
   u=(axis+1)%3;v=(axis+2)%3
   for sign in [-1,1]:
    normal=[0,0,0];normal[axis]=sign
    vertices=[]
    for a,b in [(0,0),(1,0),(1,1),(0,1)]:
     xyz=center.copy();xyz[axis]=hi[axis] if sign>0 else lo[axis];xyz[u]=hi[u] if a else lo[u];xyz[v]=hi[v] if b else lo[v];vertices.append(xyz)
    for triangle in ([(0,1,2),(0,2,3)] if sign>0 else [(0,2,1),(0,3,2)]):
     for i in triangle:data.extend(struct.pack('<6f',*vertices[i],*normal))
  parts.append({k:v for k,v in p.items() if k not in ['centerMm','sizeMm']}|dict(byteOffset=offset,vertexCount=36,boundsMm=[lo,hi]))
 (out/'geometry.bin').write_bytes(data)
 model={k:v for k,v in plan.items() if k!='boxes'}|dict(unit='mm',strideFloats=6,binary='geometry.bin',parts=parts,artifacts={'geometry.bin':hashlib.sha256(data).hexdigest(),**{s['file']:s['sha256'] for s in plan['provenance']['sources'] if 'file' in s}})
 (out/'model.json').write_text(json.dumps(model,ensure_ascii=False,indent=2)+'\n')
 print(json.dumps(dict(modelSha256=digest(out/'model.json'),geometrySha256=digest(out/'geometry.bin'),bytes=len(data),parts=len(parts))))
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--output',type=Path,required=True);build(p.parse_args().output)
