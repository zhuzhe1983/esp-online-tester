"""Sample nominal SCS0009 envelopes against K151 manufacturer plastic CAD.
Not a solid collision certificate. Pass the public asset directory; no resize.
SCS0009 dimensions: official SCS0009.pdf page6/8, A/0 (2020-11-23).
This reports conservative rectangular gear/ear overlaps, rather than deleting them.
Requires numpy, trimesh, opencv-python. Grid spacing0.5mm, interioroffset0.15mm.
"""
import argparse,json,hashlib,numpy as np,trimesh,cv2
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('assets');p.add_argument('--output',default='/tmp/stackchan-internal-fit-review.json');a=p.parse_args();root=Path(a.assets)
ids={'body':'52cb7ac41d','cover':'6176ffdaa9','side':'8ed6ee0116'};meshes={};hashes={}
for name,h in ids.items():
 f=root/f'source-manufacturer-cad-{h}.stl';hashes[f.name]=hashlib.sha256(f.read_bytes()).hexdigest();m=trimesh.load(f,process=True);v=m.vertices.copy()
 m.vertices=np.column_stack([v[:,0]+246.25-475.73,v[:,2]-9.3,-v[:,1]-328.66+12.75]);
 if name=='cover':m.apply_translation([0,-10,0])
 meshes[name]=m
axis=4.6532
parts={
 'pan-core':([-5.95,-27.8,axis-6.05],[17.35,-6.35,axis+6.05]),
 'pan-raised-gear-envelope':([-5.95,-31.6,axis-6.05],[9.35,-27.8,axis+6.05]),
 'pan-ear-envelope':([-10.55,-24.75,axis-5.5],[21.95,-23.15,axis+5.5]),
 'tilt-core':([-6,-6.05,axis-17.35],[15.45,6.05,axis+5.95]),
 'tilt-raised-gear-envelope':([15.45,-6.05,axis-9.35],[19.25,6.05,axis+5.95]),
 'tilt-ear-envelope':([10.8,-5.5,axis-21.95],[12.4,5.5,axis+10.55])}
result={'method':'0.5mm sampled interior grid, +0.15mm starting offset, section polygon even-odd test; not exact boolean collision verification','sourceSha256':hashes,'standTranslationMm':[0,0,12.75],'coverAdditionalTranslationMm':[0,-10,0],'parts':{},'limitations':['Gear envelope corners and screw ear profiles are conservative boxes, not complete motor CAD.','No voxel collision count establishes mechanical assembly accuracy.','Neither bracket screw clearance nor hard-stop calibration has been measured.','Tilt ear/chassis interference remains unresolved; preserve the residual and label placement estimated.']}
for pn,(lo,hi) in parts.items():
 collisions={name:[] for name in meshes};xs=np.arange(lo[0]+.15,hi[0],.5);zs=np.arange(lo[2]+.15,hi[2],.5)
 for y in np.arange(lo[1]+.15,hi[1],.5):
  grid=np.array(np.meshgrid(xs,zs)).reshape(2,-1).T
  for name,m in meshes.items():
   sec=m.section([0,1,0],[0,y,0]);inside=np.zeros(len(grid),bool)
   if sec:
    for loop in sec.discrete:
     poly=np.float32(loop[:,[0,2]]);inside^=np.array([cv2.pointPolygonTest(poly,tuple(q),False)>0 for q in grid])
   collisions[name].extend([[x,y,z] for x,z in grid[inside]])
 row={'nominalBoundsMm':[lo,hi],'intersections':{}}
 for name,pts in collisions.items():
  points=np.array(pts);row['intersections'][name]={'sampleCount':len(pts),'sampledVolumeMm3':len(pts)*.125,'sampleBoundsMm':np.round([points.min(0),points.max(0)],5).tolist() if len(points) else None}
 result['parts'][pn]=row
Path(a.output).write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
