#!/usr/bin/env python3
"""Reproduce the four ATOM paper masks from pinned photos and socket geometry.

The model plans retain explicit original-photo exclusion rectangles, so ordinary
rebuild-product-model.py also reproduces the corrected textures offline. This
helper only derives those rectangles and checks that source RGB is untouched.
"""
import argparse,copy,hashlib,json,math
from pathlib import Path
from PIL import Image,ImageDraw
ROOT=Path(__file__).resolve().parents[1]
PRODUCTS=('m5stack-c126','m5stack-c126-echo','m5stack-c008-c','m5stack-c008-b')
NOTE=' Exclude the two source-photo regions occupied by the separately modeled rear female-header housings, including 0.05 mm of display clearance. These rectangles are projected from this model\'s existing photo-derived header frame geometry; they avoid paper/header overlap without moving the paper, stretching its RGB image, filling a shell opening or claiming calibrated hardware dimensions.'
def sha(data):return hashlib.sha256(data).hexdigest()
def dump(data):return json.dumps(data,ensure_ascii=False,indent=2)+'\n'
def mask_image(image,spec):
 factor=4;alpha=Image.new('L',(image.width*factor,image.height*factor),0);painter=ImageDraw.Draw(alpha)
 for poly in spec.get('includePolygons',[]):painter.polygon([(round(x*factor),round(y*factor))for x,y in poly],fill=255)
 for rect in spec.get('includeRoundedRects',[]):painter.rounded_rectangle([round(v*factor)for v in rect['xyxy']],radius=rect.get('radiusPx',0)*factor,fill=255)
 for poly in spec.get('excludePolygons',[]):painter.polygon([(round(x*factor),round(y*factor))for x,y in poly],fill=0)
 for rect in spec.get('excludeRects',[]):painter.rectangle([round(v*factor)for v in rect],fill=0)
 for ellipse in spec.get('excludeEllipses',[]):painter.ellipse([round(v*factor)for v in ellipse],fill=0)
 result=image.convert('RGBA');result.putalpha(alpha.resize(image.size,Image.Resampling.LANCZOS));return result

def update(product,write):
 assets=ROOT/'web/platform/assets'/product;metadata=json.loads((assets/'model.json').read_text());plan=json.loads((assets/'model-plan.json').read_text());label=next(d for d in plan['derivedParts']if d['kind']=='label');texture=next(t for t in plan['textureSources']if t['file']==label['texture']);original=Image.open(assets/texture['file']).convert('RGBA')
 assert label['geometry']['rotation']==[0,math.pi,0] and label['geometry']['centerMm'][:2]==[0,0]
 assert not texture.get('perspectiveQuad'),'This source uses a fixed crop, not a guessed perspective'
 x0,y0,x1,y1=texture['cropXYXY'];width,height=label['geometry']['sizeMm'];rectangles=[]
 for side in ('left','right'):
  frames=[d for d in plan['derivedParts']if d['id'].startswith('header-photo-'+side+'-') and d['kind']=='box']
  assert frames,'Missing explicitly modeled header frame'
  lowx=min(d['geometry']['centerMm'][0]-d['geometry']['sizeMm'][0]/2 for d in frames)-.05;highx=max(d['geometry']['centerMm'][0]+d['geometry']['sizeMm'][0]/2 for d in frames)+.05
  lowy=min(d['geometry']['centerMm'][1]-d['geometry']['sizeMm'][1]/2 for d in frames)-.05;highy=max(d['geometry']['centerMm'][1]+d['geometry']['sizeMm'][1]/2 for d in frames)+.05
  # Rear plane rotates pi about Y: local paper +X is assembled -X.
  rectangle=[x0+(.5-highx/width)*(x1-x0),y0+(.5-highy/height)*(y1-y0),x0+(.5-lowx/width)*(x1-x0),y0+(.5-lowy/height)*(y1-y0)]
  rectangles.append([round(v,6)for v in rectangle])
 exclusions=texture['alphaMask'].setdefault('excludeRects',[])
 for rect in rectangles:
  if rect not in exclusions:exclusions.append(rect)
 if NOTE not in texture.get('notes',''):texture['notes']=texture.get('notes','')+NOTE
 source=next(p for p in metadata['referencePhotos']if p['sourceId']==texture['sourceId']);source_bytes=(assets/source['file']).read_bytes();assert sha(source_bytes)==source['sourceSha256'],'Pinned source changed'
 image=mask_image(Image.open(assets/source['file']).convert('RGB'),texture['alphaMask']).crop(texture['cropXYXY']);assert image.size==original.size
 assert image.convert('RGB').tobytes()==original.convert('RGB').tobytes(),'Mask-only fix must preserve every RGB pixel'
 for old,new in zip(original.getchannel('A').tobytes(),image.getchannel('A').tobytes()):assert new<128 or old>=128,'Mask fix cannot add visible photo pixels'
 if write:
  image.save(assets/texture['file'],lossless=True)
  record=next(t for t in metadata['textures']if t['file']==texture['file']);record.update(copy.deepcopy(texture));metadata['artifacts'][texture['file']]=sha((assets/texture['file']).read_bytes())
  (assets/'model-plan.json').write_text(dump(plan));(assets/'model.json').write_text(dump(metadata))
 else:
  assert image.tobytes()==original.tobytes(),'Committed texture does not match the corrected source mask'
  assert next(t for t in metadata['textures']if t['file']==texture['file'])['alphaMask']==texture['alphaMask'],'Metadata mask differs from model plan'
 print(json.dumps({'product':product,'texture':texture['file'],'sourceSha256':source['sourceSha256'],'excludedSourceRectangles':rectangles,'rgbPixelsUnchanged':True,'geometryUnchanged':True,'mode':'write'if write else'check'}))
if __name__=='__main__':
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--write',action='store_true');args=parser.parse_args()
 for product in PRODUCTS:update(product,args.write)
