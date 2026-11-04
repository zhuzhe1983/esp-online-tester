"""Public 3D pointer -> guest GPIO/RMT review. Caller supplies server/fixture,
opens a tracked headless session, and must close that task session afterwards.
No build, API input/press, or public default-firmware changes are performed.
"""
import argparse,json,hashlib,subprocess
from pathlib import Path
from PIL import Image
p=argparse.ArgumentParser();p.add_argument('product',choices=['m5stack-s007','m5stack-s007-pin127','m5stack-s007-pin254']);p.add_argument('--url',required=True);p.add_argument('--fixture-dir',required=True);p.add_argument('--session',default='stamp-s3-models');a=p.parse_args()
root=Path.cwd();folder=Path(a.fixture_dir);sha=lambda f:hashlib.sha256(f.read_bytes()).hexdigest();model=root/'web/platform/assets'/a.product/'model.json';files=[folder/'flasher_args.json',folder/'stamps3_fixture.bin',folder/'bootloader/bootloader.bin',folder/'partition_table/partition-table.bin'];inputHashes={str(f.relative_to(folder)):sha(f) for f in files}
code=r'''async(page)=>{
const id=PRODUCT,url=URL,files=FILES,expected=MODEL_HASH;
await page.setViewportSize({width:1300,height:1000});const cdp=await page.context().newCDPSession(page),wasmResponses=[];cdp.on('Network.responseReceived',e=>{if(new URL(e.response.url).pathname.endsWith('/esp32sim.wasm'))wasmResponses.push({requestId:e.requestId,path:new URL(e.response.url).pathname});});await cdp.send('Network.enable');await cdp.send('Network.clearBrowserCache');await cdp.send('Network.setCacheDisabled',{cacheDisabled:true});await page.goto(url);await page.waitForFunction(()=>!!window.espTester);
await page.evaluate(async(id)=>{
 window.taskOriginalFetch=window.fetch;window.taskAssets=[];window.taskInputs=[];window.taskLeds=[];
 window.fetch=async(...args)=>{const r=await taskOriginalFetch(...args),u=new URL(r.url);if(/\/(model\.json|geometry\.bin|esp32sim\.wasm)$/.test(u.pathname)){const b=await r.clone().arrayBuffer(),h=await crypto.subtle.digest('SHA-256',b);taskAssets.push({path:u.pathname,sha256:Array.from(new Uint8Array(h),v=>v.toString(16).padStart(2,'0')).join(''),bytes:b.byteLength});}return r;};
 const c=await import('./catalog.js');const scene=c.createDefaultScene(await c.loadCatalog(),id);scene.firmware=null;await espTester.configure(scene);
 const i=document.createElement('input');i.type='file';i.multiple=true;i.hidden=true;i.id='task-fixture-input';document.body.append(i);
 window.taskUnsubscribe=espTester.subscribe(e=>{if(e.type==='input')taskInputs.push(e);if(e.type==='leds')taskLeds.push(e.leds);});
},id);
await page.locator('#inspect-model').waitFor({state:'visible'});await page.locator('#task-fixture-input').setInputFiles(files);
await page.evaluate(async()=>{await espTester.loadFirmware({format:'build',files:Array.from(document.querySelector('#task-fixture-input').files)});await espTester.start();await espTester.waitFor({port:'uart0',serialIncludes:'STAMPS3_RMT 255 0 0',timeoutMs:90000});window.taskWaitLed=async rgb=>{const end=Date.now()+15000;while(Date.now()<end){const s=await espTester.observe();if(JSON.stringify(s.leds?.[0])===JSON.stringify(rgb))return s;await new Promise(r=>setTimeout(r,20));}throw Error('Guest LED update timeout');};await taskWaitLed([255,0,0]);});
const modelHash=await page.evaluate(id=>taskAssets.find(x=>x.path.endsWith('/assets/'+id+'/model.json'))?.sha256,id);if(modelHash!==expected)throw Error('Loaded model hash differs: '+modelHash);
const box=await page.locator('#scene-mount').boundingBox(),point={x:box.x+box.width*.5,y:box.y+box.height*.415};await page.mouse.move(point.x,point.y);const hover=await page.locator('#scene-mount').getAttribute('data-hover');if(hover!=='button')throw Error('Visible cap is not a button: '+hover);
await page.screenshot({path:'output/playwright/'+id+'-pointer-before.png'});
await page.evaluate(async()=>{const s=await espTester.observe();window.taskPointer={before:{guestTime:s.stats.time,leds:s.leds,serialLength:s.serialByPort.uart0.length},firmware:s.firmware};window.taskDownWait=espTester.waitFor({port:'uart0',serialIncludes:'STAMPS3_BUTTON 0',fromNow:true,timeoutMs:15000});});
try {await page.mouse.down();await page.evaluate(async()=>{await taskDownWait;await espTester.waitFor({port:'uart0',serialIncludes:'STAMPS3_RMT 0 255 0',timeoutMs:15000});const s=await taskWaitLed([0,255,0]);taskPointer.down={guestTime:s.stats.time,leds:s.leds,serial:s.serialByPort.uart0.slice(taskPointer.before.serialLength).split(/\r?\n/).filter(l=>l.startsWith('STAMPS3_'))};taskPointer.releaseStart=s.serialByPort.uart0.length;window.taskUpWait=espTester.waitFor({port:'uart0',serialIncludes:'STAMPS3_BUTTON 1',fromNow:true,timeoutMs:15000});});await page.screenshot({path:'output/playwright/'+id+'-pointer-held.png'});}finally{await page.mouse.up();}
await page.evaluate(()=>taskUpWait);const receipt=await page.evaluate(async()=>{const s=await espTester.observe();taskPointer.released={guestTime:s.stats.time,leds:s.leds,serial:s.serialByPort.uart0.slice(taskPointer.releaseStart).split(/\r?\n/).filter(l=>l.startsWith('STAMPS3_'))};await espTester.pause();taskUnsubscribe();window.fetch=taskOriginalFetch;return {...taskPointer,actualFetchedAssets:taskAssets,inputEvents:taskInputs,ledEvents:taskLeds,frame:s.frame,lastError:s.lastError};});
await page.screenshot({path:'output/playwright/'+id+'-pointer-released.png'});const wr=wasmResponses.at(-1);if(!wr)throw Error('No actual WASM response captured');const body=await cdp.send('Network.getResponseBody',{requestId:wr.requestId});const wh=await page.evaluate(async({body,base64Encoded})=>{const bytes=base64Encoded?Uint8Array.from(atob(body),x=>x.charCodeAt(0)):new TextEncoder().encode(body);return {bytes:bytes.length,sha256:Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',bytes)),x=>x.toString(16).padStart(2,'0')).join('')};},body);receipt.actualFetchedAssets.push({path:wr.path,...wh});await cdp.detach();return {productId:id,browser:page.context().browser().version(),checkedAt:new Date().toISOString(),viewport:{width:1300,height:1000},stageBox:box,pointer:point,hover,modelSha256:modelHash,...receipt};
}'''
for k,v in [('PRODUCT',a.product),('URL',a.url),('FILES',[str(f) for f in files]),('MODEL_HASH',sha(model))]:code=code.replace(k,json.dumps(v),1)
r=subprocess.run([str(Path.home()/'.codex/skills/playwright/scripts/playwright_cli.sh'),'-s',a.session,'run-code',code],capture_output=True,text=True)
if r.returncode or '### Result\n' not in r.stdout:print(r.stdout+r.stderr);raise SystemExit(1)
result=json.JSONDecoder().raw_decode(r.stdout.split('### Result\n',1)[1])[0]
assert result['before']['leds']==[[255,0,0]] and result['down']['leds']==[[0,255,0]] and result['released']['leds']==[[0,255,0]]
assert 'STAMPS3_BUTTON 0' in result['down']['serial'] and 'STAMPS3_RMT 0 255 0' in result['down']['serial'] and 'STAMPS3_BUTTON 1' in result['released']['serial']
assert result['frame'] is None and result['lastError'] is None
for artifact in result['firmware']['artifacts']:artifact['url']=None
result.update(status='pass',harness='web/platform/index.html',method='Actual mouse down/up at visible 3D cap. Fresh UART0 waiter registered before each edge. UART0 is isolated from the duplicated USB console stream. No API input/press. Model/geometry hashes are actual unmodified fetch-response clone hashes; WASM SHA is from the actual CDP response body; RGB waits for guest output.',inputHashes=inputHashes,scriptSha256=sha(Path(__file__)),scope='Internal source-built GPIO0/RMT21 fixture only, not official firmware compatibility or physical timing. Dedicated hidden file input supplies local Files without the application upload change handler.',images=[])
folder=root/'docs/platform/evidence'/a.product
for state in ['before','held','released']:
 source=root/'output/playwright'/f'{a.product}-pointer-{state}.png';dest=folder/f'pointer-{state}.webp';Image.open(source).convert('RGB').save(dest,quality=94);result['images'].append({'state':state,'file':dest.name,'sha256':sha(dest)})
(folder/'pointer-runtime.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n');print(a.product,'pointer down/up PASS',result['modelSha256'])
