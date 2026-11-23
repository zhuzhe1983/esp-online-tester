import assert from 'node:assert/strict';
import {mkdir,writeFile,readFile} from 'node:fs/promises';
import {resolve} from 'node:path';
import {createHash} from 'node:crypto';
import {pathToFileURL} from 'node:url';
import {startStickT2Preview} from './stickt2-preview.mjs';
const {chromium}=await import(pathToFileURL(resolve(process.env.PLAYWRIGHT_MODULE||'target/stickt2-browser/node_modules/playwright/index.mjs')));
const out='target/stickt2-research/browser';await mkdir(out,{recursive:true});
const wasmSha256=createHash('sha256').update(await readFile('web/wasm/esp32sim.wasm')).digest('hex');
const preview=await startStickT2Preview({port:0,useCatalogSource:true});
const receipt={wasmSha256,factory:preview.scene.firmware,syntheticRaw14NotCelsius:true,assertions:[],pages:[],externalRequests:[]};
let browser,context;
try {
 browser=await chromium.launch({headless:true});receipt.browserVersion=browser.version();const errors=[];
 context=await browser.newContext({viewport:{width:1440,height:1000}});
 await context.route('**/*',route=>{
   const url=new URL(route.request().url());
   if(url.href===preview.scene.firmware.url)return route.fulfill({body:preview.bin,contentType:'application/octet-stream',headers:{'access-control-allow-origin':'*'}});
   if(url.origin===preview.origin||['data:','blob:'].includes(url.protocol))return route.continue();
   receipt.externalRequests.push(url.origin);return route.abort();
 });
 context.on('page',child=>child.on('pageerror',e=>errors.push(e.message)));const page=await context.newPage();
 const waitConfigured=async()=>{const end=Date.now()+30000;while(Date.now()<end){if(await page.evaluate(async()=>window.espTester&&(await window.espTester.observe()).scene?.productId==='m5stack-k016-t2'))return;await page.waitForTimeout(100);}throw Error('Scene configure timeout');};
 const waitGuest=async seconds=>{const end=Date.now()+240000;while(Date.now()<end){const s=await page.evaluate(()=>window.espTester.observe());if(s.status==='error')throw Error(JSON.stringify(s.lastError));if(Number(s.stats.time)>=seconds)return;await page.waitForTimeout(100);}throw Error('Guest clock timeout at '+seconds);};
 const hotspot=()=>page.evaluate(async()=>{const f=await window.espTester.capture({format:'rgba'});let n=0,x=0,y=0;for(let i=0;i<f.pixels.length;i+=4)if(f.pixels[i]>240&&f.pixels[i+1]<20&&f.pixels[i+2]<20){n++;x+=(i/4)%f.width;y+=Math.floor(i/4/f.width);}return {count:n,center:n?[x/n,y/n]:null};});
 await page.goto(preview.url);await waitConfigured();await page.evaluate(async()=>{await window.espTester.configureNetwork({enabled:false,autoProvision:false});await window.espTester.loadFirmware();await window.espTester.start();});
 await waitGuest(6.3);receipt.runtime=await page.evaluate(async()=>{const s=await window.espTester.observe(),c=await window.espTester.capabilities();return {executionMode:s.firmware?.executionMode,artifacts:s.firmware?.artifacts,thermal:c.thermal,guestSeconds:s.stats.time};});const before=await hotspot();assert.ok(before.count>100,'real factory hotspot appeared');
 const artifact=receipt.runtime.artifacts.find(item=>item.kind==='flash');
 assert.equal(artifact?.url,preview.scene.firmware.url);assert.equal(artifact?.sha256,preview.scene.firmware.sha256);assert.equal(artifact?.bytes,4194304);
 await page.locator('[data-thermal-field="centerX"]').fill('40');await page.locator('[data-thermal-field="centerY"]').fill('30');await page.locator('[data-thermal-apply]').click();
 const t=await page.evaluate(async()=>(await window.espTester.observe()).stats.time);await waitGuest(t+.3);const after=await hotspot();assert.ok(after.count>100);const delta=after.center.map((v,i)=>v-before.center[i]);assert.ok(Math.abs(delta[0]-30)<2&&Math.abs(delta[1]+40)<2,JSON.stringify({before,after,delta}));
 receipt.assertions.push('workbench UI changes actual unmodified factory hotspot through worker sensor protocol');receipt.pages.push({mode:'studio',before,after,delta});
 await page.screenshot({path:out+'/workbench.png'});const studioPng=await page.evaluate(()=>window.espTester.capture({format:'png'}));await writeFile(out+'/workbench-frame.png',Buffer.from(studioPng.dataUrl.split(',')[1],'base64'));const shared=await page.evaluate(()=>window.espTester.share({mode:'embed',autostart:false,includeRecording:false}));await page.evaluate(()=>window.espTester.dispose());
 await page.goto(shared);await waitConfigured();assert.equal(await page.locator('[data-thermal-field="centerX"]').inputValue(),'40');
 await page.evaluate(async()=>{await window.espTester.configureNetwork({enabled:false,autoProvision:false});await window.espTester.loadFirmware();await window.espTester.start();});await waitGuest(6.3);const mini=await hotspot();assert.ok(Math.abs(mini.center[0]-after.center[0])<2&&Math.abs(mini.center[1]-after.center[1])<2);receipt.pages.push({mode:'embed',restored:mini});receipt.assertions.push('mini restores shared sensor environment and renders its actual factory hotspot');
 await page.evaluate(async()=>{await window.espTester.input({type:'thermal-environment',background:8000,hotspot:9000,centerX:80,centerY:60,radius:12});});const mt=await page.evaluate(async()=>(await window.espTester.observe()).stats.time);await waitGuest(mt+.3);const miniMoved=await hotspot();assert.ok(Math.abs(miniMoved.center[0]-before.center[0])<2);receipt.assertions.push('mini agent input changes real guest framebuffer');
 await page.screenshot({path:out+'/mini.png'});const miniPng=await page.evaluate(()=>window.espTester.capture({format:'png'}));await writeFile(out+'/mini-frame.png',Buffer.from(miniPng.dataUrl.split(',')[1],'base64'));const serial=await page.evaluate(async()=>(await window.espTester.observe()).serial);assert.ok(!serial.includes('error=2'));receipt.pageErrors=errors;assert.deepEqual(errors,[]);
 await page.evaluate(()=>window.espTester.dispose());
 // A fresh task-owned parent exercises the actual origin/session-bound mini RPC.
 const parent=await context.newPage();await parent.goto(preview.origin+'/platform/index.html');
 await parent.waitForFunction(()=>!!window.espTester);await parent.evaluate(()=>window.espTester.dispose());
 await parent.setContent('<iframe id="subject" style="width:100%;height:900px"></iframe>');
 await parent.evaluate(url=>{const frame=document.querySelector('#subject');window.rpcCall=(method,...args)=>new Promise((resolve,reject)=>{const requestId='r'+(++window.rpcSequence),timer=setTimeout(()=>{window.removeEventListener('message',listener);reject(Error('RPC timeout '+method));},240000);const listener=event=>{const m=event.data;if(event.source!==frame.contentWindow||event.origin!==location.origin||m?.session!=='stickt2_test'||m.requestId!==requestId)return;clearTimeout(timer);window.removeEventListener('message',listener);m.error?reject(Error(JSON.stringify(m.error))):resolve(m.result);};window.addEventListener('message',listener);frame.contentWindow.postMessage({channel:'esp-workbench',version:'1',session:'stickt2_test',requestId,type:method==='hello'?'hello':'call',method,args},location.origin);});window.rpcSequence=0;frame.src=url;},shared);
 await parent.locator('iframe').contentFrame().locator('[data-thermal-apply]').waitFor();
 await parent.evaluate(()=>window.rpcCall('hello'));
 const rpc=(method,...args)=>parent.evaluate(([method,args])=>window.rpcCall(method,...args),[method,args]);
 const end=Date.now()+30000;while(!(await rpc('observe')).scene&&Date.now()<end)await parent.waitForTimeout(100);
 await rpc('configureNetwork',{enabled:false,autoProvision:false});await rpc('loadFirmware');await rpc('recordingStart');
 const rpcWait=async seconds=>{const end=Date.now()+240000;while(Date.now()<end){const state=await rpc('observe');if(state.status==='error')throw Error(JSON.stringify(state.lastError));if(Number(state.stats.time)>=seconds)return;await parent.waitForTimeout(100);}throw Error('RPC guest timeout');};
 const rpcHotspot=async()=>parent.evaluate(async()=>{const f=await window.rpcCall('capture',{format:'rgba'});let n=0,x=0,y=0;for(let i=0;i<f.pixels.length;i+=4)if(f.pixels[i]>240&&f.pixels[i+1]<20&&f.pixels[i+2]<20){n++;x+=(i/4)%f.width;y+=Math.floor(i/4/f.width);}return {count:n,center:n?[x/n,y/n]:null};});
 await rpcWait(6.3);const initial=await rpcHotspot();
 await rpc('input',{type:'thermal-environment',background:8000,hotspot:9000,centerX:80,centerY:60,radius:12});
 const rt=(await rpc('observe')).stats.time;await rpcWait(rt+.3);const recordedHotspot=await rpcHotspot();const stopped=await rpc('recordingStop');
 assert.equal(stopped.events.length,1);assert.equal(stopped.events[0].type,'thermal-environment');
 const recordingUrl=await rpc('share',{mode:'embed',autostart:false});assert.ok(new URLSearchParams(new URL(recordingUrl).hash.slice(1)).get('recording'));
 const replay=await rpc('recordingPlay',stopped);assert.equal(replay.result,'completed');const replayHotspot=await rpcHotspot();
 assert.ok(Math.abs(replayHotspot.center[0]-recordedHotspot.center[0])<2&&Math.abs(replayHotspot.center[1]-recordedHotspot.center[1])<2);
 const replayPng=await rpc('capture',{format:'png'});await writeFile(out+'/replay-frame.png',Buffer.from(replayPng.dataUrl.split(',')[1],'base64'));
 receipt.recording={initial,recordedHotspot,replayHotspot,events:stopped.events,replay};receipt.assertions.push('origin/session-bound iframe RPC records, shares and replays actual sensor input into guest framebuffer');
 await parent.close();
 // Follow the recording-bearing URL and use its real restored UI, not the in-memory document.
 await page.goto('about:blank');await page.goto(recordingUrl);await waitConfigured();assert.equal(await page.locator('[data-thermal-field="centerX"]').inputValue(),'40');
 await page.evaluate(async()=>{await window.espTester.configureNetwork({enabled:false,autoProvision:false});await window.espTester.loadFirmware();});
 await page.locator('.session-tool-row').getByRole('button',{name:/Play back|回放/}).click();
 const sharedEnd=Date.now()+240000;let sharedReplay;
 while(Date.now()<sharedEnd){const state=await page.evaluate(()=>window.espTester.recordingObserve());if(state.status==='error')throw Error(JSON.stringify(state.error));if(state.receipt?.result==='completed'){sharedReplay=state.receipt;break;}await page.waitForTimeout(100);}
 assert.ok(sharedReplay,'recording URL restored UI replay completes');const sharedHotspot=await hotspot();assert.ok(Math.abs(sharedHotspot.center[0]-before.center[0])<2&&Math.abs(sharedHotspot.center[1]-before.center[1])<2);
 receipt.sharedRecording={replay:sharedReplay,hotspot:sharedHotspot};receipt.assertions.push('recording-bearing shared URL restores its initial environment and UI playback renders the recorded guest hotspot');
 await page.evaluate(()=>window.espTester.dispose());assert.deepEqual(errors,[]);assert.deepEqual(receipt.externalRequests,[]);assert.equal(receipt.runtime.executionMode,'wasm-jit');receipt.passed=true;
} catch(e){receipt.passed=false;receipt.error={message:e.message,stack:e.stack};throw e;}
finally {try{await context?.close();}finally{try{await browser?.close();}finally{await preview.close();await writeFile(out+'/receipt.json',JSON.stringify(receipt,null,2)+'\n');}}}
