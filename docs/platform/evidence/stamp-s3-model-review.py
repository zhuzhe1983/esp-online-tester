"""Capture/compare source-declared Stamp model views without rescaling geometry.

Run from the repository root after opening review-lane-02 in a tracked headless
Playwright session. The caller owns server/session lifetime and must close it.
Only original source pixels are used. Non-square photo crops are contained,
never stretched to a square. Dependencies for verify: requirements-model.txt.
"""
import argparse, subprocess, json, sys, hashlib
from pathlib import Path
parser=argparse.ArgumentParser()
parser.add_argument('action',choices=['capture','verify'])
parser.add_argument('product')
parser.add_argument('--session',default='stamp-s3-models')
args=parser.parse_args()
if not args.product.startswith('m5stack-s007') or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789-' for c in args.product):
    raise ValueError('Expected an explicit Stamp-S3/S3A product ID')
Path('output/playwright').mkdir(parents=True,exist_ok=True)
if args.action=="capture":
    id=args.product;code=r'''async(page)=>{
    const id=ID;await page.setViewportSize({width:1300,height:1000});const cdp=await page.context().newCDPSession(page);await cdp.send('Network.clearBrowserCache');await cdp.send('Network.setCacheDisabled',{cacheDisabled:true});await page.reload();await page.waitForFunction(()=>document.body.dataset.ready==='true');await page.evaluate(id=>laneReview.load(id),id);
    await page.evaluate(()=>Object.assign(laneReview.renderer.domElement.style,{position:'fixed',left:'0px',top:'0px',width:'800px',height:'800px',zIndex:99}));
    const metadata=await page.evaluate(()=>laneReview.current.meta),images=[];
    const take=async(view,options,mode)=>{const camera=await page.evaluate(({view,options})=>laneReview.show(view,options),{view,options});const file=id+'-'+view+(mode?'-'+mode:'')+'.png';await page.screenshot({path:'output/playwright/'+file,clip:{x:0,y:0,width:800,height:800}});images.push({file,view,mode:mode||'original',camera});};
    for(const view of ['front','back','side','back-oblique']) {await take(view,{},'');if(['front','back'].includes(view)){await page.evaluate(()=>laneReview.mask(true));await take(view,{},'mask');await page.evaluate(()=>laneReview.mask(false));}}
    const mats=()=>{const sh=new Set(laneReview.current.group.userData.shellMeshes);const out=[];laneReview.current.group.traverse(o=>{if(o.material&&!sh.has(o))out.push({name:o.name,color:o.material.color?.getHexString(),opacity:o.material.opacity,roughness:o.material.roughness,metalness:o.material.metalness});});return out;};
    const before=await page.evaluate(mats);
    for(const [mode,appearance] of [['original',null],['gold',{shell:{color:'#d3a334',opacity:1,finish:'metal'}}],['purple',{shell:{color:'#8754c9',opacity:.45,finish:'plastic'}}]]){if(appearance)await page.evaluate(v=>laneReview.appearance(v),appearance);for(const side of ['front','back'])await take(side+'-oblique',{position:[105,75,side==='front'?180:-180],span:Math.max(...metadata.presentation.bodySizeMm)*1.55},mode);}
    const after=await page.evaluate(mats);
    await page.evaluate(id=>laneReview.load(id),id);
    const inspection=await page.evaluate(()=>{const m=laneReview.current.model,ins=m.group.userData.inspection;if(!ins)return {available:false,error:m.group.userData.inspectionError||null};const positions=()=>Object.fromEntries(m.group.children.map(o=>[o.name,o.position.toArray()]));const before=positions();ins.setExpanded(true);const expanded=positions();const moved=Object.keys(before).filter(k=>JSON.stringify(before[k])!==JSON.stringify(expanded[k])).map(k=>({name:k,offsetMm:expanded[k].map((v,i)=>v-before[k][i])}));ins.setExpanded(false);const restored=JSON.stringify(before)===JSON.stringify(positions());ins.setExpanded(true);return {available:true,moved,exactRestoration:restored};});
    if(inspection.available){await take('inspection',{span:36,position:[-34,-5,36],target:[-1,0,4]},'');await page.evaluate(()=>laneReview.current.model.group.userData.inspection.setExpanded(false));}
    await cdp.detach();return {id,inspection,checkedAt:new Date().toISOString(),browser:page.context().browser().version(),modelMetadata:metadata,nonShellMaterialsUnchanged:JSON.stringify(before)===JSON.stringify(after),images};
    }'''.replace('ID',json.dumps(id),1)
    r=subprocess.run([str(Path.home()/'.codex/skills/playwright/scripts/playwright_cli.sh'),'-s',args.session,'run-code',code],text=True,capture_output=True)
    if r.returncode:print(r.stdout+r.stderr);raise SystemExit(1)
    x=json.JSONDecoder().raw_decode(r.stdout.split('### Result\n',1)[1])[0]
    (Path('output/playwright')/(id+'-browser-capture.json')).write_text(json.dumps(x,ensure_ascii=False,indent=2)+'\n');print(id,len(x['images']),x['nonShellMaterialsUnchanged'])
