#!/usr/bin/env python3
"""Rebuild the explicitly approximate ESP-Mosaico V1.2 exterior (no network).

The only manufacturer dimensions used are the panel's 43.3mm cover and
38.39mm active square. Housing 45x45x12.5mm and all other coordinates are
photo estimates, NOT factory CAD. See docs/platform/mosaico-model.md.
"""
import argparse, hashlib, json, math, struct
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
def write_model(destination):
 destination.mkdir(parents=True,exist_ok=True)
 metadata={'schemaVersion':'1','id':'espressif-esp-mosaico','unit':'mm','binary':'geometry.bin','strideFloats':6,'parts':[],'derivedParts':[]}
 values=[]
 def tri(a,b,c):
  u=[b[i]-a[i] for i in range(3)];v=[c[i]-a[i] for i in range(3)];n=[u[1]*v[2]-u[2]*v[1],u[2]*v[0]-u[0]*v[2],u[0]*v[1]-u[1]*v[0]];l=math.sqrt(sum(x*x for x in n))
  if l>1e-10:
   for p in [a,b,c]:values.extend([*p,*[x/l for x in n]])
 def quad(a,b,c,d):tri(a,b,c);tri(a,c,d)
 def part(name,color,role,fn,**extra):
  at=len(values);fn();pts=[values[i:i+3] for i in range(at,len(values),6)]
  metadata['parts'].append(dict(name=name,color=color,role=role,byteOffset=at*4,vertexCount=(len(values)-at)//6,boundsMm=[[min(p[i] for p in pts) for i in range(3)],[max(p[i] for p in pts) for i in range(3)]],basis='image-measurement',sourceIds=['isometric','front','back'],**extra))
 def rect(w,h,r):
  out=[]
  for cx,cy,start in [(w/2-r,h/2-r,0),(-w/2+r,h/2-r,90),(-w/2+r,-h/2+r,180),(w/2-r,-h/2+r,270)]:
   for i in range(13):
    t=math.radians(start+i*90/12);out.append([cx+r*math.cos(t),cy+r*math.sin(t)])
  return out
 def plate(w,h,r,z0,z1):
  ps=rect(w,h,r)
  for a,b in zip(ps,ps[1:]+ps[:1]):
   tri([0,0,z1],[*a,z1],[*b,z1]);tri([0,0,z0],[*b,z0],[*a,z0]);quad([*a,z0],[*b,z0],[*b,z1],[*a,z1])
 def box(size,center,rotation=(0,0,0)):
  def point(v):
   x,y,z=v
   for axis,t in enumerate(rotation):
    c=math.cos(t);s=math.sin(t)
    if axis==0:y,z=y*c-z*s,y*s+z*c
    elif axis==1:x,z=x*c+z*s,-x*s+z*c
    else:x,y=x*c-y*s,x*s+y*c
   return [x+center[0],y+center[1],z+center[2]]
  x,y,z=[s/2 for s in size];p=[point(v) for v in [(-x,-y,-z),(x,-y,-z),(x,y,-z),(-x,y,-z),(-x,-y,z),(x,-y,z),(x,y,z),(-x,y,z)]]
  for face in [(0,3,2,1),(4,5,6,7),(0,1,5,4),(1,2,6,5),(2,3,7,6),(3,0,4,7)]:quad(*[p[i] for i in face])
 def shell():
  outer=rect(45,45,2.5);inner=rect(43,43,1.5)
  for i in range(len(outer)):
   j=(i+1)%len(outer);a,b,c,d=outer[i],outer[j],inner[j],inner[i]
   # Bottom middle wall carries the separate USB opening frame.
   if a[1]<-22.49 and b[1]<-22.49:continue
   quad([*a,-6.25],[*b,-6.25],[*b,6.25],[*a,6.25]);quad([*c,-6.25],[*d,-6.25],[*d,6.25],[*c,6.25]);quad([*a,6.25],[*b,6.25],[*c,6.25],[*d,6.25]);quad([*a,-6.25],[*d,-6.25],[*c,-6.25],[*b,-6.25])
  # Front-view BOOT left, POWER right; mouth remains physically open.
  for size,center in [([15.5,1,12.5],[-12.25,-22,0]),([15.5,1,12.5],[12.25,-22,0]),([9,1,4.65],[0,-22,3.925]),([9,1,4.65],[0,-22,-3.925])]:box(size,center)
 part('shell','#e5e5df','shell',shell)
 part('front-cover','#101315','glass',lambda:plate(43.3,43.3,2.2,5.85,6.55))
 part('rear-panel','#222726','panel',lambda:plate(42.4,42.4,1.2,-6.26,-5.85))
 part('button-boot','#deded8','button',lambda:box([7,1,2.4],[-10,-22.55,0]),controlId='boot')
 part('button-power','#deded8','button',lambda:box([7,1,2.4],[10,-22.55,0]))
 part('button-ai','#ef5b22','button',lambda:box([7,1,3.8],[10,22.55,0]),controlId='ai')
 def derived(id,kind,geo,color='#333a39',**extra):metadata['derivedParts'].append(dict(id=id,kind=kind,geometry=geo,color=color,basis='image-measurement',sourceIds=['isometric','front','back'],**extra))
 for id,z in [('pcb-main',2.1),('pcb-base',-2.7)]:derived(id,'pcb-placeholder',dict(sizeMm=[40,40,1],centerMm=[0,0,z],labelSizeMm=[28,7],labelSide='both'),'#53655d',label='UNKNOWN',notes='Estimated board-space placeholder. No trace, component, thickness or board-edge CAD claimed.')
 derived('usb-c','connector-visual',dict(centerMm=[0,-22.56,0],rotation=[math.pi/2,0,0]),connectorType='USB-C',allowConnections=False,connectionStatus='visual-only-unverified',notes='Generic visual mouth, not a measured connector CAD or insertion-depth claim.')
 # Recessed header mouths: individual visible openings, no claimed insertion fit.
 for side in [-1,1]:
  for row in [-1,1]:
   for i in range(10):
    derived(f'header-{side}-{row}-{i}','box',dict(sizeMm=[.12,1.1,1.1],centerMm=[side*22.55,11.43-i*2.54,row*1.27]),'#172728',notes='2.54mm pitch is official; opening width, depth and absolute placement are estimates. No plug connections.')
 for x in [-17,17]:
  for y in [-17,17]:
   derived(f'rear-screw-{x}-{y}','cylinder',dict(centerMm=[x,y,-6.45],radiusMm=1.25,depthMm=.35),'#171d1e',material=dict(metalness=.6,roughness=.4),notes='Four visible dark screw heads; exact fastener standard and hidden shank unknown.')
 for i in range(7):derived(f'rear-pad-{i}','box',dict(sizeMm=[2.1,2.8,.08],centerMm=[(i-3)*2.54,-12,-6.34]),'#c5a65b',material=dict(metalness=.6,roughness=.45),notes='Seven pads visible in V1.2 back photo; exact pitch/size estimated, not connector fit evidence.')
 derived('rear-brand','label',dict(sizeMm=[31,7],centerMm=[0,13,-6.31],rotation=[0,math.pi,0]),'#c5cbc5',texture='rear-brand.svg',notes='Text transcribed from V1.2 back photo; typography approximation, QR code deliberately omitted.')
 derived('rear-pad-labels','label',dict(sizeMm=[23,2.8],centerMm=[0,-15,-6.31],rotation=[0,math.pi,0]),'#d8ddd4',texture='rear-pad-labels.svg',notes='Back-view labels; see official electrical pin table, not this display text, for wiring.')
 for i in range(5):derived(f'top-vent-{i}','box',dict(sizeMm=[1, .1,2.8],centerMm=[-13+i*2.8,22.56,0]),'#3a4140')
 metadata['presentation']=dict(bodySizeMm=[45,45,12.5],screen=dict(sizeMm=[38.39,38.39],positionMm=[0,0,6.56],rotation=[0,0,0]),inspectionGroups=[dict(id='front',parts=['front-cover','live-display'],offsetMm=[0,0,38],sourceIds=['guide-v12','display'],notes='Display lifted only for inspection; offset is a viewer convention, not a mechanical disassembly distance.'),dict(id='case',parts=['shell','button-boot','button-power','button-ai']+[x['id'] for x in metadata['derivedParts'] if x['id'].startswith(('header-','top-vent-'))],offsetMm=[60,-55,0],sourceIds=['front','isometric'],notes='Photo-derived exterior set aside to reveal both UNKNOWN boards; estimated internals do not become verified CAD.'),dict(id='base',parts=['pcb-base','rear-panel']+[x['id'] for x in metadata['derivedParts'] if x['id'].startswith(('rear-screw','rear-pad','rear-brand'))],offsetMm=[0,0,-38],sourceIds=['back','guide-v12'],notes='Approximate rear assembly separated along thickness axis; original positions are restored without accumulation.')])
 metadata['dimensions']=dict(width=45,height=45,depth=12.5,unit='mm',basis='image-measurement',sourceId='isometric',uncertainty='Housing +/-1mm XY and +/-2mm depth; not a manufacturer dimension.')
 metadata['provenance']=dict(method='photo-derived geometry; manufacturer CAD access blocked',sources=[s for s in json.loads((ROOT/'docs/platform/evidence/mosaico-sources/sources.json').read_text()) if s['id'] in ['guide-v12','front','back','isometric','core-front','base-back','display','schematic-core','schematic-base','faq','manufacturer-cad']],generator='tools/build-mosaico-model.py',license='Generated geometry: repository MIT. Manufacturer references and trademarks retain their original rights; no photograph textures included.')
 metadata['limitations']=['Not manufacturer CAD; MakerWorld official source could not be downloaded (HTTP403).','43.3mm cover and 38.39mm active display are independent datasheet values. Housing and control locations are estimates; 1:1 enclosure certification not achieved.','Two UNKNOWN board placeholders preserve stable IDs; internal placement, thickness, wiring, battery and speaker geometry are not certified.','Headers/USB/rear pads are visual only. Hidden fastener shanks, magnetic inserts and internal structure were not invented.','POWER key has no GPIO input hotspot. No photo or generated UI covers the guest framebuffer.']
 binary=struct.pack('<'+'f'*len(values),*values);(destination/'geometry.bin').write_bytes(binary);metadata['artifacts']={'geometry.bin':hashlib.sha256(binary).hexdigest()};
 for name,svg in {
  'rear-brand.svg':'<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="256"><text x="512" y="115" text-anchor="middle" fill="#dddeda" font-family="sans-serif" font-size="104" font-style="italic" font-weight="bold">ESP-MOSAICO</text><text x="512" y="198" text-anchor="middle" fill="#dddeda" font-family="sans-serif" font-size="45">Powered by Espressif</text></svg>',
  'rear-pad-labels.svg':'<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="128"><text x="512" y="78" text-anchor="middle" fill="#dddeda" font-family="monospace" font-size="48">GND BOOT RST RX TX 5V GND</text></svg>'
 }.items():
  (destination/name).write_text(svg+'\n');metadata['artifacts'][name]=hashlib.sha256((destination/name).read_bytes()).hexdigest()
 (destination/'model.json').write_text(json.dumps(metadata,ensure_ascii=False,indent=2)+'\n');print(json.dumps({'vertices':len(values)//6,'bytes':len(binary),'geometrySha256':metadata['artifacts']['geometry.bin'],'modelSha256':hashlib.sha256((destination/'model.json').read_bytes()).hexdigest()}))
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--output',type=Path,default=ROOT/'web/platform/assets/espressif-esp-mosaico');write_model(p.parse_args().output)
