"""Headless model evidence through the task-tracked Playwright wrapper.

The caller owns opening/closing the named session and its local preview server.
Only explicit product IDs are loaded; reference photos are never generated.
"""
import argparse
import hashlib
import json
import subprocess
from pathlib import Path

parser=argparse.ArgumentParser()
parser.add_argument('ids',nargs='+')
parser.add_argument('--session',default='onboarding-lane01')
args=parser.parse_args()
for identity in args.ids:
    assert identity.startswith('m5stack-') and all(c.islower() or c.isdigit() or c=='-' for c in identity)
code=r'''async(page)=>{
await page.setViewportSize({width:1300,height:1000});
const cdp=await page.context().newCDPSession(page);
await cdp.send('Network.clearBrowserCache');await cdp.send('Network.setCacheDisabled',{cacheDisabled:true});
await page.reload();await page.waitForFunction(()=>!!window.laneReview);
await page.evaluate(()=>Object.assign(laneReview.renderer.domElement.style,{position:'fixed',left:'0px',top:'0px',width:'800px',height:'800px',zIndex:99}));
const results=[];
for(const id of IDS){
const metadata=await page.evaluate(id=>laneReview.load(id),id);
const images=[];
for(const view of metadata.referencePhotos.map(p=>p.view)){
const camera=await page.evaluate(v=>laneReview.show(v),view);if(camera.id!==id)throw Error('Review model changed concurrently');
await page.screenshot({path:'output/playwright/'+id+'-'+view+'.png',clip:{x:0,y:0,width:800,height:800}});
await page.evaluate(()=>laneReview.mask());
await page.screenshot({path:'output/playwright/'+id+'-'+view+'-mask.png',clip:{x:0,y:0,width:800,height:800}});
images.push({view,camera,mode:'original',kind:'reference'});
}
const materials=()=>{const c=laneReview.current,s=new Set(c.group.userData.shellMeshes),out=[];c.group.traverse(o=>{if(o.material&&!s.has(o))out.push([o.name,o.material.uuid,o.material.color?.getHexString(),o.material.opacity]);});return out;};
const before=await page.evaluate(materials);
for(const [mode,appearance] of [['original',null],['gold',{shell:{color:'#d3a334',opacity:1,finish:'metal'}}],['half-clear',{shell:{color:'#8754c9',opacity:.45,finish:'plastic'}}]]){
if(appearance)await page.evaluate(v=>laneReview.appearance(v),appearance);
for(const side of ['front','back']){
const camera=await page.evaluate(({side})=>laneReview.show(side+'-oblique',{position:[105,75,side==='front'?180:-180],span:Math.max(...laneReview.current.meta.presentation.bodySizeMm)*1.55}),{side});
await page.screenshot({path:'output/playwright/'+id+'-'+mode+'-'+side+'-oblique.png',clip:{x:0,y:0,width:800,height:800}});
images.push({view:side+'-oblique',camera,mode,kind:'appearance'});
}}
const after=await page.evaluate(materials);if(await page.evaluate(()=>laneReview.current.id)!==id)throw Error('Review model changed concurrently');
results.push({id,checkedAt:new Date().toISOString(),cacheClearedAndDisabled:true,browser:page.context().browser().version(),nonShellMaterialsUnchanged:JSON.stringify(before)===JSON.stringify(after),parts:metadata.parts.map(p=>({name:p.name,texture:p.texture||null})),images});
}
await cdp.detach();return results;
}'''.replace('IDS',json.dumps(args.ids))
wrapper=Path.home()/'.codex/skills/playwright/scripts/playwright_cli.sh'
result=subprocess.run([str(wrapper),'-s',args.session,'run-code',code],text=True,capture_output=True)
if result.returncode:
    print(result.stdout+result.stderr);raise SystemExit(result.returncode)
payload=json.JSONDecoder().raw_decode(result.stdout.split('### Result\n',1)[1])[0]
for record in payload:
    identity=record['id'];assets=Path('web/platform/assets')/identity
    model=(assets/'model.json').read_bytes();meta=json.loads(model)
    assert record['parts']==[{'name':p['name'],'texture':p.get('texture')}for p in meta['parts']],'Stale browser model'
    assert record['nonShellMaterialsUnchanged'],'Appearance leaked outside shell'
    record['modelSha256']=hashlib.sha256(model).hexdigest()
    record['rendererSha256']=hashlib.sha256(Path('web/platform/manufacturer-model.js').read_bytes()).hexdigest()
    for image in record['images']:
        filename=f"{identity}-{image['view']}.png" if image.get('kind')=='reference' else f"{identity}-{image['mode']}-{image['view']}.png"
        image['file']=filename;image['sha256']=hashlib.sha256((Path('output/playwright')/filename).read_bytes()).hexdigest()
    output=Path('docs/platform/evidence')/identity/'browser-review.json';output.parent.mkdir(parents=True,exist_ok=True);output.write_text(json.dumps(record,ensure_ascii=False,indent=2)+'\n')
    print(identity+': captured source views and three appearance modes; non-shell materials unchanged')