else:
    from PIL import Image, ImageOps, ImageDraw
    import numpy as np, cv2
    id=args.product;assets=Path('web/platform/assets')/id;e=Path('docs/platform/evidence')/id;shots=Path('output/playwright');meta=json.load(open(assets/'model.json'));r=json.load(open(e/'verification.json'));b=json.load(open(Path('output/playwright')/(id+'-browser-capture.json')))
    assert meta==b.pop('modelMetadata'),'stale browser metadata'
    sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    b.update(harness='web/platform/review-lane-02.html',modelSha256=sha(assets/'model.json'),rendererSha256=sha(Path('web/platform/manufacturer-model.js')),captureMethod='Task-owned headless Chromium through tracked Playwright CLI wrapper; no private browser data.')
    for im in b['images']:im['sha256']=sha(shots/im['file'])
    (e/'browser-review.json').write_text(json.dumps(b,ensure_ascii=False,indent=2)+'\n')
    def mask(im):
     ar=(np.asarray(im.convert('RGB')).min(2)<245).astype('uint8');cs,_=cv2.findContours(ar,cv2.RETR_EXTERNAL,cv2.CHAIN_APPROX_SIMPLE);out=np.zeros_like(ar)
     if cs:cv2.drawContours(out,[max(cs,key=cv2.contourArea)],-1,1,-1)
     return out.astype(bool)
    photos=[]
    for view in ['front','back','back-oblique']:
     p=next(x for x in meta['referencePhotos'] if x['view']==view);ref=ImageOps.pad(Image.open(assets/p['file']).convert('RGB'),(800,800),method=Image.Resampling.LANCZOS,color='white');render=Image.open(shots/(id+'-'+view+'.png')).convert('RGB');mp=shots/(id+'-'+view+'-mask.png');mim=Image.open(mp).convert('RGB') if mp.exists() else render;a,c=mask(ref),mask(mim);iou=float((a&c).sum()/(a|c).sum());quant=view in ['front','back'];diff=np.full((800,800,3),255,dtype=np.uint8);diff[a&c]=[50,70,60];diff[a&~c]=[255,85,85];diff[c&~a]=[60,145,255];panel=Image.new('RGB',(1600,440),'white');ImageDraw.Draw(panel).text((10,12),id+' '+view+' | official / model / 50% overlay / contour difference | IoU '+format(iou,'.6f')+' | '+('pass' if quant and iou>=.95 else 'fail' if quant else 'qualitative only'),fill='black')
     for col,im in enumerate([ref,render,Image.blend(ref,render,.5),Image.fromarray(diff)]):panel.paste(im.resize((400,400),Image.Resampling.LANCZOS),(col*400,40))
     path=e/(view+'-comparison.webp');panel.save(path,quality=94)
     photos.append({'view':view,'sourceId':p['sourceId'],'sourceSha256':p['sourceSha256'],'referenceCropXYXY':p.get('cropXYXY'),'renderSha256':sha(shots/(id+'-'+view+'.png')),'renderMaskSha256':sha(mp) if mp.exists() else None,'silhouetteIoU':iou,'minimum':.95 if quant else None,'status':('pass' if iou>=.95 else 'fail') if quant else 'qualitative-only','camera':meta['reviewViews'].get(view,{}),'comparisonFile':path.name,'comparisonSha256':sha(path),'method':'Original photograph/crop uniformly contained into 800×800 with white letterbox; no aspect distortion. Largest external filled contour, min RGB<245. Camera registration only, source geometry unchanged. Black render masks preserve photo alpha; geometry and texture realism require separate visual inspection.'})
    panel=Image.new('RGB',(1200,860),'white');draw=ImageDraw.Draw(panel)
    for row,side in enumerate(['front','back']):
     for col,mode in enumerate(['original','gold','purple']):
      image=Image.open(shots/(id+'-'+side+'-oblique-'+mode+'.png')).convert('RGB');panel.paste(image.resize((400,400),Image.Resampling.LANCZOS),(col*400,row*430+30));draw.text((col*400+10,row*430+10),side+' '+mode,fill='black')
    panel.save(e/'appearance-review.webp',quality=94)
    r.update(photos=photos,browser={'status':'reviewed-partial','harness':b['harness'],'browser':b['browser'],'views':['front','back','side','back-oblique'],'appearanceViews':['front','back'],'nonShellMaterialsUnchanged':b['nonShellMaterialsUnchanged'],'observations':['Exposed PCB photo stays on the native board underside, with CAD holes and border clipping. Brass standoff and screw retain their separate native mechanical geometry.','Original, gold and translucent purple views retain PCB, USB, antenna and fastener material states; source cover geometry does not rescale.','Bare S007 has optional loose headers/sticker; these are not attached in the default source configuration.','Copper plating within holes and USB/screw/standoff surface details are incomplete; shallow cover embossing is view/light dependent; the hex mark is photo-informed appearance, not a machined recess.'],'limitations':['Contour pass only verifies registered outside shape; it does not certify internal circuitry, machining accuracy or reflectance. The PCB photo is also used as texture, so its appearance comparison is not independent electronic-layout validation.','Official 24 mm length target remains failed against USB-inclusive source envelope. Firmware behavior is validated in the separate runtime evidence.']},genericRendererSha256=b['rendererSha256'],reviewScriptSha256=sha(Path('web/platform/review-lane-02.js')),buildScriptSha256=sha(Path('tools/onboard-lane-01.py')),appearanceComparisonSha256=sha(e/'appearance-review.webp'))
    (e/'verification.json').write_text(json.dumps(r,ensure_ascii=False,indent=2)+'\n');print([(x['view'],x['silhouetteIoU'],x['status']) for x in photos])
