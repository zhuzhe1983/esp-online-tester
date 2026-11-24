"""Metadata-only source registration; native geometry and source scale remain unchanged."""
from pathlib import Path
import json,hashlib
ROOT=Path(__file__).resolve().parents[3];A=ROOT/'web/platform/assets'
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def save(p,d):p.write_text(json.dumps(d,ensure_ascii=False,indent=2)+'\n')
for p in A.glob('waveshare-*/model-plan.json'):
 if p.parent.name not in ['waveshare-esp32-s3-touch-lcd-2-8-v1','waveshare-esp32-s3-touch-lcd-2-8-v2','waveshare-esp32-s3-lcd-2-8','waveshare-esp32-s3-touch-lcd-1-69','waveshare-esp32-s3-touch-lcd-7','waveshare-esp32-s3-lcd-7','waveshare-esp32-s3-touch-lcd-7-external-antenna','waveshare-esp32-s3-touch-lcd-1-28-b']:continue
 d=json.loads(p.read_text());pid=d['id'];colors={}
 if 'lcd-2-8'in pid:
  d['photoRegistration']['review']['pixelFromModelXY']=[[0,-639/73.06,152+33.53*639/73.06],[-441/50.54,0,344.5]]
 elif pid.endswith('1-69'):
  d['presentation']['screen']['cornerRadiusMm']=3.2;d['presentation']['screen']['basis']+=' Rounded3.2mm corner radius is a photo estimate, not manufacturer measurement.'
  d['photoRegistration']={'review':{'file':'dimension-photo.webp','pixelFromModelXY':[[0,-415/41.13,247+19.292*415/41.13],[-333/33.13,0,307.5]],'includeParts':['native-lcd-cover'],'basis':'Published41.13×33.13 frontcover registered from actual unchanged image x247..662,y141..474; native axes rotated only for comparison.'}}
 elif pid.endswith('1-28-b'):
  d['photoRegistration']={'review':{'file':'dimension-photo.webp','pixelFromModelXY':[[374/38.51,0,292.5],[0,-374/38.51,334]],'includeParts':['native-touch-cover'],'basis':'Bare-board manufacturer38.51mm cover in originalphoto. This comparison provides no evidence of the unknown metalcase.'}}
 elif 'touch-lcd-7'in pid:
  d['photoRegistration']={'review':{'file':'dimension-photo.webp','pixelFromModelXY':[[711/192.96,0,483.5],[0,-412/110.76,82+54.36*412/110.76]],'includeParts':['native-touch-cover'],'basis':'Published192.96×110.76 cover, unchangedimage x128..839,y82..494; nativecover includesflex beyond visibleface but is not scaled.'}};colors={'native-touch-cover':'#17191b'}
  d['materialBasis']+=' Native STEP pale touchcover color conflicts with blackmanufacturer photograph; that independent opticalcover material is explicitly overridden to illustrative#17191b from dimension-photo, not classified as recolorable enclosure.'
 else:
  d['photoRegistration']={'review':{'file':'dimension-photo.webp','pixelFromModelXY':[[614/165.4,0,492-1.65*614/165.4],[0,-362/97.6,277-2.126*362/97.6]],'includeParts':['native-display-carrier'],'basis':'Native165.4×97.6 carrier registered against outerframe silhouette x185..799,y96..458. Published164×97 LCD measurement has different scope; this explicit overlay does not rescale theCAD.'}};colors={'native-front-face':'#17191b'}
 for part in d['parts']:
  if part['name']in colors:part['color']=colors[part['name']];part['materialSourceIds']=['dimension-photo'];part['materialBasis']='Illustrative black opticalbezel appearance from exact manufacturerphoto; sourceCAD geometry untouched.'
 save(p,d)
 mpath=p.parent/'model.json';m=json.loads(mpath.read_text())
 for key in['dimensions','presentation','limitations','referencePhotos','method','sourceSearch','photoRegistration','sourceCoverage']:
  if key in d:m[key]=d[key]
 for part in m['parts']:
  new=next(x for x in d['parts']if x['name']==part['name'])
  for k,v in new.items():
   if k not in['geometry','matrixRows']:part[k]=v
 m['provenance']['materialBasis']=d['materialBasis'];m['artifacts']['model-plan.json']=sha(p);save(mpath,m);bc=p.parent/'build-check.json';b=json.loads(bc.read_text());b['modelSha256']=sha(mpath);save(bc,b)
print('Nativegeometry unchanged; visualregistration/material metadata refined against unchanged original references.')
