#!/usr/bin/env node
// Caller-selected PaperS3 firmware, real page worker, isolated Flash/TF.
// Feed JSON action lines on stdin or --actions; quit always disposes the worker.
import assert from 'node:assert/strict';
import {readFile,writeFile,mkdir,appendFile} from 'node:fs/promises';
import {basename,join} from 'node:path';
import {Worker} from 'node:worker_threads';
import {createHash} from 'node:crypto';
import {deflateSync} from 'node:zlib';
import {createInterface} from 'node:readline';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDefaultScene} from '../web/platform/catalog.js';
import {createDeviceStorage} from '../web/platform/device-storage.js';
import {createSdCardStorage} from '../web/platform/sd-card-storage.js';
const args=process.argv.slice(2),arg=k=>args[args.indexOf(k)+1];
for(const k of ['--url','--sha256','--output'])assert(args.includes(k)&&arg(k),`Required ${k}`);
const url=arg('--url'),expected=arg('--sha256'),out=arg('--output'),root=new URL('../',import.meta.url),hash=b=>createHash('sha256').update(b).digest('hex');
assert(/^https:\/\//.test(url)&&/^[a-f0-9]{64}$/.test(expected));await mkdir(out,{recursive:true});
const response=args.includes('--file')?null:await fetch(url,{signal:AbortSignal.timeout(90000)});if(response)assert(response.ok,`Firmware HTTP ${response.status}`);const image=new Uint8Array(response?await response.arrayBuffer():await readFile(arg('--file')));assert.equal(hash(image),expected);
const catalog={};for(const key of ['products','accessories','firmwares'])catalog[key]=JSON.parse(await readFile(new URL(`web/platform/catalog/${key}.json`,root))).items;
const fetchImpl=async(value,options)=>{const u=new URL(value);if(String(u)===url)return new Response(image);if(u.origin!=='https://papers3-probe.invalid')return fetch(value,options);try{return new Response(await readFile(new URL(u.pathname.endsWith('.elf')?'web/wasm/fw/'+basename(u.pathname):'web'+u.pathname,root)));}catch{return new Response('',{status:404});}};
let queryClock;
const workerFactory=u=>{const module=new URL('web'+u.pathname,root),thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(d,t)=>parentPort.postMessage(d,t);await import(${JSON.stringify(String(module))});parentPort.on('message',data=>globalThis.onmessage({data}));`)));const adapter={postMessage:(...a)=>thread.postMessage(...a),terminate:()=>thread.terminate()};let seq=0;const clocks=new Map();queryClock=()=>new Promise((resolve,reject)=>{const id=`probe-clock-${++seq}`,timer=setTimeout(()=>{clocks.delete(id);reject(Error('Clock reply timeout'));},90000);clocks.set(id,time=>{clearTimeout(timer);resolve(time);});adapter.postMessage({op:'text',data:'{}',inputId:id});});thread.on('message',d=>{if(clocks.has(d.inputQueued)){clocks.get(d.inputQueued)(d.time);clocks.delete(d.inputQueued);}else adapter.onmessage?.({data:d});});thread.on('error',e=>adapter.onerror?.(e));return adapter;};
const runtime=createPlatformRuntime({catalog,fetchImpl,workerFactory,baseUrl:'https://papers3-probe.invalid/platform/',assetBaseUrl:'https://papers3-probe.invalid/',deviceStorage:createDeviceStorage({indexedDB:null,dbName:'papers3-edc-flash'}),sdStorage:createSdCardStorage({indexedDB:null,BroadcastChannel:null,dbName:'papers3-edc-sd'})});
const receipt={checkedAt:new Date().toISOString(),productId:'m5stack-c139',board:'papers3',source:{url,resolvedUrl:response?.url??null,input:response?'download':'caller-local-file',bytes:image.length,sha256:expected},firmwarePatched:false,wasmSha256:hash(await readFile(new URL('web/wasm/esp32sim.wasm',root))),actions:[],frames:[]};
const crc32=b=>{let n=0xffffffff;for(const v of b){n^=v;for(let i=0;i<8;i++)n=(n>>>1)^((n&1)?0xedb88320:0);}return(n^0xffffffff)>>>0;};
function png(frame){const chunk=(s,b)=>{const name=Buffer.from(s),len=Buffer.alloc(4),crc=Buffer.alloc(4);len.writeUInt32BE(b.length);crc.writeUInt32BE(crc32(Buffer.concat([name,b])));return Buffer.concat([len,name,b,crc]);},h=Buffer.alloc(13);h.writeUInt32BE(frame.width);h.writeUInt32BE(frame.height,4);h[8]=8;h[9]=6;const rows=Buffer.alloc((frame.width*4+1)*frame.height);for(let y=0;y<frame.height;y++)rows.set(frame.pixels.subarray(y*frame.width*4,(y+1)*frame.width*4),y*(frame.width*4+1)+1);return Buffer.concat([Buffer.from([137,80,78,71,13,10,26,10]),chunk('IHDR',h),chunk('IDAT',deflateSync(rows)),chunk('IEND',Buffer.alloc(0))]);}
const wait=async seconds=>{const target=await queryClock()+seconds,end=Date.now()+300000;while(await queryClock()<target){const s=await runtime.observe();assert.equal(s.status,'running',`Guest ${s.status}: ${JSON.stringify(s.lastError??null)}; time=${s.stats?.time??null}; frames=${s.stats?.frames??0}`);assert(!(s.serialByPort.uart0??s.serial).includes('Guru Meditation Error'),'Guest panic');assert(Date.now()<end,`Guest time timeout at ${s.stats?.time??null}, target ${target}`);await new Promise(r=>setTimeout(r,10));}};
const snapshot=async()=>{const s=await runtime.observe();return {status:s.status,stats:s.stats,frame:s.frame,sdCard:s.sdCard,network:s.network,lastError:s.lastError,romBoots:(s.serialByPort.uart0?.match(/ESP-ROM:esp32s3/g)||[]).length};};
async function action(a){assert(a&&typeof a==='object');let result;
 switch(a.op){
 case 'wait':assert(a.seconds>0&&a.seconds<=300);await wait(a.seconds);result=await snapshot();break;
 case 'tap':{const down=await runtime.input({type:'touch',x:a.x,y:a.y,down:true});let up;try{await wait(a.hold??0.15);}finally{up=await runtime.input({type:'touch',x:a.x,y:a.y,down:false});}await wait(a.settle??2);result={down,up,state:await snapshot()};break;}
 case 'capture':{assert(/^[a-z0-9_-]+$/.test(a.name));await runtime.pause();const f=await runtime.capture({format:'rgba'}),p=png(f);await writeFile(join(out,a.name+'.png'),p);result={name:a.name,width:f.width,height:f.height,rgbaSha256:hash(f.pixels),pngSha256:hash(p),state:await snapshot()};receipt.frames.push(result);await runtime.start();break;}
 case 'network-config':result=await runtime.configureNetwork(a.options);break;
 case 'http':result=await runtime.requestNetworkHttp(a.options);await writeFile(join(out,'http-'+receipt.actions.length+'.json'),JSON.stringify(result));result={status:result.status,bytes:result.body?.length,bodyPreview:result.body?.slice(0,300)};break;
 case 'serial':{const s=await runtime.observe();result=s.serialByPort.uart0?.slice(-(a.tail??5000));break;}
 case 'observe':result=await snapshot();break;
 case 'quit':return false;
 default:throw Error('Unknown action');
 }
 // Connection secrets and HTTP request bodies stay in the caller-owned raw file.
 receipt.actions.push({action:a.op==='network-config'?{op:a.op}:a,result});await appendFile(join(out,'actions.jsonl'),JSON.stringify(a)+'\n');console.log(JSON.stringify({op:a.op,result}));return true;
}
try{const scene=createDefaultScene(catalog,'m5stack-c139');scene.firmware={url,sha256:expected};await runtime.configure(scene);if(args.includes('--network-config-file')){const settings=JSON.parse(await readFile(arg('--network-config-file'),'utf8'));await runtime.configureNetwork({mode:'local-helper',endpoint:settings.endpoint,autoProvision:false});}await runtime.loadFirmware();await runtime.start();const bootSeconds=args.includes('--boot-seconds')?Number(arg('--boot-seconds')):30;assert(Number.isFinite(bootSeconds)&&bootSeconds>0&&bootSeconds<=300,'--boot-seconds must be in (0,300]');receipt.bootSeconds=bootSeconds;await wait(bootSeconds);await action({op:'capture',name:'boot'});console.log('READY');
 if(args.includes('--actions')){for(const a of JSON.parse(await readFile(arg('--actions'),'utf8')))if(!await action(a))break;}
 else{const lines=createInterface({input:process.stdin,crlfDelay:Infinity});try{for await(const line of lines){if(!line.trim())continue;try{if(!await action(JSON.parse(line)))break;}catch(e){console.log(JSON.stringify({error:e.message}));}}}finally{lines.close();}}
 receipt.status='completed';
}catch(e){receipt.status='error';receipt.error=e.message;process.exitCode=1;console.error(e.message);}
finally{try{const s=await runtime.observe();receipt.final=await snapshot();await writeFile(join(out,'serial.log'),s.serial);await writeFile(join(out,'serial-uart0.log'),s.serialByPort.uart0??'');await writeFile(join(out,'receipt.json'),JSON.stringify(receipt,null,2)+'\n');}finally{await runtime.dispose();}}
