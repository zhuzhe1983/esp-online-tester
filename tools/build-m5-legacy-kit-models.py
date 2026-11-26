#!/usr/bin/env python3
"""Rebuild five old SKU previews from their committed plans and fixed inputs.
No network. Related K001 CAD is explicitly a family-top reference, not exact SKU CAD.
"""
import argparse,gzip,hashlib,importlib.util,json,math,shutil,struct
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
def digest(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def build(id,output,source_root=ROOT,plan_path=None):
 source_root=Path(source_root).resolve();source=source_root/'web/platform/assets'/id;plan_path=Path(plan_path) if plan_path else source/'model-plan.json';plan=json.loads(plan_path.read_text());output.mkdir(parents=True,exist_ok=True)
 spec=importlib.util.spec_from_file_location('claw_geometry',source_root/'tools/build-cloudzao-claw154-model.py');g=importlib.util.module_from_spec(spec);spec.loader.exec_module(g)
 for f,h in plan['inputs'].items():
  if digest(source/f)!=h:raise ValueError('Changed fixed input '+f)
 records=[];blobs=[]
 def part(name,data,**props):
  blob=struct.pack('<'+'f'*len(data),*data);points=[data[i:i+3] for i in range(0,len(data),6)]
  records.append(dict(name=name,byteOffset=sum(map(len,blobs)),vertexCount=len(data)//6,boundsMm=[[min(p[i] for p in points) for i in range(3)],[max(p[i] for p in points) for i in range(3)]],**props));blobs.append(blob)
 w,h,d=plan['bodyMm'];derived=list(plan.get('derivedParts',[]));present=dict(plan['presentation']);photo=['photo-1','photo-3'] if plan['kind']=='photo' else ['photo-1','official-docs']
 if plan['kind']=='core-family':
  base=source_root/'web/platform/assets/m5stack-k001';snapshot=plan['familyCad'].get('metadataSnapshot')
  if snapshot:
   packed=(plan_path.parent/snapshot['file']).read_bytes()
   if len(packed)!=snapshot['bytes'] or hashlib.sha256(packed).hexdigest()!=snapshot['sha256']:raise ValueError('Changed shared CAD metadata snapshot')
   metadata=gzip.decompress(packed)
   if len(metadata)!=snapshot['uncompressedBytes'] or hashlib.sha256(metadata).hexdigest()!=snapshot['uncompressedSha256'] or snapshot['uncompressedSha256']!=plan['familyCad']['inputs']['model.json']:raise ValueError('Changed original shared CAD metadata')
  else:metadata=(base/'model.json').read_bytes()
  config=json.loads(metadata)
  for f,sha in plan['familyCad']['inputs'].items():
   if f=='model.json' and snapshot:continue
   if digest(base/f)!=sha:raise ValueError('Changed shared CAD input '+f)
  data=(base/'geometry.bin').read_bytes();shift=d/2-5.632286529541016
  for p in config['parts']:
   values=list(struct.unpack('<'+'f'*(p['vertexCount']*6),data[p['byteOffset']:p['byteOffset']+p['vertexCount']*24]))
   for i in range(2,len(values),6):values[i]+=shift
   props={k:p[k] for k in ['role','color'] if k in p};props.update(basis='related manufacturer K001 top CAD; exact old-SKU assembly unknown',sourceIds=['family-cad'])
   if p['name']=='main-frame':props['color']=plan['shellColor']
   part(p['name'],values,**props)
  body_bottom=-d/2+(7.6 if id=='m5stack-k007' else 0);frame_bottom=-8.445000190734863+shift
  part('source-photo-base-wall',g.ring(g.rounded(54,54,3),g.rounded(52,52,2),body_bottom,frame_bottom),role='shell',color='#d5d8d2',sourceIds=photo,basis='photo and official envelope approximation; hidden latch and port apertures uncalibrated')
  part('source-photo-base-bottom',g.solid(54,54,.7,3,[0,0,body_bottom+.35]),role='shell',color='#bfc6c0',sourceIds=photo,basis='plain underside approximation; LEGO hole pattern and magnets not reconstructed')
  if id=='m5stack-k007':part('charging-base',g.ring(g.rounded(54,54,3),g.rounded(52,52,2),-d/2,body_bottom),role='shell',color='#b3b8b5',sourceIds=photo,basis='separate charging tray approximation; 7.6 mm estimated from total 28.6 vs 21 mm host envelope, contacts and hidden attachment unknown')
  present['screen']={**config['presentation']['screen'],'positionMm':[0,-.25,5.17+shift]}
  present['buttons']=[{**b,'positionMm':[b['positionMm'][0],b['positionMm'][1],b['positionMm'][2]+shift],'sourceIds':['family-cad']} for b in config['presentation']['buttons']]
  for b in config['derivedParts']:
   if b['id'] in ['usb-c','grove']:
    item=json.loads(json.dumps(b));item['geometry']['centerMm'][2]+=shift;item['sourceIds']=['family-cad','photo-1'];item['allowConnections']=False;derived.append(item)
  pcb_z=(frame_bottom+body_bottom)/2
  derived.append(dict(id='pcb-main',kind='pcb-placeholder',color='#7e8983',geometry=dict(sizeMm=[44,44,1],centerMm=[0,0,pcb_z],labelSizeMm=[32,8],labelSide='both'),sourceIds=['official-docs'],basis='unknown internal layout; free-space estimate only'))
  top=[p['name'] for p in records if p['name'] not in ['source-photo-base-wall','source-photo-base-bottom','charging-base']]+['live-display']+['cover-key:'+x for x in ['button-a','button-b','button-c']]
  low=['source-photo-base-wall','source-photo-base-bottom']+(['charging-base'] if id=='m5stack-k007' else [])
  present['inspectionGroups']=[dict(id='related-top',parts=top,offsetMm=[0,0,30],sourceIds=['family-cad'],notes='Related K001 top geometry stays rigid; inspection spacing only, not verified old-SKU disassembly.'),dict(id='estimated-base',parts=low,offsetMm=[0,0,-25],sourceIds=['photo-1'],notes='Estimated old base and optional charging tray, shown below the fixed UNKNOWN board.')]
 else:
  # Independent photo envelopes preserve the old display/controller identity.
  screen=present['screen'];sw,sh=screen['sizeMm'];sx,sy,_=screen['positionMm'];r=2.6 if id.endswith('k016-b') else 2.4
  part('shell-wall',g.ring(g.rounded(w,h,r),g.rounded(w-2,h-2,r-1),-d/2+.6,d/2-.6),role='shell',color=plan['shellColor'],sourceIds=['photo-3'],basis='photo-derived shell wall; ports are nominal visual openings only')
  part('rear-cover',g.solid(w,h,.6,r,[0,0,-d/2+.3]),role='shell',color=plan['shellColor'],sourceIds=['photo-4' if id.endswith('k016-t') else 'photo-2'],basis='rear outline approximation; hidden structure and decal geometry unknown')
  if plan.get('frontRecessContours'):
   spec=importlib.util.spec_from_file_location('legacy_recess_geometry',source_root/'tools/build-waveshare-models.py');cad=importlib.util.module_from_spec(spec);spec.loader.exec_module(cad)
   shape={'shape':'plate','outlineMm':g.rounded(w,h,r),'holesMm':[g.rounded(sw+.5,sh+.5,.6,sx,sy)],'depthMm':.6,'centerZMm':d/2-.3,'subtract':[{'shape':'plate','outlineMm':outline,'depthMm':.2,'centerZMm':d/2} for outline in plan['frontRecessContours']]}
   values=cad.tessellate(cad.procedural(shape),.04,.15).reshape(-1).tolist()
  else:values=g.ring(g.rounded(w,h,r),g.rounded(sw+.5,sh+.5,.6,sx,sy),d/2-.6,d/2)
  part('front-bezel',values,role='shell',color=plan['shellColor'],sourceIds=['photo-3'],basis=('front photo and independent screen nominal dimensions; optional source-traced M5 recess depth0.1mm estimated; no baked firmware picture' if plan.get('frontRecessContours') else 'front photo and independent screen nominal dimensions; no baked firmware picture'))
  if id.endswith('k016-b'):
   part('side-button',g.solid(1.2,4,2.4,.4,[w/2+.45,8.2,0]),role='button',color='#343536',controlId='button',sourceIds=['photo-3'],basis='visible right-side actuator; position/height approximately +/-1 mm')
   front=['front-bezel','live-display'];rear=['rear-cover']+[x['id']for x in derived if x.get('kind')=='label']
  else:
   part('front-button',g.solid(11,10,1.2,1.6,[0,-15,d/2-.15]),role='button',color='#292a2c',controlId='button-a',sourceIds=['photo-3'],basis='visible face actuator; external size estimated from exact SKU photo')
   # Side camera lobe and mounting ear are kept independent from the main body.
   part('camera-lobe',g.solid(7,18,d-2,2,[-w/2-2.5,9,0]),role='shell',color=plan['shellColor'],sourceIds=['photo-3','photo-4'],basis='outline approximation; hidden depth unknown')
   derived.extend([dict(id='camera-rim',kind='cylinder',color='#1b1d20',geometry=dict(radiusMm=2.8,depthMm=.6,centerMm=[0,8,-d/2-.3]),sourceIds=['photo-4'],basis='visible rear lens bezel; radius/depth estimated'),dict(id='camera-lens',kind='cylinder',color='#261b35',geometry=dict(radiusMm=1.7,depthMm=.08,centerMm=[0,8,-d/2-.64]),sourceIds=['photo-4'],basis='dark optical appearance only; no thermal input or optical transfer simulation')])
   outline=[[-w/2,-5],[-w/2-6,-9],[-w/2-6,-17],[-w/2,-21]]
   holes=[]
   for y in [-10.5,-16]:holes.append([[-w/2-3+1.6*math.cos(a),y+1.6*math.sin(a)] for a in [2*math.pi*i/24 for i in range(24)]])
   derived.append(dict(id='mount-ear',kind='plate',color=plan['shellColor'],geometry=dict(outlineMm=outline,holesMm=holes,depthMm=2,centerMm=[0,0,-d/2+1]),sourceIds=['photo-3','photo-4'],basis='two visibly open holes and ear outline estimated from front/rear photos; not threaded CAD'))
   front=['front-bezel','front-button','live-display'];rear=['rear-cover','camera-rim','camera-lens']
  derived.append(dict(id='pcb-main',kind='pcb-placeholder',geometry=dict(sizeMm=[w-5,h-7,1],centerMm=[0,0,0],labelSide='both',labelSizeMm=[min(w-7,22),5]),sourceIds=['official-shop'],basis='unknown board layout; outline/position estimated from enclosure space'))
  present['inspectionGroups']=[dict(id='front',parts=front,offsetMm=[0,0,22],sourceIds=['photo-3'],notes='Photo-derived exterior front group; viewing clearance only.'),dict(id='back',parts=rear,offsetMm=[0,0,-22],sourceIds=['photo-4' if id.endswith('k016-t') else 'photo-2'],notes='Estimated rear closure; exposes the fixed UNKNOWN interior without inventing PCB parts.')]
 (output/'geometry.bin').write_bytes(b''.join(blobs))
 textures=[]
 for texture in plan.get('texturePlans',[]):
  import cv2,numpy as np
  from PIL import Image,ImageDraw
  image=Image.open(source/texture['sourceFile']).convert('RGBA');quad=np.asarray(texture['quadPx'],dtype=np.float32);size=texture['sizePx'];matrix=cv2.getPerspectiveTransform(quad,np.asarray([[0,0],[size[0]-1,0],[size[0]-1,size[1]-1],[0,size[1]-1]],dtype=np.float32));image=Image.fromarray(cv2.warpPerspective(np.asarray(image),matrix,tuple(size),flags=cv2.INTER_LANCZOS4,borderMode=cv2.BORDER_CONSTANT));mask=Image.new('L',tuple(size),0);ImageDraw.Draw(mask).rounded_rectangle((0,0,size[0]-1,size[1]-1),radius=texture.get('cornerRadiusPx',0),fill=255);image.putalpha(mask);image.save(output/texture['file']);textures.append({**texture,'perspectiveMatrix':matrix.tolist(),'sha256':digest(output/texture['file'])})
 for f in plan['inputs']:
  if (source/f).resolve()!=(output/f).resolve():shutil.copyfile(source/f,output/f)
 m=dict(schemaVersion='1',id=id,unit='mm',strideFloats=6,binary='geometry.bin',parts=records,derivedParts=derived,presentation=present,dimensions=plan['dimensions'],referencePhotos=plan['referencePhotos'],provenance=dict(kind='source-derived',sources=plan['sources'],generator='tools/build-m5-legacy-kit-models.py',license='Original approximation geometry: MIT. Related K001 manufacturer CAD: M5Stack MIT. Official photographs: copyright M5Stack; see NOTICE.md.'),limitations=plan['limitations'],reviewViews=dict(front=dict(position=[0,0,180],span=max(w,h)*1.4),back=dict(position=[0,0,-180],span=max(w,h)*1.4),side=dict(position=[180,0,0],span=max(w,h)*1.4)),artifacts={f:digest(output/f) for f in ['geometry.bin']+list(plan['inputs'])})
 if textures:m['textures']=textures;m['artifacts'].update({t['file']:t['sha256']for t in textures})
 (output/'model.json').write_text(json.dumps(m,ensure_ascii=False,indent=2)+'\n')
 return m
if __name__=='__main__':
 ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('product_id');ap.add_argument('--output',required=True,type=Path);ap.add_argument('--source-root',default=ROOT,type=Path);ap.add_argument('--plan',type=Path);args=ap.parse_args();build(args.product_id,args.output,args.source_root,args.plan)
