#!/usr/bin/env python3
"""Calibrated same-photo overlay, NOT independent 1:1 certification. Needs numpy/OpenCV."""
import argparse,json,math,hashlib
from pathlib import Path
import numpy as np
import cv2
ROOT=Path(__file__).resolve().parents[1]
p=argparse.ArgumentParser(description=__doc__);p.add_argument('--evidence',type=Path,required=True);a=p.parse_args();reports=[]
for key in ['ink4','ink4-plus']:
 pid='metalio-'+key;assets=ROOT/'web/platform/assets'/pid;plan=json.loads((assets/'model-plan.json').read_text());photo=cv2.imread(str(assets/'reference-front.png'));render=cv2.imread(str(a.evidence/(pid+'-front.png')))
 sw,sh=plan['screenSizeMm'];sx,sy=plan['screenCenterMm'];w,h,d=plan['presentation']['bodySizeMm'];span=h*1.45;scale=render.shape[0]/span;center=render.shape[0]/2
 def pixels(ps):return np.float32([[center+x*scale,center-y*scale]for x,y in ps])
 screen=[[sx-sw/2,sy+sh/2],[sx+sw/2,sy+sh/2],[sx+sw/2,sy-sh/2],[sx-sw/2,sy-sh/2]]
 M=cv2.getPerspectiveTransform(pixels(screen),np.float32(plan['calibration']['screenPixels']))
 warped=cv2.warpPerspective(render,M,(photo.shape[1],photo.shape[0]),borderValue=(255,255,255))
 mask=np.any(warped<245,axis=2);overlay=photo.copy();overlay[mask]=(photo[mask].astype(float)*.55+warped[mask]*.45).astype(np.uint8)
 body=[[-w/2,h/2],[w/2,h/2],[w/2,-h/2],[-w/2,-h/2]];projected=cv2.perspectiveTransform(pixels(body)[None],M)[0];reference=np.float32(plan['calibration']['enclosurePixels']);residual=np.linalg.norm(projected-reference,axis=1)
 cv2.polylines(overlay,[np.round(projected).astype(np.int32)],True,(255,100,0),2);cv2.polylines(overlay,[reference.astype(np.int32)],True,(0,180,0),2)
 cv2.putText(overlay,'BLUE: model  GREEN: photo corners',(15,30),cv2.FONT_HERSHEY_SIMPLEX,.6,(30,30,30),2);cv2.imwrite(str(a.evidence/(pid+'-overlay.png')),overlay)
 reports.append({'id':pid,'sourceCadSha256':plan['sourceSha256'],'sourcePhotoSha256':hashlib.sha256((assets/'reference-front.png').read_bytes()).hexdigest(),'frontCamera':{'spanMm':span,'positionMm':[0,0,250],'targetMm':[0,0,0],'up':[0,1,0]},'renderPixels':list(render.shape[:2][::-1]),'homography':M.tolist(),'enclosureCornerErrorPixels':residual.tolist(),'maxErrorPixels':float(max(residual)),'method':'Official CAD front render reprojected to the source photo by 4 estimated display corners; comparison residual only. Corners are virtual rectangle corners (rounded silhouette not segmented).','cadConversionStatus':'PASS: original scale, components and assembly retained','oneToOneStatus':'NOT PASSED','limitations':['Body geometry comes from independent manufacturer CAD; display plane and cover-print calibration still use this photo.','CAD dimensions measured; no independent dimension drawing or physical sample verification. STL unit interpretation is millimeters, not explicit source metadata.','No orthogonal side/reference for thickness; Plus back photo missing.','Inset/virtual corners selected manually; marketing distortion and camera uncertainty retained.']})
(a.evidence/'photo-comparison.json').write_text(json.dumps(reports,indent=2)+'\n');print(json.dumps([{'id':r['id'],'maxErrorPixels':r['maxErrorPixels'],'oneToOneStatus':r['oneToOneStatus']}for r in reports],indent=2))
