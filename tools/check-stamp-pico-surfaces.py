#!/usr/bin/env python3
"""Check final Pico texture alpha against its actual carrier outline/holes.

Uses the model's physical projection and rendered alpha threshold. One texture
pixel allows only raster/UV-edge rounding; it is not a mechanical tolerance.
Run with the pinned model environment from docs/platform/model-reproduction.md.
"""
import argparse,json
from pathlib import Path
import cv2,numpy as np
from PIL import Image

ROOT=Path(__file__).resolve().parents[1]

def check(identity):
 if identity not in ['m5stack-c050-b','m5stack-k051','m5stack-k051-b']:raise ValueError('Unsupported product')
 folder=ROOT/'web/platform/assets'/identity;model=json.loads((folder/'model.json').read_text());parts={p['id']:p for p in model['derivedParts']};results=[]
 for label_id,carrier_id in [('printed-film','printed-film-carrier'),('pcb-back-photo','pcb-main')]:
  label=parts[label_id];carrier=parts[carrier_id]['geometry'];alpha=np.asarray(Image.open(folder/label['texture']).convert('RGBA'))[:,:,3];height,width=alpha.shape;size=label['geometry']['sizeMm'];mirror=-1 if label_id=='pcb-back-photo' else 1
  def project(ring):return np.int32([[round((mirror*x/size[0]+.5)*(width-1)),round((.5-y/size[1])*(height-1))]for x,y in ring])
  support=np.zeros((height,width),np.uint8);cv2.fillPoly(support,[project(carrier['outlineMm'])],255)
  for hole in carrier['holesMm']:cv2.fillPoly(support,[project(hole)],0)
  outside=(alpha>=128)&(cv2.dilate(support,np.ones((3,3),np.uint8))==0)
  results.append({'partId':label_id,'carrierId':carrier_id,'opaquePixels':int((alpha>=128).sum()),'outsideCarrierBeyondOneTexturePixel':int(outside.sum()),'oneTexturePixelMm':max(size[0]/width,size[1]/height)})
 return {'productId':identity,'method':'Alpha>=128 projected into the independently defined physical carrier outline minus its holes; one texture pixel allowance for raster/UV-edge rounding. Checks all opaque texture pixels, not only outer bounds.','parts':results,'passed':all(r['opaquePixels']>0 and r['outsideCarrierBeyondOneTexturePixel']==0 for r in results)}

if __name__=='__main__':
 parser=argparse.ArgumentParser();parser.add_argument('product');parser.add_argument('--output',type=Path);args=parser.parse_args();result=check(args.product);text=json.dumps(result,ensure_ascii=False,indent=2)+'\n';print(text,end='')
 if args.output:args.output.write_text(text)
 raise SystemExit(0 if result['passed'] else 1)
