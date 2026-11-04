#!/usr/bin/env node
// Actual guest execution and peripheral outputs; no fabricated UART/LED/PWM responses.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import {createHash} from 'node:crypto';
import {Worker} from 'node:worker_threads';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDefaultScene} from '../web/platform/catalog.js';
import {createDeviceStorage} from '../web/platform/device-storage.js';
const arg=k=>process.argv[process.argv.indexOf(k)+1],file=arg('--firmware');
assert(process.argv.includes('--firmware')&&file,'--firmware <source-built merged.bin> is required');
const root=process.argv.includes('--assets-root')?pathToFileURL(resolve(arg('--assets-root'))+'/'):new URL('../',import.meta.url),origin='https://stampfly-demo.invalid';
const hash=b=>createHash('sha256').update(b).digest('hex'),bytes=new Uint8Array(await readFile(file)),catalog={};
for(const k of ['products','accessories','firmwares'])catalog[k]=JSON.parse(await readFile(new URL(`web/platform/catalog/${k}.json`,root))).items;
const fetchImpl=async value=>{const u=new URL(value);assert.equal(u.origin,origin);try{return new Response(await readFile(new URL('web'+u.pathname,root)));}catch{return new Response('',{status:404});}};
const workerFactory=url=>{const real=new URL('web'+url.pathname,root),thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(d,t)=>parentPort.postMessage(d,t);await import(${JSON.stringify(String(real))});parentPort.on('message',d=>globalThis.onmessage({data:d}));`)));const w={postMessage:(...args)=>thread.postMessage(...args),terminate:()=>thread.terminate()};thread.on('message',data=>w.onmessage?.({data}));thread.on('error',e=>w.onerror?.(e));return w;};
const api=createPlatformRuntime({catalog,fetchImpl,workerFactory,baseUrl:origin+'/platform/',assetBaseUrl:origin+'/',deviceStorage:createDeviceStorage({indexedDB:null,dbName:'stampfly-uart-demo-proof'})});
const delay=ms=>new Promise(r=>setTimeout(r,ms)),uart=s=>s.serialByPort.uart0||'';
async function wait(label,predicate){const end=Date.now()+120000;while(Date.now()<end){const s=await api.observe();if(predicate(s))return s;if(s.status==='error')throw Error(JSON.stringify(s.lastError));await delay(20);}const s=await api.observe();throw Error(label+' timeout '+JSON.stringify({status:s.status,motors:s.motors,serial:uart(s).slice(-2500)}));}
const targets=process.argv.includes('--product')?[arg('--product')]:['m5stack-k138','m5stack-k138-v11'];
const receipt={checkedAt:new Date().toISOString(),method:'actual-wasm-worker-full-ROM',firmwareSha256:hash(bytes),firmwareBytes:bytes.length,wasmSha256:hash(await readFile(new URL('web/wasm/esp32sim.wasm',root))),products:[],limitations:['Independent public UART demo, not official factory firmware or physical-flight qualification.','This probe qualifies MOTOR/STOP, UART and sensor initialization; closed-loop flight requires separate dynamics evidence.']};
const matches=(s,duty)=>s.motors?.length===4&&s.motors.every((m,i)=>m.gpio===[5,42,10,41][i]&&Math.abs(m.duty-duty[i])<1/65535&&Math.abs(m.frequencyHz-150000)<1500);
try{for(const id of targets){assert(['m5stack-k138','m5stack-k138-v11'].includes(id));const scene=createDefaultScene(catalog,id);scene.firmware=null;await api.configure(scene);await api.loadFirmware({file:new File([bytes],'stampfly-uart-demo.bin')});await api.start();
 const ready=await wait('READY/off',s=>uart(s).includes('SF1 READY stampfly-uart-demo 0.1.0')&&matches(s,[0,0,0,0]));
 const r={product:id,initialMotors:ready.motors,sensorsInitialized:uart(ready).split(/\r?\n/).filter(l=>l.startsWith('SF1 SENSOR_INIT')),checks:[],commands:[]};receipt.products.push(r);
 async function command(line,response,duties){const at=uart(await api.observe()).length;const payload=line+'\n';for(let i=0;i<payload.length;i+=96){const clock=(await api.observe()).stats.time;await api.input({type:'serial',port:'uart0',data:payload.slice(i,i+96)});if(i+96<payload.length)await wait('UART fragmented line consumption',s=>s.stats.time>clock+.02);}const s=await wait(line,s=>uart(s).slice(at).includes(response)&&matches(s,duties));r.commands.push({line,response,motors:s.motors});return s;}
 const expected=[.125,.25,.5,.75];await command('SF1 MOTOR 0.125 0.25 0.5 0.75','SF1 ACK MOTOR',expected);
 for(const [line,code] of [['SF1 MOTOR NaN 0 0 0','RANGE'],['SF1 MOTOR 1.1 0 0 0','RANGE'],['SF1 MOTOR 0 0 0','FORMAT'],['SF1 MOTOR 0 0 0 0 extra','FORMAT'],['SF1 STOP extra','FORMAT'],['SF1 LAND','MODE'],['x'.repeat(160),'LINE_TOO_LONG']])await command(line,'SF1 ERR '+code,expected);
 await api.pause();const paused=await api.observe();await delay(120);const frozen=await api.observe();assert.equal(uart(frozen),uart(paused));assert.deepEqual(frozen.motors,paused.motors);assert.equal(frozen.stats.time,paused.stats.time);await assert.rejects(api.input({type:'serial',port:'uart0',data:'SF1 STOP\n'}),{code:'NOT_RUNNING'});await api.start();
 await command('SF1 STOP','SF1 ACK STOP',[0,0,0,0]);await command('SF1 MOTOR 0.125 0.25 0.5 0.75','SF1 ACK MOTOR',expected);
 await api.reset();const reset=await wait('reset/off/fresh READY',s=>uart(s).includes('SF1 READY stampfly-uart-demo 0.1.0')&&matches(s,[0,0,0,0]));assert(!uart(reset).includes('SF1 ACK MOTOR'));assert.equal(reset.frame,null);
 r.checks=['real UART0 command parsing to four correctly routed independent LEDC duties','nonfinite/range/arity/long-line errors preserve prior motor outputs','pause freezes guest and rejects new input','STOP writes all motor duties zero','platform reset boots same firmware with zero motors','no framebuffer'];r.status='pass';await api.pause();
}receipt.status='pass';console.log(JSON.stringify(receipt,null,2));}finally{await api.dispose();}
