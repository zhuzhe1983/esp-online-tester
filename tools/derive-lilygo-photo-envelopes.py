#!/usr/bin/env python3
"""Derive conservative exterior-envelope preview plans from retained product photos.

This is a measured image projection, not photogrammetry. The original photographs,
their hashes, segmentation parameters and chosen projection are retained. The
generated thickness and absolute scale are debug spacing, never product facts.
Use --photo-bank with directories named by product ID containing N.image and
N.source.json. No downloads, runtime changes or existing CAD replacements occur.
"""
import argparse
import hashlib
import json
import math
from pathlib import Path
import shutil
import cv2
import numpy as np
from PIL import Image

ROOT=Path(__file__).resolve().parents[1]
ASSETS=ROOT/'web/platform/assets'

def analyze(path,resolution=None,override=None):
    override=override or {}
    im=Image.open(path).convert('RGB')
    if override.get('cropXYXY'):im=im.crop(override['cropXYXY'])
    original=np.array(im);height,width=original.shape[:2]
    ratio=min(1,800/max(width,height));rgb=cv2.resize(original,None,fx=ratio,fy=ratio,interpolation=cv2.INTER_AREA)
    hsv=cv2.cvtColor(rgb,cv2.COLOR_RGB2HSV);gray=cv2.cvtColor(rgb,cv2.COLOR_RGB2GRAY)
    # White studio background is removed. Thin wires/text are excluded from the
    # coarse envelope; their electrical or mechanical structure is not invented.
    mask=((gray>25) if override.get('background')=='black' else ((gray<232)&((hsv[:,:,1]>18)|(gray<205)))).astype(np.uint8)*255
    mask[:12]=0;mask[-12:]=0;mask[:,:12]=0;mask[:,-12:]=0
    mask=cv2.morphologyEx(mask,cv2.MORPH_OPEN,cv2.getStructuringElement(cv2.MORPH_ELLIPSE,(7,7)))
    mask=cv2.morphologyEx(mask,cv2.MORPH_CLOSE,cv2.getStructuringElement(cv2.MORPH_ELLIPSE,(11,11)))
    contours,_=cv2.findContours(mask,cv2.RETR_EXTERNAL,cv2.CHAIN_APPROX_SIMPLE)
    if not contours:return None
    contour=max(contours,key=cv2.contourArea);area=cv2.contourArea(contour)
    if area<rgb.shape[0]*rgb.shape[1]*.035:return None
    hull=cv2.convexHull(contour);x,y,w,h=cv2.boundingRect(hull)
    if min(w,h)<35:return None
    hull=cv2.approxPolyDP(hull,.003*cv2.arcLength(hull,True),True)[:,0,:].astype(float)
    occupancy=area/(w*h)
    filled=np.zeros_like(mask);cv2.fillConvexPoly(filled,hull.astype(np.int32),255)
    color=np.median(rgb[(filled>0)&(gray<220)],axis=0).astype(int).tolist()
    screen=None
    if resolution:
        dark=((gray<90)&(filled>0)).astype(np.uint8)*255
        dark=cv2.morphologyEx(dark,cv2.MORPH_CLOSE,cv2.getStructuringElement(cv2.MORPH_RECT,(9,9)))
        ds,_=cv2.findContours(dark,cv2.RETR_EXTERNAL,cv2.CHAIN_APPROX_SIMPLE)
        desired=resolution['width']/resolution['height'];choices=[]
        for d in ds:
            dx,dy,dw,dh=cv2.boundingRect(d);a=cv2.contourArea(d);fraction=dw*dh/(w*h)
            if not(.12<fraction<.83) or min(dw,dh)<25 or a/(dw*dh)<.7:continue
            # Only an axis-aligned, filled, pixel-aspect-consistent dark window is
            # accepted. Angled/composite photos retain a separate debug panel.
            error=min(abs(math.log(dw/dh/desired)),abs(math.log(dw/dh*desired)))
            if error>.16:continue
            choices.append((a*(1-error),[dx,dy,dw,dh],abs(math.log(dw/dh/desired))>abs(math.log(dw/dh*desired))))
        if choices:_,rect,rotated=max(choices);screen={'rect':rect,'rotated':rotated,'basis':'candidate dark window; visual calibration pending'}
    return {'imageSize':[width,height],'analysisScale':ratio,'cropXYXY':override.get('cropXYXY'),'background':override.get('background','white'),'hull':hull.tolist(),'bounds':[x,y,w,h],'occupancy':occupancy,'color':color,'screen':screen,'score':occupancy+(0.25 if screen else 0)}

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--photo-bank',type=Path,required=True);p.add_argument('--audit',type=Path,required=True);p.add_argument('--runtime-profiles',type=Path);p.add_argument('--patches',type=Path,required=True);p.add_argument('--overrides',type=Path,default=Path(__file__).with_name('lilygo-photo-outline-overrides.json'));p.add_argument('--ids',nargs='*');args=p.parse_args()
    overrides=json.loads(args.overrides.read_text()) if args.overrides.exists() else {}
    audit=json.loads(args.audit.read_text());runtime=json.loads(args.runtime_profiles.read_text())if args.runtime_profiles else{'items':[],'sources':[]};runtime_by_id={r['productId']:r for r in runtime['items']};runtime_sources={s['id']:s for s in runtime.get('sources',[])};patches=json.loads(args.patches.read_text());patch_by_id={r['id']:r for r in patches['items']};rows=[]
    for row in audit['items']:
        id=row['id'];folder=ASSETS/id;planfile=folder/'model-plan.json'
        if args.ids and id not in args.ids:continue
        if not row['wasRuntimeUnavailable']or not planfile.exists():continue
        plan=json.loads(planfile.read_text())
        if plan.get('cad')or plan.get('manualPhotoCalibration'):continue
        profile=runtime_by_id.get(id);resolution=({k:profile['display'][k]for k in ['width','height']}if profile else row.get('normalizedSpecifications',{}).get('singleResolution'))
        candidates=[];photos=[];source_ids=[];source_records=[]
        for i in range(3):
            file=args.photo_bank/id/(str(i)+'.image');receipt=args.photo_bank/id/(str(i)+'.source.json')
            if not file.exists()or not receipt.exists():continue
            source=json.loads(receipt.read_text());raw=file.read_bytes()
            if hashlib.sha256(raw).hexdigest()!=source['sha256']:raise ValueError('Photo hash mismatch: '+id)
            extension={'JPEG':'.jpg','PNG':'.png','WEBP':'.webp'}.get(Image.open(file).format,'.image');name='reference-'+str(i)+extension;shutil.copyfile(file,folder/name)
            source.update(id=id+'-official-photo-'+str(i),file=name,kind='manufacturer-photo');photos.append({'sourceId':source['id'],'file':name,'view':'official gallery view '+str(i),'notes':'Original manufacturer image; no generated imagery.'});source_ids.append(source['id']);source_records.append(source)
            override=overrides.get(id,{})
            result=analyze(file,resolution,override if override.get('view')==i else None)
            if result:candidates.append((result['score']+(.12 if i==1 else 0)+(10 if override.get('view')==i else 0),i,result))
        if not candidates:
            plan['debugPreview']['reason']='No usable isolated product contour recovered from the fetched official photographs; screen/unknown debug fallback retained.'
            rows.append({'id':id,'mode':plan['mode'],'reason':plan['debugPreview']['reason'],'photosFetched':len(photos)});planfile.write_text(json.dumps(plan,ensure_ascii=False,indent=2)+'\n');continue
        _,index,chosen=max(candidates,key=lambda c:c[0]);x,y,w,h=chosen['bounds'];scale=60/max(w,h);bw,bh=w*scale,h*scale;depth=max(2.2,min(bw,bh)*.12)
        hull=np.array(chosen['hull']);hull[:,0]=(hull[:,0]-x-w/2)*scale;hull[:,1]=(y+h/2-hull[:,1])*scale
        # Image coordinates are Y-down; reverse winding to retain outward faces.
        hull=hull[::-1].tolist();color='#'+''.join(f'{max(35,min(170,c)):02x}'for c in chosen['color'])
        plan.update(mode='photo-derived',dimensions={'width':bw,'height':bh,'depth':depth,'basis':'photo projection normalized to 60 mm for debugging; absolute scale and depth uncalibrated'},parts=[{'name':'photo-envelope','role':'envelope','kind':'polygon','sizeMm':[bw,bh,depth],'outlineMm':hull,'centerMm':[0,0,0],'color':color,'basis':'photo-projection','sourceIds':[id+'-official-photo-'+str(index)]}],referencePhotos=photos,retainedFiles=[p['file']for p in photos])
        plan['derivedParts']=[{'id':'pcb-main','kind':'pcb-placeholder','basis':'unknown; display spacing only','sourceIds':source_ids,'color':'#536066','geometry':{'sizeMm':[bw*.68,bh*.68,.3],'centerMm':[0,0,-depth/2-.2],'labelSide':'back','labelSizeMm':[min(bw*.55,bh*2.2),min(bw*.55,bh*2.2)/4]},'notes':'Replaceable UNKNOWN internal board, not a derived PCB outline or guessed components.'}]
        presentation={'bodySizeMm':[bw,bh,depth+.5]}
        if resolution:
            # Detected dark windows are intentionally not auto-promoted: screen
            # placement needs visual approval. The live panel sits beside the
            # photo envelope with a visible DEBUG label, avoiding counterfeit UI.
            sw=45*resolution['width']/max(resolution.values());sh=45*resolution['height']/max(resolution.values());sx=bw/2+8+sw/2
            plan['parts'].append({'name':'debug-screen-frame','role':'component','kind':'ring','sizeMm':[sw+3,sh+8,1.4],'openingMm':[sw,sh],'radiusMm':2,'innerRadiusMm':.2,'centerMm':[sx,0,0],'color':'#20343d','basis':'debug-layout'})
            presentation['screen']={'sizeMm':[sw,sh],'positionMm':[sx,1,.72],'rotation':[0,0,0],'notes':'Separate live debug panel; physical display location has not been calibrated.'}
            plan['derivedParts'].append({'id':'debug-panel-label','kind':'printed-marking','basis':'debug-layout','sourceIds':source_ids,'lines':['LIVE DEBUG'],'color':'#bde5d8','geometry':{'sizeMm':[sw,3.2],'centerMm':[sx,-sh/2-2,.72]}})
            presentation['bodySizeMm']=[bw+sw+12,max(bh,sh+8),depth+.5]
            patch_by_id[id]['displayHint']={**resolution,'sourceIds':profile['sourceIds']if profile else row['sourceIds'],'basis':'selected BSP native orientation'if profile else'official table; runtime orientation unverified','physicalSizeMm':None}
        plan['presentation']=presentation;plan['photoDerivation']={'method':'largest connected studio-photo foreground, convex projected envelope; not photogrammetry','chosenSourceId':id+'-official-photo-'+str(index),'chosenView':index,'analysis':chosen,'manualViewSelection':overrides.get(id),'modelMillimetresPerAnalysisPixel':scale,'scaleBasis':'debug layout only','thicknessBasis':'debug spacing only; not recovered from side views','algorithm':'tools/derive-lilygo-photo-envelopes.py','independentViews':[{'view':i,'bounds':r['bounds'],'occupancy':r['occupancy']}for _,i,r in candidates],'multiViewStatus':'Additional manufacturer views retained for comparison; perspective, depth and hidden topology remain uncalibrated.'}
        plan['provenance']['kind']='photo-derived';plan['provenance']['sources']=[s for s in plan['provenance']['sources']if not s['id'].startswith(('model-photo-',id+'-official-photo-'))]+source_records
        if profile:
            known={s['id']for s in plan['provenance']['sources']}
            plan['provenance']['sources'].extend(s.copy()for sid,s in runtime_sources.items()if sid in profile['sourceIds']and sid not in known)
        plan['limitations']=['依据官方多角度图片建立外轮廓粗模；这是照片投影的凸包，不是摄影测量或制造用 CAD。','侧壁厚度和绝对尺度尚未标定；画面以最长边 60 mm 归一化，不能当作实物尺寸。','未猜测内部元件、丝印、插针线序或连接器精确结构；背面 UNKNOWN 占位可替换。','有屏幕时，真实固件画面放在独立 LIVE DEBUG 面板；实体屏幕窗口尚未逐款校准。','原始照片仅作为来源，不将照片里的演示画面贴到实时屏幕。']
        plan['debugPreview']={'mode':'photo-derived','isPhysicalReplica':False,'physicalDimensionsVerified':False,'displayPlacement':'separate-debug-panel'if resolution else'no-verified-display','reason':'Multi-view exterior source references exist; projection recovered, physical depth/scale/window placement uncalibrated.'};span=max(presentation['bodySizeMm'][:2])*1.2;plan['reviewViews']={'front':{'position':[0,0,200],'span':span},'back':{'position':[0,0,-200],'span':span},'side':{'position':[200,0,0],'span':span}}
        planfile.write_text(json.dumps(plan,ensure_ascii=False,indent=2)+'\n');patch_by_id[id].update(mode='photo-derived',sourceIds=row['sourceIds']+source_ids);patch_by_id[id]['model'].update(basis='photo-derived',mode='photo-derived',notes='官方多视图照片轮廓粗模；厚度、绝对尺度及实体屏幕位置未标定。')
        rows.append({'id':id,'mode':'photo-derived','photosFetched':len(photos),'chosenView':index,'contourOccupancy':chosen['occupancy'],'physicalScaleVerified':False,'screenPlacement':'separate-debug-panel'if resolution else'not-declared'})
    args.patches.write_text(json.dumps(patches,ensure_ascii=False,indent=2)+'\n');print(json.dumps({'items':rows,'count':len(rows)},ensure_ascii=False))
if __name__=='__main__':main()
