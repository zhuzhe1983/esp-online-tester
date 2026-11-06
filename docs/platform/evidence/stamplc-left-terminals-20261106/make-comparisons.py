#!/usr/bin/env python3
"""Make small evidence panels from independently retained before/after captures.

No perspective warp, geometry retouch or image generation is used. The two
capture directories contain left.png, left-front.png and left-back.png.
An optional official-photo comparison is written only to ignored output/.
"""
import argparse,hashlib,json
from pathlib import Path
from PIL import Image,ImageDraw

HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[3]
PHOTO_URL='https://m5stack-doc.oss-cn-shenzhen.aliyuncs.com/1129/K141_12.webp'
PHOTO_SHA='e47b69f889acb70bc8b312ff781eb25d5c05a09a8334868c7b03a1fcc00fd7f3'
def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def panel(images,labels,size):
 width,height=size;out=Image.new('RGB',(width*len(images),height+25),'white');draw=ImageDraw.Draw(out)
 for i,(image,label)in enumerate(zip(images,labels)):
  image=image.convert('RGB');image.thumbnail(size,Image.Resampling.LANCZOS)
  out.paste(image,(i*width+(width-image.width)//2,25+(height-image.height)//2));draw.text((i*width+12,7),label,fill='black')
 return out
def main():
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--before',type=Path,required=True);parser.add_argument('--after',type=Path,required=True);parser.add_argument('--official-photo',type=Path);parser.add_argument('--full-left',type=Path);args=parser.parse_args()
 if bool(args.official_photo)!=bool(args.full_left):parser.error('--official-photo and --full-left must be supplied together')
 if args.official_photo and digest(args.official_photo)!=PHOTO_SHA:parser.error('Official photo SHA-256 does not match the retained reference')
 receipt={'method':'Model-render captures with uniform downsampling and white panel padding; qualitative comparison, no photo-calibrated metrology. No standalone manufacturer photo is embedded in the published panels.','images':[],'externalReference':{'url':PHOTO_URL,'sha256':PHOTO_SHA,'usage':'Optional local-only photo comparison; source download and comparison panel stay in ignored output/.'}}
 for view in ['left','left-front','left-back']:
  before=args.before/(view+'.png');after=args.after/(view+'.png');dest=HERE/(view+'.webp');panel([Image.open(before),Image.open(after)],['Before: original offset','After: CAD cut-out aligned'],(380,380)).save(dest,quality=82,method=6)
  receipt['images'].append({'file':dest.name,'sha256':digest(dest),'beforePngSha256':digest(before),'afterPngSha256':digest(after),'bytes':dest.stat().st_size})
 if args.official_photo:
  dest=ROOT/'output/playwright/stamplc-left/official-left.webp';dest.parent.mkdir(parents=True,exist_ok=True)
  panel([Image.open(args.official_photo),Image.open(args.full_left)],['Official K141_12 photo','Current model: orthographic left'],(400,400)).save(dest,quality=85,method=6)
  receipt['externalReference']['localComparison']={'ignoredPath':dest.relative_to(ROOT).as_posix(),'sha256':digest(dest),'renderPngSha256':digest(args.full_left),'bytes':dest.stat().st_size}
 assert all(image['bytes']<50*1024 for image in receipt['images'])
 (HERE/'images.json').write_text(json.dumps(receipt,indent=2)+'\n')
if __name__=='__main__':main()
