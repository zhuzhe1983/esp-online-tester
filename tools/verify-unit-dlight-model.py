#!/usr/bin/env python3
"""Fixed DLight references versus actual 800x800 browser views; no model fitting."""
import argparse,hashlib,json,datetime
from pathlib import Path
import numpy as np,cv2
from PIL import Image,ImageDraw
p=argparse.ArgumentParser();p.add_argument('--renders',type=Path,default=Path('output/playwright'));a=p.parse_args()
id='m5stack-unit-dlight-u136';assets=Path('web/platform/assets')/id;evidence=Path('docs/platform/evidence')/id
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
meta=json.loads((assets/'model.json').read_text());report=json.loads((evidence/'verification.json').read_text());assert report['modelSha256']==sha(assets/'model.json')
report['dimensions']=[x for x in report['dimensions']if x['name'] in ['shell-front source-fidelity','shell-back source-fidelity','shell-width','shell-height','shell-depth']]
# Official drawing: two diameter5 mounting holes,16 apart,12 from bottom.
binary=(assets/'geometry.bin').read_bytes();v=np.concatenate([np.frombuffer(binary,dtype='<f4',offset=x['byteOffset'],count=x['vertexCount']*6).reshape(-1,6)[:,:3] for x in meta['parts'] if x['sourceComponent'] is not None]);centers=[]
for x in [-8,8]:
 r=np.linalg.norm(v[:,:2]-[x,-4],axis=1);ring=np.unique(v[(r>2.3)&(r<2.7),:2],axis=0);assert len(ring)>=30
 coeff=np.linalg.lstsq(np.column_stack([2*ring[:,0],2*ring[:,1],np.ones(len(ring))]),(ring**2).sum(1),rcond=None)[0];cx,cy,c=coeff;centers.append([cx,cy]);radius=np.sqrt(c+cx*cx+cy*cy)
 for name,value,target in [(f'hole{x:+} diameter',2*radius,5),(f'hole{x:+} from bottom',cy+16,12)]:report['dimensions'].append({'name':name,'measuredMm':float(value),'targetMm':target,'errorMm':float(abs(value-target)),'toleranceMm':.05,'pass':bool(abs(value-target)<=.05),'sourceId':'dimensions'})
value=float(np.linalg.norm(np.array(centers[0])-centers[1]));report['dimensions'].append({'name':'hole center distance','measuredMm':value,'targetMm':16,'errorMm':abs(value-16),'toleranceMm':.05,'pass':abs(value-16)<=.05,'sourceId':'dimensions'})
def mask(im,threshold):
 m=(np.asarray(im.convert('RGB')).min(2)<threshold).astype('uint8');contours,_=cv2.findContours(m,cv2.RETR_EXTERNAL,cv2.CHAIN_APPROX_SIMPLE);out=np.zeros_like(m);cv2.drawContours(out,[max(contours,key=cv2.contourArea)],-1,1,-1);return out.astype(bool)
photos=[]
for view,num in [('front',4),('back',2),('connector',3)]:
 src=assets/f'reference-photo-{num}.webp';ref=Image.open(src).convert('RGB');path=a.renders/f'dlight-{view}.png';im=Image.open(path).convert('RGB');assert ref.size==im.size==(800,800)
 mask_path=a.renders/f'dlight-{view}-mask.png';rm=Image.open(mask_path) if mask_path.exists() else im
 r,m=mask(ref,248),mask(rm,128 if mask_path.exists() else 248);iou=float((r&m).sum()/(r|m).sum());quant=view in ['front','back']
 photo={'view':view,'sourceId':f'photo-{num}','sourceSha256':sha(src),'renderSha256':sha(path),'silhouetteIoU':iou,'camera':meta['reviewViews'][view],'status':('pass' if iou>=.95 else 'fail') if quant else 'qualitative-only','method':'Fixed800x800 views; reference minRGB<248; actual renderer black-mask pass minRGB<128 (or render minRGB<248 for connector), largest filled exterior contour. Interior holes, labels and material excluded. No image/model deformation.'}
 if quant:photo['minimum']=.95
 else:photo['reason']='Perspective photograph without camera intrinsics; approximate orthographic pose, no quantitative1:1 pass claim.'
 if mask_path.exists():photo['renderMaskSha256']=sha(mask_path)
 photos.append(photo);diff=np.full((800,800,3),255,dtype=np.uint8);diff[r&m]=[50,70,60];diff[r&~m]=[255,85,85];diff[m&~r]=[60,145,255]
 panel=Image.new('RGB',(1600,440),'white');ImageDraw.Draw(panel).text((12,12),f'{view}: photo / render / overlay / outer-mask difference | IoU{iou:.4f}'+(''if quant else' | qualitative pose only'),fill='black')
 for i,img in enumerate([ref,im,Image.blend(ref,im,.5),Image.fromarray(diff)]):panel.paste(img.resize((400,400)),(i*400,40))
 panel.save(evidence/f'{view}-comparison.webp',quality=90)
report.update({'checkedAt':datetime.datetime.now(datetime.timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ'),'photos':photos,'verificationScriptSha256':sha(Path(__file__)),'status':'partial','visualLimits':['Exact U136 manufacturer shell CAD; original triangles retained with rigid transform only.','Internal UNKNOWN board is simplified; the visible sensor top uses sourced package bounds with estimated depth, not a complete PCB reconstruction.','Rear screw absent; original CAD M5 lettering shallow; connector detail and depth approximate.','Back label uses the same photo as texture source and is not independently validated by that image.','Front and back have orthographic outer-contour checks; the oblique connector pose is qualitative only. Back paper uses the same source photograph, not independent validation.']})
(evidence/'verification.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n');print(json.dumps({'dimensions':report['dimensions'],'photos':photos},indent=2));assert all(x['pass']for x in report['dimensions']);assert all(x['status']!='fail'for x in photos)
