#!/usr/bin/env python3
"""Independent drawing dimensions + manufacturer source fidelity + photo silhouette audit.
Run build first; capture four 800x800 model-review views to --renders.
Requires numpy, trimesh, Pillow, opencv-python-headless. No fit parameters optimized here.
"""
import argparse,json,hashlib,datetime
from pathlib import Path
import numpy as np, trimesh, cv2
from PIL import Image,ImageDraw
p=argparse.ArgumentParser();p.add_argument('--assets',type=Path,default=Path('web/platform/assets/m5stack-stopwatch'));p.add_argument('--renders',type=Path,default=Path('output/playwright'));p.add_argument('--output',type=Path,default=Path('docs/platform/evidence/stopwatch-model'));p.add_argument('--allow-unpassed',action='store_true',help='Write a diagnostic report without requiring every photo view to pass.');a=p.parse_args();a.output.mkdir(parents=True,exist_ok=True)
def sha(f):return hashlib.sha256(f.read_bytes()).hexdigest()
meta=json.loads((a.assets/'model.json').read_text());parts=trimesh.load(a.assets/'source.stl').split(only_watertight=False);data=(a.assets/'geometry.bin').read_bytes();matrix=np.array(meta['sourceTransform']['matrixRows']);errors=[];all_vertices=[]
for item in meta['parts']:
    v=np.frombuffer(data,dtype='<f4',offset=item['byteOffset'],count=item['vertexCount']*6).reshape(-1,6)[:,:3]
    original=parts[item['sourceComponent']];expected=original.vertices[original.faces].reshape(-1,3)@matrix[:3,:3].T+matrix[:3,3]
    errors.append(float(abs(v-expected).max()));all_vertices.append(v)
body=all_vertices[0];size=body.max(axis=0)-body.min(axis=0)
checks=[{'name':'CAD rigid transform, all source triangle positions','measuredMm':max(errors),'toleranceMm':.05,'pass':max(errors)<=.05},
 {'name':'Body diameter X vs official drawing φ51.95','measuredMm':float(size[0]),'targetMm':51.95,'toleranceMm':.05,'pass':abs(float(size[0])-51.95)<=.05},
 {'name':'Body diameter Y vs official drawing φ51.95','measuredMm':float(size[1]),'targetMm':51.95,'toleranceMm':.05,'pass':abs(float(size[1])-51.95)<=.05},
 {'name':'Assembled thickness, shell + explicitly added glass','measuredMm':float(size[2])+.5,'targetMm':15.5,'toleranceMm':.05,'pass':abs(float(size[2])+.5-15.5)<=.05,'note':'Glass thickness is inferred from drawing minus source shell, not a second independent measurement.'}]
photos=[];panels=[]
# Fixed threshold and fixed photo/view transforms; do not refit from the candidate render.
def mask(im):
    arr=np.array(im.convert('RGB'));m=(np.min(arr,axis=2)<205).astype('uint8')
    contours,_=cv2.findContours(m,cv2.RETR_EXTERNAL,cv2.CHAIN_APPROX_SIMPLE);out=np.zeros_like(m);cv2.drawContours(out,[max(contours,key=cv2.contourArea)],-1,1,-1);return out.astype(bool)
for view,num in [('front','06'),('back','07'),('right','08'),('bottom','09')]:
    reference=Image.open(a.assets/f'reference-photo-{num}.webp').convert('RGB').resize((800,800),Image.Resampling.LANCZOS)
    render_file=a.renders/f'stopwatch-{view}-render.png'
    if not render_file.exists(): photos.append({'view':view,'status':'unmeasured'});continue
    rendered=Image.open(render_file).convert('RGB');assert rendered.size==(800,800)
    r,m=mask(reference),mask(rendered);intersection=(r&m).sum();union=(r|m).sum();iou=float(intersection/union)
    # 0.95 is a predeclared visual silhouette gate, not a manufacturing claim.
    photos.append({'view':view,'photoId':f'photo-{num}','referenceSha256':sha(a.assets/f'reference-photo-{num}.webp'),'renderSha256':sha(render_file),'silhouetteIoU':iou,'minimum':.95,'pass':iou>=.95,'method':'800x800 fixed aligned views; min RGB <205, largest external filled contour. Interior labels/screen/material are excluded from score.'})
    diff=np.full((800,800,3),255,dtype=np.uint8);diff[r&m]=[50,70,60];diff[r&~m]=[255,85,85];diff[m&~r]=[60,145,255]
    panel=Image.new('RGB',(1600,850),'white');d=ImageDraw.Draw(panel);d.text((20,10),f'{view} | photo / render / 50% overlay / silhouette difference | IoU={iou:.4f}',fill='black')
    for j,im in enumerate([reference,rendered,Image.blend(reference,rendered,.5),Image.fromarray(diff)]):panel.paste(im.resize((400,800//2)),(j*400,40))
    panel=panel.crop((0,0,1600,440));panel.save(a.output/f'{view}-comparison.webp',quality=90);panels.append(panel)
report={'schemaVersion':'1','checkedAt':datetime.datetime.now(datetime.timezone.utc).isoformat().replace('+00:00','Z'),'modelSha256':sha(a.assets/'model.json'),'geometrySha256':sha(a.assets/'geometry.bin'),'rendererSha256':sha(Path('web/platform/stopwatch-model.js')),'dimensions':checks,'photos':photos,'limitations':['Silhouette fit uses published outer diameter; it does not independently certify absolute physical scale.','Photo tests do not certify internal geometry, optical active display edge, material reflectance or connector manufacturing accuracy.','Display deliberately blank. Rear label derived from photo-07, so its appearance is not an independent validation sample.','No physical caliper or hardware measurement has been performed.']}
report['modelReviewSha256']=sha(Path('web/platform/model-review.js'))
report['verificationScriptSha256']=sha(Path(__file__))
report['cameraViews']={'front':{'position':[0,0,180],'up':[0,1,0],'spanMm':84.78},'back':{'position':[0,0,-180],'up':[0,1,0],'spanMm':85.246},'right':{'position':[180,0,0],'up':[0,0,1],'spanMm':73.24},'bottom':{'position':[0,-180,0],'up':[0,0,1],'spanMm':73.24}}
(a.output/'verification.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
print(json.dumps({'dimensions':checks,'photos':photos},ensure_ascii=False,indent=2))
assert all(c['pass'] for c in checks), 'Dimensional gate failed'
# Preserve a failed photo test rather than discarding evidence or changing its threshold.
if any(not c.get('pass',False) for c in photos):
    print('VISUAL GATE HAS UNPASSED VIEWS: inspect the saved comparisons.')
    if not a.allow_unpassed: raise SystemExit(1)
