#!/usr/bin/env python3
"""Independent official drawing targets + raw CAD fidelity + public-photo comparisons.
Capture gps-model-review.html views first. Oblique poses are explicitly qualitative.
"""
import argparse,datetime,hashlib,json
from pathlib import Path
import numpy as np,trimesh,cv2
from PIL import Image,ImageDraw
p=argparse.ArgumentParser();p.add_argument('--assets',type=Path,default=Path('web/platform/assets/m5stack-gps-u032-v11'));p.add_argument('--renders',type=Path,default=Path('output/playwright'));p.add_argument('--output',type=Path,default=Path('docs/platform/evidence/gps-model'));a=p.parse_args();a.output.mkdir(parents=True,exist_ok=True)
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
meta=json.loads((a.assets/'model.json').read_text());raw=trimesh.load(a.assets/'source.stl').split(only_watertight=False);binary=(a.assets/'geometry.bin').read_bytes();matrix=np.array(meta['sourceTransform']['matrixRows']);checks=[];allv=[]
def check(name,measured,target,tolerance,source='dimension-drawing'):
 checks.append({'name':name,'measuredMm':float(measured),'targetMm':target,'errorMm':float(abs(measured-target)),'toleranceMm':tolerance,'pass':bool(abs(measured-target)<=tolerance),'sourceId':source})
for part in meta['parts']:
 v=np.frombuffer(binary,dtype='<f4',offset=part['byteOffset'],count=part['vertexCount']*6).reshape(-1,6)[:,:3];original=raw[part['sourceComponent']];expected=original.vertices[original.faces].reshape(-1,3)@matrix[:3,:3].T+matrix[:3,3]
 check(part['name']+' rigid transformed source triangles',abs(v-expected).max(),0,.05,'official-cad');allv.append(v)
v=np.concatenate(allv);sizes=v.max(0)-v.min(0)
for axis,target in enumerate([24,48,8]):check(['width','height','depth'][axis],sizes[axis],target,.05)
# Fit each source through-hole circle, selecting only its 2.3..2.8 mm radius neighborhood.
centers=[]
for x in [-8,8]:
 r=np.linalg.norm(v[:,:2]-[x,-12],axis=1);ring=np.unique(v[(r>2.3)&(r<2.8),:2],axis=0);assert len(ring)>=30
 coeff=np.linalg.lstsq(np.column_stack([ring[:,0]*2,ring[:,1]*2,np.ones(len(ring))]),(ring**2).sum(1),rcond=None)[0];cx,cy,c=coeff;radius=np.sqrt(c+cx*cx+cy*cy);centers.append([cx,cy]);check(f'hole {x:+d} diameter',2*radius,5,.05);check(f'hole {x:+d} from bottom',cy+24,12,.05)
check('hole center spacing',np.linalg.norm(np.array(centers[0])-centers[1]),16,.05)
# Largest filled external contour isolates the published silhouette; holes/labels excluded.
def mask(im):
 rgb=np.array(im.convert('RGB'));m=(rgb.min(2)<248).astype('uint8');contours,_=cv2.findContours(m,cv2.RETR_EXTERNAL,cv2.CHAIN_APPROX_SIMPLE);out=np.zeros_like(m);cv2.drawContours(out,[max(contours,key=cv2.contourArea)],-1,1,-1);return out.astype(bool)
photos=[]
for view,num in [('front',7),('back',8),('oblique',6),('rear',5)]:
 ref=Image.open(a.assets/f'reference-photo-{num}.webp').convert('RGB');renderPath=a.renders/f'gps-{view}-render.png';render=Image.open(renderPath).convert('RGB');assert ref.size==render.size==(800,800)
 r,m=mask(ref),mask(render);iou=float((r&m).sum()/(r|m).sum());quantified=view in ['front','back']
 photos.append({'view':view,'sourceId':f'photo-{num}','referenceSha256':sha(a.assets/f'reference-photo-{num}.webp'),'renderSha256':sha(renderPath),'silhouetteIoU':iou,'acceptance':{'minimum':.95,'pass':iou>=.95} if quantified else {'status':'qualitative-only','reason':'Uncalibrated perspective photograph; approximate orthographic pose, no quantitative pass claim.'},'method':'800x800; min RGB <248, largest external filled contour. Labels and holes excluded; fixed camera only, no model deformation.'})
 diff=np.full((800,800,3),255,dtype=np.uint8);diff[r&m]=[50,70,60];diff[r&~m]=[255,85,85];diff[m&~r]=[60,145,255]
 panel=Image.new('RGB',(1600,440),'white');d=ImageDraw.Draw(panel);d.text((12,12),f'{view}: official photo / render / 50% overlay / outer silhouette difference | IoU {iou:.4f}'+(' | qualitative pose only' if not quantified else ''),fill='black')
 for j,im in enumerate([ref,render,Image.blend(ref,render,.5),Image.fromarray(diff)]):panel.paste(im.resize((400,400)),(j*400,40))
 panel.save(a.output/f'{view}-comparison.webp',quality=90)
unknown=a.renders/'gps-unknown-render.png'
if unknown.exists():Image.open(unknown).save(a.output/'pcb-unknown.webp',quality=90)
report={'schemaVersion':'1','checkedAt':datetime.datetime.now(datetime.timezone.utc).isoformat().replace('+00:00','Z'),'productId':'m5stack-gps-u032-v11','sourceCommit':meta['provenance']['commit'],'sourceCadSha256':meta['provenance']['sourceSha256'],'modelSha256':sha(a.assets/'model.json'),'geometrySha256':sha(a.assets/'geometry.bin'),'connectorRendererSha256':sha(Path('web/platform/grove-geometry.js')),'rendererSha256':sha(Path('web/platform/gps-model.js')),'reviewSha256':sha(Path('web/platform/gps-model-review.js')),'verificationScriptSha256':sha(Path(__file__)),'dimensions':checks,'photos':photos,'cameraViews':{'front':{'position':[0,0,180],'up':[0,1,0],'target':[.2,0,0],'spanMm':72.45},'back':{'position':[0,0,-180],'up':[0,1,0],'target':[0,0,0],'spanMm':72.45},'oblique':{'position':[65,-160,95],'up':[0,1,0],'target':[0,0,0],'rollRadians':.53,'spanMm':73},'rear':{'position':[-45,-75,-160],'up':[0,1,0],'target':[0,0,0],'rollRadians':-.45,'spanMm':73}},'limitations':['Manufacturer shell CAD is dimensionally consistent with official drawing; this is not a complete product manufacturing model.','GPIO, baud timing, power, RF, satellite acquisition and GNSS module commands are not modeled.','PCB layout unknown; blank board shape/thickness/position are estimates, excluded from all dimensional claims.','Photo labels are derived from front/back photos, not independent appearance validation for those areas.','Cyan connector details and visible fastener are estimates; drawings constrain opening and location only.','Strong platform lighting uses adjusted display reflectance and unlit photo labels; material identity and reflectance are not verified.','Oblique photographs are qualitative only: lens/projection/calibrated pose unavailable. No physical hardware measurement performed.']}
(a.output/'verification.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n');print(json.dumps({'dimensions':checks,'photos':photos},indent=2));assert all(c['pass'] for c in checks);assert all(x['acceptance'].get('pass',True) for x in photos)
