#!/usr/bin/env python3
"""Compare exact-SKU reference silhouettes with actual tracked-browser captures.

No automatic pose fitting or geometry deformation. Cameras are declared in model-plan.json;
white studio-background masks are a coarse silhouette check, not internal CAD acceptance.
"""
import argparse,hashlib,json
from pathlib import Path
import numpy as np
from PIL import Image
from scipy.ndimage import binary_fill_holes,binary_closing,label

ROOT=Path(__file__).resolve().parents[1]
ID='m5stack-k159-b'
VIEWS={'front':('daf0a58e49',[480,235,1120,1365]),'back':('4bcd22006c',[480,235,1120,1360]),'ports':('fdb6861a7c',[175,735,1415,870]),'side':('c3df0b6aff',[145,760,1455,840])}
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def silhouette(image,roi=None):
 pixels=np.asarray(image.convert('RGB'));mask=pixels.min(axis=2)<247
 if roi:
  region=np.zeros(mask.shape,dtype=bool);x0,y0,x1,y1=roi;region[y0:y1,x0:x1]=True;mask &= region
 mask=binary_closing(mask,iterations=2);labels,count=label(mask)
 if not count:raise ValueError('Empty reference mask')
 sizes=np.bincount(labels.ravel());sizes[0]=0;return binary_fill_holes(labels==sizes.argmax())
def main():
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--screenshots',type=Path,required=True);parser.add_argument('--output',type=Path,required=True);args=parser.parse_args();args.output.mkdir(parents=True,exist_ok=True)
 assets=ROOT/'web/platform/assets'/ID;metadata=json.loads((assets/'model.json').read_text());results=[]
 for view,(photo,roi) in VIEWS.items():
  source=assets/f'reference-product-photo-{photo}.webp';capture=args.screenshots/f'paperdiy-final-original-{view}.png';ref=Image.open(source);render=Image.open(capture).convert('RGB');referenceMask=Image.fromarray(silhouette(ref,roi).astype('uint8')*255).resize(render.size,Image.Resampling.NEAREST);a=np.asarray(referenceMask)>0;b=silhouette(render)
  intersection=int(np.count_nonzero(a&b));union=int(np.count_nonzero(a|b));intersectionOverUnion=intersection/union
  aligned=ref.resize(render.size,Image.Resampling.LANCZOS).convert('RGB');comparison=Image.new('RGB',(render.width*2,render.height),'white');comparison.paste(aligned);comparison.paste(render,(render.width,0));comparison.save(args.output/f'{view}-comparison.webp',lossless=True)
  # Cyan: reference silhouette, orange: rendered silhouette; overlap darkens.
  overlay=np.full((*a.shape,3),255,dtype=np.uint8);overlay[a]=[58,180,200];overlay[b]=[245,145,45];overlay[a&b]=[60,82,80];Image.fromarray(overlay).save(args.output/f'{view}-silhouette.png')
  results.append(dict(view=view,sourceSha256=sha(source),captureSha256=sha(capture),referenceRoiPx=roi,camera=metadata['reviewViews'][view],pixels=list(render.size),intersectionPixels=intersection,unionPixels=union,silhouetteIoU=round(intersectionOverUnion,6),scope='Independent photo silhouette only; no verification of internal geometry, colorimetry or hardware tolerances.'))
 result=dict(productId=ID,modelSha256=sha(assets/'model.json'),method='Original manufacturer photo pixels, declared manual ROI, min(RGB)<247, binary closing 2px, largest component and hole fill. Reference mask resized nearest-neighbor to actual browser raster. No learned segmentation or automatic alignment.',results=results,limitations=['Near-frontal studio photos retain lens/perspective and soft-edge uncertainty.','Ports and side are only a few dozen pixels tall after downsampling; anti-aliasing/lighting affect IoU more strongly.','This check does not certify exact internal layout or optical materials.'])
 (args.output/'photo-comparison.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n');print(json.dumps({r['view']:r['silhouetteIoU'] for r in results}))
if __name__=='__main__':main()
