#!/usr/bin/env python3
"""Check real C5 UART capture differences against the native emitting-face projection.

Input is the JSON result of check-stamp-c5-led-browser.js. Run with the pinned
model Python environment (Pillow), --receipt and --capture-dir. No image edits.
"""
import argparse,hashlib,json,math
from pathlib import Path
from PIL import Image

def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def check(result,captures,root):
 sku=result['productId'];off=['ready-off','off','wrong-g27','reset-off','final-off'];on=['on','before-reset-on','after-reset-on'];names=off+on
 images={name:Image.open(captures/f'{sku}-uart-led-{name}.png').convert('RGBA')for name in names};size=images[off[0]].size
 assert all(image.size==size for image in images.values())
 assert all(images[name].tobytes()==images[off[0]].tobytes()for name in off),'Off/wrong-pin/reset differ from initial off'
 assert all(images[name].tobytes()==images[on[0]].tobytes()for name in on),'Repeated real guest on states differ'
 a=list(zip(*[iter(images[off[0]].tobytes())]*4));b=list(zip(*[iter(images[on[0]].tobytes())]*4));changed=[i for i,(x,y)in enumerate(zip(a,b))if x!=y];assert len(changed)>4,'No visible LED response'
 bbox=[min(i%size[0]for i in changed),min(i//size[0]for i in changed),max(i%size[0]for i in changed)+1,max(i//size[0]for i in changed)+1]
 p=result['projection'];scale=[size[i]/p['canvasCssSize'][i]for i in range(2)];points=[[xy[i]*scale[i]for i in range(2)]for xy in p['emittingFaceCanvasVertices']]
 expected=[min(v[0]for v in points),min(v[1]for v in points),max(v[0]for v in points),max(v[1]for v in points)]
 assert bbox[0]>=math.floor(expected[0])-2 and bbox[1]>=math.floor(expected[1])-2 and bbox[2]<=math.ceil(expected[2])+2 and bbox[3]<=math.ceil(expected[3])+2,(bbox,expected)
 blue=sum(b[i][2]>max(b[i][0],b[i][1])+40 and b[i][2]>100 for i in changed);assert blue>=len(changed)*.5,'Changed surface is not predominantly blue'
 for stage in result['stages']:
  assert stage['status']=='running'and not stage['lastError'];assert stage['leds']==([[0,0,255]]if stage['name']in on else[[0,0,0]])
 assets=root/'web/platform/assets'/sku
 return {'productId':sku,'modelSha256':sha(assets/'model.json'),'geometrySha256':sha(assets/'geometry.bin'),'firmwareSha256':result['firmwareSha256'],'captureSizePx':size,'changedPixels':len(changed),'blueDominantChangedPixels':blue,'changedBoundsPx':bbox,'nativeD1ProjectionBoundsPx':expected,'projectionEdgeAllowancePx':2,'allOffStatesByteIdentical':True,'allOnStatesByteIdentical':True,'capturedPngSha256':{name:sha(captures/f'{sku}-uart-led-{name}.png')for name in names},'scope':'Guest UART commands and reset drive runtime index0; all changed pixels lie on the source-identified native D1 emitting surface. Two-pixel allowance covers projection rounding/antialiasing; light output is not photometrically calibrated.'}

def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--receipt',type=Path,required=True);p.add_argument('--capture-dir',type=Path,required=True);p.add_argument('--root',type=Path,default=Path(__file__).resolve().parents[1]);p.add_argument('--output',type=Path,required=True);args=p.parse_args();receipt=json.loads(args.receipt.read_text());assert receipt['pageErrors']==[]
 results=[check(r,args.capture_dir,args.root)for r in receipt['results']];assert {r['productId']for r in results}=={'m5stack-s016','m5stack-s016-dip'}
 args.output.write_text(json.dumps({'passed':True,'products':results},indent=2)+'\n');print(json.dumps({'passed':True,'products':[{'productId':r['productId'],'changedPixels':r['changedPixels']}for r in results]}))
if __name__=='__main__':main()
