#!/usr/bin/env python3
"""Write K152 native source-face partitions; then use public model rebuilding.

Unlike indices in an old exported binary, native source-face indices address the
actual pinned STL component loaded by the exporter. No key geometry is invented.
"""
import hashlib,json
from pathlib import Path
import numpy as np
import trimesh
ROOT=Path(__file__).resolve().parents[1]
ASSETS=ROOT/'web/platform/assets/m5stack-k152'
def ranges(indices):
    out=[]
    for i in sorted(map(int,indices)):
        if out and out[-1][1]==i:out[-1][1]+=1
        else:out.append([i,i+1])
    return out
def main():
    plan=json.loads((ASSETS/'model-plan.json').read_text());metadata=json.loads((ASSETS/'model.json').read_text())
    source=(ASSETS/'source.stl').read_bytes();assert hashlib.sha256(source).hexdigest()==metadata['provenance']['sourceSha256']
    mesh=trimesh.load(ASSETS/'source.stl').split(only_watertight=False)[10]
    matrix=np.array(plan['matrixRows']);xyz=(mesh.vertices@matrix[:3,:3].T+matrix[:3,3])[mesh.faces]
    assert len(xyz)==39180
    selected=xyz[:,:,2].max(1)>6.5
    edges=mesh.face_adjacency[np.all(selected[mesh.face_adjacency],axis=1)]
    groups=trimesh.graph.connected_components(edges,nodes=np.flatnonzero(selected),min_len=1)
    assert len(groups)==56 and all(len(g)==250 for g in groups)
    caps=[]
    for group in groups:
        v=xyz[group].reshape(-1,3);bounds=np.array([v.min(0),v.max(0)])
        caps.append(dict(faces=np.sort(group),bounds=bounds,center=bounds.mean(0)))
    rows=sorted(set(round(float(p['center'][1]),4)for p in caps),reverse=True);assert len(rows)==4
    carrier=next(p for p in plan['parts']if p['name']=='keyboard-carrier')
    carrier['sourceFaceRanges']=ranges(np.flatnonzero(~selected));carrier['role']='internal'
    plan['parts']=[p for p in plan['parts']if not p.get('controlId','').startswith('key-r')]
    coverage=np.zeros(len(xyz),dtype=int);coverage[~selected]+=1
    for row,y in enumerate(rows):
        items=sorted([p for p in caps if abs(p['center'][1]-y)<.001],key=lambda p:p['center'][0]);assert len(items)==14
        for column,cap in enumerate(items):
            name=f'key-r{row}-c{column}';coverage[cap['faces']]+=1
            plan['parts'].append(dict(sourceComponent=10,name=name,role='button',color=carrier['color'],controlId=name,sourceFaceRanges=ranges(cap['faces'])))
    assert np.all(coverage==1)
    (ASSETS/'model-plan.json').write_text(json.dumps(plan,ensure_ascii=False,indent=2)+'\n')
    print('Native source partitions written:56 caps x250 faces +25180 carrier faces. Regenerate with tools/rebuild-product-model.py in a fresh scratch directory.')
if __name__=='__main__':main()
