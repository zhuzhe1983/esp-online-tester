"""Use the actual renderer and production alpha-aware closest-surface picker."""
import argparse,hashlib,json,subprocess
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--session',default='cardputer-classic-keys');a=p.parse_args();ids=['m5stack-k132','m5stack-k132-v11'];receipts={id:json.loads((Path('docs/platform/evidence')/id/'keyboard-partition.json').read_text())for id in ids}
js=r'''async(page)=>{
const cdp=await page.context().newCDPSession(page);await cdp.send('Network.clearBrowserCache');await cdp.send('Network.setCacheDisabled',{cacheDisabled:true});await page.reload();await page.waitForFunction(()=>!!window.laneReview);const results=[];
for(const [id,partition]of Object.entries(RECEIPTS)){
const result=await page.evaluate(async({id,partition})=>{
 const THREE=await import('./vendor/three.module.min.js');const{pickVisibleSurface}=await import('./scene-interaction.js');await laneReview.load(id);const current=laneReview.current;const outputs=[];
 for(const[mode,appearance]of[['original',null],['gold',{shell:{color:'#d3a334',opacity:1,finish:'metal'}}],['half-clear',{shell:{color:'#8754c9',opacity:.45,finish:'plastic'}}]]){
  if(appearance)laneReview.appearance(appearance);
  for(const[view,position]of[['front',[0,0,250]],['oblique',[105,75,180]],['back',[0,0,-250]]]){
   const cameraView=laneReview.show(view,{position,span:120,target:[0,0,0],up:[0,1,0]});const camera=new THREE.OrthographicCamera(-60,60,60,-60,2,3000);camera.position.set(...position);camera.lookAt(0,0,0);camera.updateProjectionMatrix();camera.updateMatrixWorld();const ray=new THREE.Raycaster();
   const hits=partition.keys.map(key=>{const point=new THREE.Vector3(key.centerMm[0],key.centerMm[1],key.boundsMm[1][2]);const ndc=point.clone().project(camera);ray.setFromCamera(new THREE.Vector2(ndc.x,ndc.y),camera);const hit=pickVisibleSurface(ray,current.group),actual=hit?.object.userData.control||null;return{id:key.id,hitControl:actual,hitName:hit?.object.name||null,pixel:[(ndc.x+1)*400,(1-ndc.y)*400],viewerPointMm:point.toArray(),passed:view==='back'?!actual:actual===key.id};});
   outputs.push({mode,view,camera:cameraView,keys:hits,passed:hits.every(x=>x.passed)});
  }
 }
 const caps=current.model.buttons.filter(x=>x.userData.control?.startsWith('key-r'));if(caps.length!==56)throw Error(id+': key count');if(!outputs.every(x=>x.passed))throw Error(id+': closest surface failed '+JSON.stringify(outputs.flatMap(v=>v.keys.filter(k=>!k.passed).map(k=>({...k,view:v.view,mode:v.mode})))));
 return{id,checkedAt:new Date().toISOString(),capCount:caps.length,outputs,passed:true};
},{id,partition});result.browserVersion=page.context().browser().version();results.push(result);
}
await cdp.detach();return results;
}'''.replace('RECEIPTS',json.dumps(receipts))
wrapper=Path.home()/'.codex/skills/playwright/scripts/playwright_cli.sh';r=subprocess.run([str(wrapper),'-s',a.session,'run-code',js],text=True,capture_output=True)
if r.returncode:print(r.stdout+r.stderr);raise SystemExit(r.returncode)
data=json.JSONDecoder().raw_decode(r.stdout.split('### Result\n',1)[1])[0]
for item in data:
 id=item['id'];item.update(modelSha256=hashlib.sha256((Path('web/platform/assets')/id/'model.json').read_bytes()).hexdigest(),rendererSha256=hashlib.sha256(Path('web/platform/manufacturer-model.js').read_bytes()).hexdigest(),pickerSha256=hashlib.sha256(Path('web/platform/scene-interaction.js').read_bytes()).hexdigest(),scope='56 actual native key caps; production closest-surface picker in front/oblique and back occlusion, each in default/gold/half-transparent modes. No firmware claim.');(Path('docs/platform/evidence')/id/'keyboard-picking.json').write_text(json.dumps(item,ensure_ascii=False,indent=2)+'\n');print(id+':56/56 closest key hits; back occlusion; three appearances passed')
