#!/usr/bin/env python3
"""Check the StampFly underside against retained source geometry, not a screenshot.

Uses only Python stdlib. Browser/photo evidence remains a separate required check.
"""
import hashlib,json,math,struct
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
def read(path):return json.loads(path.read_text())
def near(a,b,tol=2e-6):assert abs(a-b)<=tol,(a,b)
def clip(poly,axis,value,sign):
 result=[]
 for a,b in zip(poly,poly[1:]+poly[:1]):
  ia=sign*(a[axis]-value)>=0;ib=sign*(b[axis]-value)>=0
  if ia:result.append(a)
  if ia!=ib:
   t=(value-a[axis])/(b[axis]-a[axis]);result.append([a[k]+t*(b[k]-a[k])for k in range(2)])
 return result

def verify(sku):
 assets=ROOT/'web/platform/assets'/sku;plan=read(assets/'model-plan.json');meta=read(assets/'model.json');derived={p['id']:p for p in plan['derivedParts']};surfaces={p['name']:p for p in plan['surfaces']};board=derived['optical-flow-pcb'if sku.endswith('v11')else'pcb-unknown-marker']['geometry'];center=board['centerMm'];size=board['sizeMm'];top=center[2]+size[2]/2;bottom=center[2]-size[2]/2
 raw=(assets/'source-frame.stl').read_bytes();source=next(s for s in meta['provenance']['sources']if s['id']=='ecosystem-frame-stl');assert hashlib.sha256(raw).hexdigest()==source['sha256'];count=struct.unpack_from('<I',raw,80)[0];contact=0;faces=0
 for i in range(count):
  values=struct.unpack_from('<12f',raw,84+i*50);vs=[[-values[j],values[j+2],values[j+1]]for j in [3,6,9]]
  # Native broad cradle bottom faces are inside this central volume.
  if max(abs(v[2]-top)for v in vs)>2e-6 or max(abs(v[0])for v in vs)>15 or max(abs(v[1])for v in vs)>20:continue
  poly=[v[:2]for v in vs]
  for axis in [0,1]:
   poly=clip(poly,axis,center[axis]-size[axis]/2,1);poly=clip(poly,axis,center[axis]+size[axis]/2,-1)
  if len(poly)>2:
   area=abs(sum(a[0]*b[1]-b[0]*a[1]for a,b in zip(poly,poly[1:]+poly[:1])))/2;contact+=area;faces+=1
 assert faces==4 and contact>=59.99,(faces,contact)
 pcb=surfaces['optical-flow-pcb-photo'];near(pcb['positionMm'][2],bottom-.01);assert pcb['sizeMm']==size[:2]
 for name,body in [('optical-flow-package-photo','optical-flow-package'),('optical-flow-socket-photo','optical-flow-socket-roof')]:
  face=surfaces[name];g=derived[body]['geometry'];near(face['positionMm'][2],g['centerMm'][2]-g['sizeMm'][2]/2-.01)
  for axis in [0,1]:assert abs(face['positionMm'][axis]-g['centerMm'][axis])+face['sizeMm'][axis]/2<=g['sizeMm'][axis]/2+1e-8
 lens=derived['optical-flow-lens']['geometry'];face=surfaces['optical-flow-lens-photo'];near(face['positionMm'][2],lens['centerMm'][2]-lens['depthMm']/2-.01)
 assert face.get('outlineMm'),'A transparent texture must not conceal an unsupported square lens carrier'
 for x,y in face['outlineMm']:assert math.hypot(x-lens['centerMm'][0],y-lens['centerMm'][1])<=lens['radiusMm']+1e-8
 near(bottom-(lens['centerMm'][2]-lens['depthMm']/2),2.28)
 tex=next(t for t in plan['textureSources']if t['file']=='optical-flow-back.webp');assert len(tex['alphaMask']['excludeRects'])==2
 assert board['labelSide']=='front';group=plan['presentation']['inspectionGroups'][0];required={p for p in derived if p.startswith('optical-flow-')}|{p for p in surfaces if p.startswith('optical-flow-')};required.add('optical-flow-pcb'if sku.endswith('v11')else'pcb-unknown-marker');assert required<=set(group['parts'])
 for name in meta['artifacts']:assert hashlib.sha256((assets/name).read_bytes()).hexdigest()==meta['artifacts'][name],name
 battery=next((p for p in meta['parts']if p['name']=='battery'),None);gap=None
 if battery:gap=battery['boundsMm'][0][2]-top;assert gap>=.74
 assert meta['sourceTransform']['scale']==1
 assert plan==read(ROOT/'docs/platform/evidence'/sku/'source-audit.json')['plan']
 return {'productId':sku,'sourceFrameSha256':source['sha256'],'cradleContactFaces':faces,'cradleContactAreaMm2':contact,'boardTopZMm':top,'boardPhotoOffsetMm':.01,'packageTotalProtrusionMm':2.28,'nativeBatteryToBoardGapMm':gap,'inspectionParts':len(group['parts']),'passed':True}
if __name__=='__main__':print(json.dumps([verify(sku)for sku in ['m5stack-k138','m5stack-k138-v11']],indent=2))
