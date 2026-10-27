#!/usr/bin/env python3
"""Check K152 antenna scale and mounting against the preserved manufacturer seat."""
import argparse, datetime, hashlib, json
from pathlib import Path
import numpy as np

p=argparse.ArgumentParser();p.add_argument('--output',type=Path);args=p.parse_args()
root=Path(__file__).resolve().parents[1];assets=root/'web/platform/assets/m5stack-k152'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
metadata=json.loads((assets/'model.json').read_text());part=next(p for p in metadata['parts'] if p['name']=='cap-back')
raw=(assets/'geometry.bin').read_bytes();vertices=np.frombuffer(raw,dtype='<f4',offset=part['byteOffset'],count=part['vertexCount']*6).reshape(-1,6)[:,:3]
vertices=np.unique(vertices,axis=0);plane=vertices[(abs(vertices[:,1]-36.039993)<.0001)&(vertices[:,0]>26)]
# Select the 7mm outer seat arc on the top plane, then solve its independent center.
plane=plane[abs(np.linalg.norm(plane[:,[0,2]]-[33.96,-17.15],axis=1)-7)<.001]
xz=plane[:,[0,2]];x,z,c=np.linalg.lstsq(np.c_[2*xz,np.ones(len(xz))],(xz*xz).sum(axis=1),rcond=None)[0]
radius=float(np.sqrt(x*x+z*z+c));center=np.array([x,36.039993,z])
lathe=[p for p in metadata['derivedParts'] if p['id'].startswith('antenna-') and p['kind']=='lathe']
points=np.concatenate([np.array(p['geometry']['profileMm']) for p in lathe]);length=float(points[:,1].max()-points[:,1].min());diameter=float(2*points[:,0].max())
checks=[{'name':'published antenna total length including collar','measuredMm':length,'targetMm':108,'pass':abs(length-108)<.00001},
 {'name':'published maximum antenna diameter','measuredMm':diameter,'targetMm':9.3,'pass':abs(diameter-9.3)<.00001},
 {'name':'axis intersects CAD mounting-seat center','measuredMm':max(float(np.linalg.norm(np.array(p['geometry']['centerMm'])-center))for p in lathe),'toleranceMm':.0001},
 {'name':'all antenna segments use outward +Y axis','pass':all(p['geometry']['rotation']==[0,0,0]for p in lathe)},
 {'name':'manufacturer geometry unchanged','pass':sha(assets/'geometry.bin')=='d2f0e0d0a6222cc4d03da7ca3959cee4d3295d482c792bc57c64b3dec326765e'}]
checks[2]['pass']=checks[2]['measuredMm']<=checks[2]['toleranceMm']
report={'checkedAt':datetime.datetime.now(datetime.timezone.utc).isoformat().replace('+00:00','Z'),'modelSha256':sha(assets/'model.json'),'geometrySha256':sha(assets/'geometry.bin'),'sourceStlSha256':sha(assets/'source.stl'),'rendererSha256':sha(root/'web/platform/manufacturer-model.js'),'scriptSha256':sha(Path(__file__)),
 'cadSeat':{'arcVertices':len(xz),'centerMm':center.tolist(),'radiusMm':radius,'maxResidualMm':float(abs(np.linalg.norm(xz-[x,z],axis=1)-radius).max())},'checks':checks,
 'limitations':['108mm is the full collar-to-tip extent, not108mm added beyond a separate collar.','Profile segmentation and seam details are photo-derived estimates, not antenna manufacturer CAD.','Source image07 fades the upper antenna and its apparent horizontal position differs from the CAD seat; the original source case is preserved.','No internal RF contact, articulation, or physical caliper measurement verified.']}
if args.output:args.output.parent.mkdir(parents=True,exist_ok=True);args.output.write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps(report,indent=2));assert all(c['pass'] for c in checks)
