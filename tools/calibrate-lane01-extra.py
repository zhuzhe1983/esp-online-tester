"""Register fixed review cameras to independent official photos, never scale geometry."""
import json
from pathlib import Path
import cv2,numpy as np
from PIL import Image
IDS='k146 s007 k051-b k131-v11 k147 c126-ext c123 k150 k010-aws-v13 k128-se c050-b k016-c'.split()
for short in IDS:
 assets=Path('web/platform/assets')/('m5stack-'+short);p=assets/'model-plan.json';d=json.loads(p.read_text());meta=json.loads((assets/'model.json').read_text());refs={p['sourceId']:p for p in meta['referencePhotos']};d.setdefault('reviewViews',{})
 for texture in d.get('textureSources',[]):
  for rect in texture.get('alphaMask',{}).get('includeRoundedRects',[]):
   if 'bounds'in rect:rect['xyxy']=rect.pop('bounds')
   if 'radius'in rect:rect['radiusPx']=rect.pop('radius')
 for photo in d['photos']:
  view=photo['view'];source=refs[photo['sourceId']];im=Image.open(assets/source['file']).convert('RGB').resize((800,800));mask=(np.asarray(im).min(2)<245).astype('uint8');contours,_=cv2.findContours(mask,cv2.RETR_EXTERNAL,cv2.CHAIN_APPROX_SIMPLE);x,y,w,h=cv2.boundingRect(max(contours,key=cv2.contourArea));size=d['dimensions'];width=size['width'];span=width*800/w;back=view in ['back','handset-back','board-back','label-side'];front=view in ['front','proto-front','component-side','handset-front'];
  if back or front:
   sign=-1 if back else 1;target=[(400-x-w/2)*span/800*sign,(y+h/2-400)*span/800,0];d['reviewViews'][view]={'span':span,'target':target,'position':[target[0],target[1],250*sign],'up':[0,1,0],'sourceMask':{'thresholdMinRgb':245,'bboxPixels':[x,y,w,h],'sourcePixels':[800,800]},'basis':'Independent official photograph width and documented body width set camera scale/center only; no geometry scaling. Scope differences and manufacturer-dimension discrepancies remain failures.'}
   if view in ['front','back']:
    photo['silhouetteComparable']=True;photo['silhouetteNotes']='Near-orthographic independent official body photo. Camera registered by documented body width; source CAD unchanged; measurement and scope mismatches remain failures.'
  else:
   pos=[0,-250,0];up=[0,0,1]
   if short in ['k010-aws-v13','k128-se']:pos=[-250,0,0]
   if view in ['covered-front','covered-angle','marketing-angle','front-package','dock','shield-and-headers']:
    pos=[85,65,250];up=[0,1,0]
   d['reviewViews'][view]={'span':max(d['presentation']['bodySizeMm'])*1.45,'position':pos,'target':[0,0,0],'up':up,'basis':'Qualitative camera orientation chosen from official photograph; perspective/crop not metrically calibrated.'}
 p.write_text(json.dumps(d,ensure_ascii=False,indent=2)+'\n')
 print(short,'review cameras registered; original geometry unchanged')
