#!/usr/bin/env python3
"""Contact sheets: actual CPU model views alongside retained official photographs."""
import argparse,json,pathlib,hashlib,math
from PIL import Image,ImageDraw,ImageFont,ImageOps
p=argparse.ArgumentParser();p.add_argument('--repo',required=True);p.add_argument('--renders',required=True);p.add_argument('--out',required=True);p.add_argument('--rows',type=int,default=4);p.add_argument('--cell',type=int,default=320);p.add_argument('--format',choices=['png','jpg'],default='jpg');p.add_argument('--photo-order');a=p.parse_args()
r=pathlib.Path(a.renders);repo=pathlib.Path(a.repo);out=pathlib.Path(a.out);out.mkdir(parents=True,exist_ok=True);receipt=json.load(open(r/'render-receipt.json'));products=receipt['products'];font=ImageFont.truetype('/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf',15);small=ImageFont.truetype('/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf',12);large=ImageFont.truetype('/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf',20)
photo_order=json.load(open(a.photo_order)) if a.photo_order else {};C=a.cell;H=C+62;banner=78
pages=[]
def photo_refs(refs,identity):
 ordered=photo_order.get(identity,[])
 refs=[x for x in refs if x.get('exists')];selected=[next(x for x in refs if x.get('sourceId')==key or pathlib.Path(x['file']).name==key)for key in ordered]
 for direction in ['front','back','side']:
  candidate=next((x for x in refs if direction in x.get('view','') and x not in selected),None)
  if candidate and len(selected)<3:selected.append(candidate)
 for x in refs:
  if len(selected)>=3:break
  if x not in selected:selected.append(x)
 return selected[:3]
def tile(canvas,file,x,y,caption):
 try:
  im=Image.open(file).convert('RGBA');im.thumbnail((C-16,C-16));base=Image.new('RGBA',im.size,'white');base.alpha_composite(im);canvas.paste(base.convert('RGB'),(x+(C-im.width)//2,y+(C-im.height)//2));ImageDraw.Draw(canvas).text((x+8,y+C+2),caption[:50],font=small,fill='#1a2432')
 except Exception as e:ImageDraw.Draw(canvas).text((x+8,y+30),'Missing image: '+str(e)[:25],font=small,fill='red')
for first in range(0,len(products),a.rows):
 chunk=products[first:first+a.rows];im=Image.new('RGB',(C*6,len(chunk)*H+banner),'#edf0f4');d=ImageDraw.Draw(im);d.text((12,8),'M5Stack actual 3D CPU QA | '+chunk[0].get('appearance','original')+' | '+str(first+1)+'-'+str(first+len(chunk)),font=large,fill='#12253d');d.text((12,37),'Production Three meshes / UV / alpha -> Cycles CPU. Black screen is blank, not firmware.',font=font,fill='#34465c');d.text((12,58),'Left: front/back/oblique. Right: separate retained manufacturer photos; no pose-calibrated overlay.',font=small,fill='#34465c')
 for row,item in enumerate(chunk):
  y=banner+row*H;d.rectangle((0,y,C*6,y+27),fill='#d4dde8');d.text((8,y+5),item['id']+' | '+item['name'][:75],font=font,fill='#10243c')
  for col,v in enumerate(item['views'][:3]):tile(im,r/v['file'],col*C,y+28,'CPU '+v['view'])
  for col,ref in enumerate(photo_refs(item['references'],item['id'])):tile(im,repo/ref['file'],(col+3)*C,y+28,'Official '+ref.get('view',ref.get('sourceId','photo')))
  for col in range(1,6):d.line((col*C,y+28,col*C,y+H),fill='#c0c9d4')
 file=out/('contact-%02d.%s'%(len(pages)+1,a.format));im.save(file,**({'quality':91,'subsampling':0}if a.format=='jpg' else {}));pages.append({'file':file.name,'sha256':hashlib.sha256(file.read_bytes()).hexdigest(),'products':[x['id']for x in chunk],'selectedPhotoFiles':{x['id']:[p['file']for p in photo_refs(x['references'],x['id'])]for x in chunk}})
(out/'contact-receipt.json').write_text(json.dumps({'scope':receipt['scope'],'photos':'Copyright belongs to the manufacturer; source IDs, original files and hashes remain in render-receipt.json. Reference photographs remain distinct from model pixels.','photoOrderSha256':hashlib.sha256(pathlib.Path(a.photo_order).read_bytes()).hexdigest()if a.photo_order else None,'pages':pages},indent=2)+'\n');print(json.dumps({'products':len(products),'pages':len(pages),'out':str(out)}))
