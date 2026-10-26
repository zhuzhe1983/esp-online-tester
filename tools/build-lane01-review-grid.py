"""Retain compact evidence from actual captured default/gold/transparent views."""
import argparse,hashlib,json
from pathlib import Path
from PIL import Image,ImageDraw
p=argparse.ArgumentParser();p.add_argument('ids',nargs='+');a=p.parse_args()
for id in a.ids:
 root=Path('docs/platform/evidence')/id;record=root/'browser-review.json';b=json.loads(record.read_text());canvas=Image.new('RGB',(1050,770),'white');draw=ImageDraw.Draw(canvas)
 for c,(mode,label)in enumerate([('original','Original'),('gold','Gold'),('half-clear','Half-transparent purple')]):
  draw.text((c*350+8,8),label,fill='black')
  for r,side in enumerate(['front','back']):
   file=Path('output/playwright')/f'{id}-{mode}-{side}-oblique.png';data=file.read_bytes();item=next(i for i in b['images']if i['file']==file.name);assert hashlib.sha256(data).hexdigest()==item['sha256'],f'{id}: stale screenshot'
   im=Image.open(file).convert('RGB').resize((350,350),Image.Resampling.LANCZOS);canvas.paste(im,(350*c,30+360*r));draw.text((c*350+8,30+360*r),side,fill='black')
 draw.text((8,752),id+' | native manufacturer geometry; partial; UNKNOWN internals not certified',fill='black');output=root/'appearance-review.webp';canvas.save(output,quality=91);b['retainedAppearanceEvidence']={'file':output.name,'sha256':hashlib.sha256(output.read_bytes()).hexdigest(),'source':'Six actual tracked headless-browser screenshots listed above; resized only for compact review.'};record.write_text(json.dumps(b,ensure_ascii=False,indent=2)+'\n');print(id+' appearance evidence retained')
