"""Integrate exact polygon intersections of original CAD sections; no mesh deformation.
Requires model dependencies plus shapely==2.2.0.
usage: python tools/verify-stackchan-rgb-sections.py ASSET_DIRECTORY [--output receipt.json]
Baseline uses recorded rigid transforms from model-plan.json before RGB fit revision.
The section area is exact up to STL precision; volume remains numerical Z integration.
"""
import argparse,hashlib,json,math
from pathlib import Path
import numpy as np,trimesh,shapely
from shapely.geometry import Polygon,GeometryCollection,box
from shapely.affinity import translate
p=argparse.ArgumentParser();p.add_argument('assets');p.add_argument('--output',default='/tmp/stackchan-rgb-fit-exact.json');p.add_argument('--dz',type=float,default=.1);a=p.parse_args();root=Path(a.assets)
M=np.array([[1,0,0,-475.73],[0,0,1,-9.3],[0,-1,0,-328.66],[0,0,0,1]])
c=math.sqrt(.5)
locals={'shell':np.eye(4),'left':np.array([[c,0,c,449.95448897060345],[0,1,0,-332.2132],[-c,0,c,35.075511029396544],[0,0,0,1]]),'right':np.array([[-c,0,-c,501.5055110293966],[0,-1,0,-332.2132],[-c,0,c,35.075511029396544],[0,0,0,1]])}
files={'shell':'source.stl','left':'source-manufacturer-cad-fbd57f39a7.stl','right':'source-manufacturer-cad-c71853d138.stl'}
meshes={};sources={}
for name,fn in files.items():
 f=root/fn;m=trimesh.load(f,process=True);m.apply_transform(M@locals[name]);meshes[name]=m
 sources[name]={'file':fn,'sha256':hashlib.sha256(f.read_bytes()).hexdigest(),'worldMatrixRows':(M@locals[name]).tolist(),'watertight':bool(m.is_watertight)}
invalid=[]
def section(m,z):
 s=m.section([0,0,1],[0,0,z]);r=GeometryCollection()
 if s:
  for line in s.discrete:
   q=Polygon(line[:,:2])
   if not q.is_valid:
    invalid.append(z);q=shapely.make_valid(q)
   r=r.symmetric_difference(q)
 return r
zs=np.arange(-11.9468+a.dz/2,19.0532,a.dz)
slices=[]
for z in zs:slices.append((float(z),*[section(meshes[n],z)for n in ['shell','left','right']]))
measurements=[]
for side,index,sign in [('left',2,1),('right',3,-1)]:
 for t in [0,.05,.1,.125,.15,.175,.2,.225,.25]:
  area=[];bad=[]
  for row in slices:
   g=translate(row[index],xoff=sign*t,yoff=-t);i=row[1].intersection(g);area.append(i.area)
   if i.area>1e-8:bad.append({'z':round(row[0],6),'areaMm2':round(i.area,8),'intersectionBoundsMm':list(i.bounds)})
  measurements.append({'side':side,'translationMm':[sign*t,-t,0],'intersectionMm3':round(sum(area)*a.dz,8),'maximumSectionIntersectionMm2':round(max(area),8),'intersectingSections':len(bad),'worstSections':sorted(bad,key=lambda k:-k['areaMm2'])[:3]})
# Nominal rectangular packages before and after proposed relocation; exact XY polygons.
leds=[]
for side,sign,index in [('left',-1,2),('right',1,3)]:
 for i in range(6):
  r={'side':side,'index':i}
  for key,center,guide_t in [('old',[sign*22.4,23.85,-6.9468+5*i],0),('candidate',[sign*19.4,23.9,-8.9468+5*i],.175),('finalProposed',[sign*19.2,23.9,-8.9468+5*i],.15)]:
   b=box(center[0]-2,center[1]-.4,center[0]+2,center[1]+.4);v=[0.,0.]
   for z in np.arange(center[2]-1+a.dz/2,center[2]+1,a.dz):
    shell=section(meshes['shell'],z);guide=translate(section(meshes[side],z),xoff=-sign*guide_t,yoff=-guide_t)
    v[0]+=b.intersection(shell).area*a.dz;v[1]+=b.intersection(guide).area*a.dz
   r[key]={'centerMm':center,'shellIntersectionMm3':round(v[0],8),'guideIntersectionMm3':round(v[1],8),'guideInwardTranslationMm':guide_t}
  leds.append(r)
out={'sources':sources,'method':{'shapelyVersion':shapely.__version__,'zStepMm':a.dz,'invalidSectionRepairs':len(invalid),'caveat':'Section areas are polygon intersections of STL faces. Integrated volume is sampled and not a CAD Boolean or calibrated fit guarantee. Candidate package positions are inferred from nominal PCB and CAD cavity, not manufacturer PCB layout.'},'guideShellMeasurements':measurements,'ledMeasurements':leds}
Path(a.output).write_text(json.dumps(out,indent=2)+'\n')
print(json.dumps({'output':a.output,'guideShellMeasurements':[{k:v for k,v in r.items()if k!='worstSections'}for r in measurements],'ledMeasurements':leds,'invalidSectionRepairs':len(invalid)},indent=2))
