#!/usr/bin/env python3
"""Compare exact CoreMatrix source photographs against production-renderer captures.

Capture via tools/fixtures/corematrix-model-browser-check.js first. Inputs are
explicit visual probes, never firmware evidence. No mesh/image registration warp
is applied here; camera poses are fixed in the public model plan.
"""
import argparse, hashlib, json
from pathlib import Path
import cv2
import numpy as np
from PIL import Image, ImageDraw
ROOT=Path(__file__).resolve().parents[1]
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def dump(path,data):path.write_text(json.dumps(data,ensure_ascii=False,indent=2)+'\n')
def save_preview(image,path):
 for scale in [1,.8,.6]:
  preview=image if scale==1 else image.resize((round(image.width*scale),round(image.height*scale)),Image.Resampling.LANCZOS)
  for quality in [90,75,60,45,30]:
   preview.save(path,quality=quality)
   if path.stat().st_size<48*1024:return
 raise ValueError('Comparison preview exceeds the evidence size limit')
def mask(im,threshold):
 binary=(np.asarray(im.convert('RGB')).min(2)<threshold).astype('uint8');contours,_=cv2.findContours(binary,cv2.RETR_EXTERNAL,cv2.CHAIN_APPROX_SIMPLE);result=np.zeros_like(binary)
 if contours:cv2.drawContours(result,[max(contours,key=cv2.contourArea)],-1,1,-1)
 return result.astype(bool)
def main():
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--renders',type=Path,default=ROOT/'output/playwright');args=parser.parse_args();assets=ROOT/'web/platform/assets/m5stack-c158';out=ROOT/'docs/platform/evidence/m5stack-c158';meta=json.loads((assets/'model.json').read_text());plan=json.loads((assets/'model-plan.json').read_text());report=json.loads((out/'verification.json').read_text());browser=json.loads((out/'browser/geometry.json').read_text())
 assert browser['modelSha256']==report['modelSha256']==sha(assets/'model.json'),'Capture/build receipt does not match final model'
 assert browser['geometrySha256']==sha(assets/'geometry.bin')
 # Every source triangle is included once, even where a native key/legend was
 # partitioned for its independent material. No implicit mesh repair or discard.
 indices=[i for p in plan['parts'] for a,b in p['sourceFaceRanges']for i in range(a,b)];assert sorted(indices)==list(range(17152))
 results=[]
 for view in ['front','back','left','right','top','bottom']:
  photo=next(p for p in meta['referencePhotos']if p['view']==view);original=assets/photo['file'];capture=args.renders/f'm5stack-c158-{view}.png';capture_mask=args.renders/f'm5stack-c158-{view}-mask.png';reference=Image.open(original).convert('RGB').resize((800,800),Image.Resampling.LANCZOS);render=Image.open(capture).convert('RGB');assert render.size==(800,800);r=mask(reference,245);m=mask(Image.open(capture_mask),128);intersection=int((r&m).sum());union=int((r|m).sum());iou=intersection/union
  diff=np.full((800,800,3),255,dtype=np.uint8);diff[r&m]=[55,65,60];diff[r&~m]=[235,75,75];diff[m&~r]=[60,140,230]
  panel=Image.new('RGB',(1600,440),'white');ImageDraw.Draw(panel).text((10,10),f'CoreMatrix {view} | official / rendered visual probe / 50% overlay / silhouette difference | IoU {iou:.4f}',fill='black')
  for i,im in enumerate([reference,render,Image.blend(reference,render,.5),Image.fromarray(diff)]):panel.paste(im.resize((400,400)),(i*400,40))
  filename=view+'-comparison.webp';save_preview(panel,out/filename)
  results.append({'view':view,'sourceId':photo['sourceId'],'sourceSha256':photo['sourceSha256'],'renderSha256':sha(capture),'renderMaskSha256':sha(capture_mask),'silhouetteIoU':iou,'minimum':.95,'status':'pass'if iou>=.95 else'fail','camera':meta['reviewViews'][view],'comparisonFile':filename,'comparisonSha256':sha(out/filename),'method':'Largest filled external contour; reference minRGB<245, production renderer black material mask minRGB<128. Fixed800×800 output and source-width camera calibration. No mesh scaling/image warping. This metric excludes LED brightness, materials, legends, internal CAD and firmware correctness.'})
 selected=['front','back','left','top','led-corners','inspection','gold-back','purple-back','gold-left','purple-back-oblique']
 dump(out/'browser'/'captures.json',{'storage':'Raw screenshots stay in the caller-supplied ignored render directory.','captures':{f'm5stack-c158-{name}.png':{'sha256':sha(args.renders/f'm5stack-c158-{name}.png'),'bytes':(args.renders/f'm5stack-c158-{name}.png').stat().st_size}for name in selected}})
 materials=Image.new('RGB',(1600,800),'white')
 for i,name in enumerate(['front','back','left','top','gold-back','purple-back','purple-back-oblique','inspection']):materials.paste(Image.open(args.renders/f'm5stack-c158-{name}.png').resize((400,400)),((i%4)*400,(i//4)*400));ImageDraw.Draw(materials).text(((i%4)*400+6,(i//4)*400+6),name,fill='black')
 save_preview(materials,out/'material-review.webp')
 report.update(photos=results,browser={'receipt':'browser/geometry.json','sha256':sha(out/'browser/geometry.json')},completeAssemblyDimensions=browser['boundsMm'],sourceTriangleCoverage={'sourceTriangles':17152,'selectedTriangles':len(indices),'duplicates':len(indices)-len(set(indices)),'omitted':0},status='partial',buildScriptSha256=sha(ROOT/'tools/onboard-lane-01.py'),reviewScriptSha256=sha(ROOT/'tools/fixtures/corematrix-model-review.js'),browserCheckSha256=sha(ROOT/'tools/fixtures/corematrix-model-browser-check.js'),verificationScriptSha256=sha(Path(__file__)),genericRendererSha256=sha(ROOT/'web/platform/manufacturer-model.js'),visualAssessment={'unknownReadable':'Public inspection separation exposes rear UNKNOWN label while complete assembly remains visible.','appearance':'Gold and transparent purple preserve original LED, connector, key and printed-sheet materials.','knownDeviations':['Renderer material response and LED optical bloom differ from photographed lighting; no calibrated photometric match.','Internal PCB/components have no manufacturing CAD; blank UNKNOWN board and approximate visible connector envelopes remain.','Rear sheet thickness/depth, screw hidden geometry, connector internal depth and pin lengths are unmeasured.','Orthographic side photographs contain small perspective, shadow and lens effects; silhouette results do not prove every visible detail.']});dump(out/'verification.json',report)
 print(json.dumps({'completeAssemblyMm':browser['boundsMm']['size'],'photoSilhouetteIoU':{p['view']:p['silhouetteIoU']for p in results},'modelSha256':report['modelSha256'],'status':'partial'},indent=2))
if __name__=='__main__':main()
