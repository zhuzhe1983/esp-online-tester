import assert from 'node:assert/strict';
import {readFile,writeFile} from 'node:fs/promises';
import {resolve,relative,dirname} from 'node:path';
import {pathToFileURL,fileURLToPath} from 'node:url';
import {Worker} from 'node:worker_threads';
import {createHash} from 'node:crypto';
const arg=name=>{const i=process.argv.indexOf(name);return i<0?null:process.argv[i+1];};
for(const name of ['--root','--original','--v11','--out'])assert(arg(name),`Missing ${name}`);
const root=resolve(arg('--root')),out=resolve(arg('--out')),origin='https://stampfly-final-regression.invalid';
const hash=b=>createHash('sha256').update(b).digest('hex'), sourceHashes={}, fetchHashes={};
const rootFile=p=>resolve(root,p);
const modules=new Set();
async function inspectModule(p){p=resolve(p);if(modules.has(p))return;modules.add(p);const b=await readFile(p);sourceHashes[relative(root,p)]=hash(b);const s=b.toString();for(const m of s.matchAll(/(?:\bfrom\s*|\bimport\s*\(?\s*)['"](\.[^'"]+)['"]/g)){const target=resolve(dirname(p),m[1]);if(/\.(?:js|mjs)$/.test(target))await inspectModule(target);}}
for(const p of ['web/platform/runtime.js','web/platform/device-storage.js','web/platform/catalog.js','web/platform/local-firmware.js','web/wasm/worker.js'])await inspectModule(rootFile(p));
for(const p of ['tools/probe-stampfly.mjs','web/wasm/esp32sim.wasm','web/wasm/fw/esp32s3_rev0_rom.elf'])sourceHashes[p]=hash(await readFile(rootFile(p)));
const expectedWasm='4d8d85eb406e4c3641e5166ac1318d5d7b3ec5f4b7e5230ed5c8c406e7f762d6';
assert.equal(sourceHashes['web/wasm/esp32sim.wasm'],expectedWasm);assert.equal((await readFile(rootFile('web/wasm/esp32sim.wasm'))).length,5006250);
const {createPlatformRuntime}=await import(pathToFileURL(rootFile('web/platform/runtime.js')));
const {createDeviceStorage}=await import(pathToFileURL(rootFile('web/platform/device-storage.js')));
const {createDefaultScene}=await import(pathToFileURL(rootFile('web/platform/catalog.js')));
const catalog={};for(const key of ['products','accessories','firmwares']){const p=`web/platform/catalog/${key}.json`,bytes=await readFile(rootFile(p));catalog[key]=JSON.parse(bytes).items;sourceHashes[p]=hash(bytes);}
const inputs=[
 {productId:'m5stack-k138',board:'stampfly',version:'v1.1.0',url:'https://m5burner-cdn.m5stack.com/firmware/f62f9d3a62b18d530e8a5a1281312aac.bin',sha256:'0138cc333d05790f8804fabae223f4c7eb40d162ad1d23d8c8daead6412d4359',bytes:861424,path:arg('--original')},
 {productId:'m5stack-k138-v11',board:'stampfly-v11',version:'v1.0.0',url:'https://raw.githubusercontent.com/m5stack/M5StampFly/2b96752c72c465c63b260996046c71c7675f81b2/firmware/stampfly_firmware_v1.0.0_20240802.bin',sha256:'d903acc1815cb8b08224aa4e9a581bb23b69b606813f9edbf4af5002efda113b',bytes:861360,path:arg('--v11')}
];
const firmwareBytes=new Map();for(const f of inputs){const bytes=await readFile(f.path);assert.equal(bytes.length,f.bytes);assert.equal(hash(bytes),f.sha256);firmwareBytes.set(f.url,bytes);}
const fetchImpl=async value=>{const url=new URL(value),cached=firmwareBytes.get(url.href);if(cached)return new Response(cached);assert.equal(url.origin,origin);try{const p='web'+url.pathname,b=await readFile(rootFile(p));fetchHashes[p]=hash(b);return new Response(b);}catch{return new Response('',{status:404});}};
let workerCreations=0,workerTerminations=0;
const workerFactory=url=>{workerCreations++;const real=pathToFileURL(rootFile('web'+url.pathname));assert(modules.has(fileURLToPath(real)));const thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(real.href)});parentPort.on('message',data=>globalThis.onmessage({data}));`)));const adapter={postMessage:(message,...args)=>thread.postMessage(message,...args),terminate:()=>{workerTerminations++;return thread.terminate();}};thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',e=>adapter.onerror?.(e));return adapter;};
const receipt={schemaVersion:1,checkedAt:new Date().toISOString(),command:'node <temporary-harness>/factory-probe.mjs --root <integration-root> --original <exact-original-factory.bin> --v11 <exact-v11-factory.bin> --out <receipt.json>',backend:'actual-platform-runtime-and-worker-WASM-under-Node',nodeVersion:process.version,jit:true,wasmSha256:expectedWasm,wasmBytes:5006250,sourceHashes,fetchHashes,harnessSha256:hash(await readFile(fileURLToPath(import.meta.url))),products:[],limitations:['Correctness-only run; no host CPU performance measurements.','Local files with exact pinned public source hashes supply factory payloads to the real firmware loader; this run does not revalidate browser CORS or public download availability.','No browser or physical pointer action in this regression; prior Round1 browser evidence is separate.','Internal protocol fixture results are separate from these unmodified official factory images.','No flight, controller pairing, RF exchange, thrust/RPM calibration, collision, or physical sensor timing is claimed.','Stationary ideal IMU, no-target optical environment and fixed supply are the board model inputs.']};
const markers=['Finish sensor init!','ESP-NOW Ready.','Finish StampFly init!','Enjoy Flight!'];
const count=(s,m)=>s.split(m).length-1,usb=s=>s.serialByPort?.usb||'';
const lines=s=>s.split(/\r?\n/).filter(l=>/Finish sensor init!|ESP-NOW Ready\.|Finish StampFly init!|Enjoy Flight!|RTC_SW_CPU_RST|POWERON|#INIT Status:|#APP_STATUS:|Guru Meditation|abort\(\)/.test(l)).map(l=>l.replace(/(?:[0-9a-f]{2}:){5}[0-9a-f]{2}/gi,'<emulated-mac>'));
const delay=ms=>new Promise(r=>setTimeout(r,ms));
let runtime,lastState;
async function wait(label,predicate){const until=Date.now()+180000;while(Date.now()<until){lastState=await runtime.observe();if(lastState.status==='error')throw Error(`Runtime error in ${label}: ${JSON.stringify(lastState.lastError)}`);if(predicate(lastState))return lastState;await delay(20);}throw Error(`Timed out: ${label}`);}
function motorCheck(s){assert.equal(s.flight,null);assert.equal(s.scene.flight,undefined);assert.equal(s.frame,null);assert.deepEqual(s.motors.map(x=>[x.id,x.gpio]),[['front-left',5],['front-right',42],['rear-left',10],['rear-right',41]]);assert(s.motors.every(x=>x.duty===0&&Math.abs(x.frequencyHz-150000)<1500));}
try{
 for(const f of inputs){
  const {path,...identity}=f,record={...identity,checks:[]};receipt.products.push(record);
  runtime=createPlatformRuntime({catalog,fetchImpl,workerFactory,baseUrl:origin+'/platform/',assetBaseUrl:origin+'/',deviceStorage:createDeviceStorage({indexedDB:null,dbName:`stampfly-final-${f.productId}`})});
  const scene=createDefaultScene(catalog,f.productId);scene.firmware=null;await runtime.configure(scene);const loaded=await runtime.loadFirmware({url:f.url,sha256:f.sha256,bootMode:'rom'});
  assert.equal(loaded.product.runtime.board,f.board);assert.equal(loaded.firmware.appDirect,false);assert.deepEqual(loaded.firmware.stubs,[]);assert.equal(loaded.firmware.executionMode,'wasm-jit');record.loader={appDirect:loaded.firmware.appDirect,stubs:loaded.firmware.stubs,executionMode:loaded.firmware.executionMode,artifacts:loaded.firmware.artifacts};
  await runtime.start();let state=await wait('first complete ROM boot',s=>s.stats.time>=5&&markers.every(m=>usb(s).includes(m)));
  assert(!/Guru Meditation|abort\(\)/.test(state.serial));for(const m of markers)assert.equal(count(usb(state),m),1);assert.equal(count(usb(state),'RTC_SW_CPU_RST'),0);motorCheck(state);
  record.firstBoot={guestSeconds:state.stats.time,port:'usb',markers:Object.fromEntries(markers.map(m=>[m,count(usb(state),m)])),serialEvidence:lines(usb(state)),leds:state.leds,motors:state.motors,motorOutput:state.motorOutput,framebuffer:false};record.checks.push('complete-unchanged-factory-image-from-ROM-no-stubs-first-sensor-and-application-init');
  const freshOffset=usb(state).length,workerCount=workerCreations,pressedAt=state.stats.time;record.input={event:{type:'button',id:'button',pressed:true},pressedGuestSeconds:pressedAt,freshUsbOffset:freshOffset,hostResetUsed:false};await runtime.input(record.input.event);
  state=await wait('G0 held for at least 150 ms guest time',s=>s.stats.time>=pressedAt+0.15);record.input.releasedGuestSeconds=state.stats.time;await runtime.input({type:'button',id:'button',pressed:false});
  state=await wait('fresh G0-triggered software reboot and second complete init',s=>{const fresh=usb(s).slice(freshOffset),rst=fresh.indexOf('RTC_SW_CPU_RST');return rst>=0&&markers.every(m=>fresh.slice(rst).includes(m));});
  assert.equal(workerCreations,workerCount,'Software reboot must stay inside the same live worker.');assert(!/Guru Meditation|abort\(\)/.test(state.serial));const fresh=usb(state).slice(freshOffset);assert.equal(count(fresh,'RTC_SW_CPU_RST'),1);for(const m of markers){assert.equal(count(fresh,m),1);assert.equal(count(usb(state),m),2);}motorCheck(state);
  record.secondBoot={guestSeconds:state.stats.time,freshResetMarker:'RTC_SW_CPU_RST',freshMarkers:Object.fromEntries(markers.map(m=>[m,count(fresh,m)])),totalMarkers:Object.fromEntries(markers.map(m=>[m,count(usb(state),m)])),serialEvidence:lines(fresh),sameWorker:true,leds:state.leds,motors:state.motors,motorOutput:state.motorOutput,framebuffer:false};record.checks.push('fresh-USB-suffix-after-G0-has-one-software-reset-and-second-complete-sensor-and-application-init','same-live-worker-no-host-reset-or-synthetic-output','unpaired-factory-four-real-PWM-routes-zero-duty-no-framebuffer');
  await runtime.pause();await runtime.dispose();runtime=null;record.status='pass';await writeFile(out,JSON.stringify(receipt,null,2)+'\n');console.error(`${f.productId}: passed first and fresh second factory boot`);
 }
 for(const [p,sha]of Object.entries(sourceHashes)){if(p.startsWith('web/platform/catalog/'))continue;assert.equal(hash(await readFile(rootFile(p))),sha,`Source changed during run: ${p}`);}
 receipt.sourceJsAndWasmUnchangedDuringRun=true;receipt.status='pass';
}catch(e){receipt.status='fail';receipt.error=String(e.message).replaceAll(root,'<integration-root>');if(lastState)receipt.lastObserved={status:lastState.status,guestSeconds:lastState.stats?.time,serialEvidence:lines(usb(lastState)),leds:lastState.leds,motors:lastState.motors};process.exitCode=1;}
finally{if(runtime)await runtime.dispose();receipt.workerLifecycle={created:workerCreations,terminated:workerTerminations};await writeFile(out,JSON.stringify(receipt,null,2)+'\n');}
