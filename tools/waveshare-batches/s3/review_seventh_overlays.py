import json,pathlib,gzip,hashlib,math
import numpy as np
from scipy.spatial import ConvexHull,QhullError
from PIL import Image,ImageDraw
W=pathlib.Path(__file__).resolve().parents[3];O=W/'docs/research/waveshare-s3-20261123/seventh-verification/model-views';O.mkdir(parents=True,exist_ok=True)
ids=['waveshare-esp32-s3-lcd-1-9', 'waveshare-esp32-s3-touch-lcd-1-9', 'waveshare-esp32-s3-touch-lcd-3-5', 'waveshare-esp32-s3-touch-lcd-3-5-c', 'waveshare-esp32-s3-lcd-1-3', 'waveshare-esp32-s3-lcd-1-3-b', 'waveshare-esp32-s3-lcd-1-3-c', 'waveshare-esp32-s3-touch-lcd-1-85c', 'waveshare-esp32-s3-touch-lcd-1-85c-box-en', 'waveshare-esp32-s3-touch-lcd-1-85c-box', 'waveshare-esp32-s3-touch-amoled-2-41-v2', 'waveshare-esp32-s3-touch-amoled-2-41-v2-b'];records=[]
for id in ids:
 A=W/'web/platform/assets'/id;p=json.loads((A/'model-plan.json').read_text());m=json.loads((A/'model.json').read_text());reg=p.get('photoRegistration',{});note='Source photo alignment is approximate; hidden outlines do not establish physical fit.'
 if id.endswith('lcd-1-9'):
  touch='touch' in id
  affine=np.array([[0,8.346 if touch else 8.568,355.775 if touch else 357.34],[8.299 if touch else 8.527,0,246.5 if touch else 265.0]])
  note+=' Optical glass dimension endpoints register the native unscaled CAD; active-area and optical-window definitions remain separate.'
 elif 'lcd-3-5' in id:
  affine=np.array([[0,5.34,361.5],[5.34,0,276.5]])
  note+=' Reference depicts case-C. Bare native assembly is overlaid only as its internal footprint, not a bare-photo or case-fit claim.'
 else:
  l,t,r,b=reg['sourcePixels'];w,h=reg['physicalSizeMm'];cx,cy,_=reg['centerMm'];affine=np.array([[(r-l)/w,0,(l+r)/2-cx*(r-l)/w],[0,-(b-t)/h,(t+b)/2+cy*(b-t)/h]])
  if 'amoled-2-41' in id:
   affine=affine @ np.array([[0,1,0],[-1,0,0],[0,0,1]])
   if id.endswith('-b'):note+=' Bare V2 source establishes the shared optical window only; optional case comes from its separate manufacturer drawing.'
  if id.endswith('lcd-1-3-c'):note+=' Prism stand intentionally remains an unassembled native-CAD reference; its separate posed footprint is not compared to this bare LCD photo.'
 source=A/'dimension-photo.jpg';im=Image.open(source).convert('RGB');canvas=Image.new('RGB',(im.width,im.height+94),'white');canvas.paste(im,(0,94));draw=ImageDraw.Draw(canvas);draw.text((12,8),id,fill='black');draw.text((12,26),'Unmodified original image pixels + actual model outlines at 1:1 pixel scale',fill='black');draw.text((12,44),'Orange: native optical/shell; cyan: hidden PCB footprint; pink: runtime display region',fill='black');draw.text((12,62),'Registration approx. +/-0.8mm; no hardware-fit or browser/WebGL acceptance.',fill='black')
 raw=gzip.decompress((A/'geometry.bin.gz').read_bytes());included=[]
 for part in m['parts']:
  n=part['name'];role=part['role']
  if role not in ['pcb','panel','shell'] or 'photo' in n:continue
  if '1-3-c' in id and role=='shell':continue
  pos=np.frombuffer(raw,dtype='<f4',count=part['vertexCount']*6,offset=part['byteOffset']).reshape(-1,6)[:,:2]
  try:pts=pos[ConvexHull(pos).vertices]
  except QhullError:continue
  uv=np.column_stack([pts,np.ones(len(pts))]) @ affine.T;uv[:,1]+=94;draw.line([tuple(x) for x in np.vstack([uv,uv[0]])],fill='#00bca6' if role=='pcb' else '#ed742f',width=2);included.append(n)
 screen=m['presentation']['screen'];w,h=screen['sizeMm'];x,y,_=screen['positionMm'];pts=np.array([[x+w/2*math.cos(a),y+h/2*math.sin(a)] for a in np.linspace(0,2*math.pi,97)]) if screen['shape']=='circle' else np.array([[x-w/2,y-h/2],[x+w/2,y-h/2],[x+w/2,y+h/2],[x-w/2,y+h/2],[x-w/2,y-h/2]])
 uv=np.column_stack([pts,np.ones(len(pts))]) @ affine.T;uv[:,1]+=94;draw.line([tuple(x) for x in uv],fill='#ef1688',width=2)
 dst=O/(id+'-photo-registration.png');canvas.save(dst);records.append({'id':id,'modelSha256':hashlib.sha256((A/'model.json').read_bytes()).hexdigest(),'sourceFile':source.name,'sourceSha256':hashlib.sha256(source.read_bytes()).hexdigest(),'imagePixels':[im.width,im.height],'pixelFromModelXY':affine.tolist(),'includedParts':included,'registrationUncertaintyMm':.8,'scope':note,'output':dst.name,'outputSha256':hashlib.sha256(dst.read_bytes()).hexdigest()});print(id,flush=True)
(O/'photo-registration-receipts.json').write_text(json.dumps({'scope':'Registered original source pixels and actual generated model outlines. The transform is disclosed rather than represented as calibrated inspection.', 'results':records},indent=2)+'\n')
