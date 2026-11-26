// Production Worker + compiled WASM; guest GPIO and external outputs, no mock exports.
// ESP32SIM_WASM=/absolute/esp32sim.wasm ESP32SIM_M5_EDGE_PRODUCT_IDS=m5stack-u139 node --test tools/platform-m5-edge-worker-test.mjs
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {Worker} from 'node:worker_threads';
import {m5SsrProbe,m5GatewayH2Probe,m5CardKb2Probe,CARDKB2_COLUMNS,m5KmeterProbe,m5SwitchC6Probe,m5A172Probe,m5ToughC5Probe} from './fixtures/m5-edge-rv-probes.mjs';
const wasm=await readFile(process.env.ESP32SIM_WASM||new URL('../web/wasm/esp32sim.wasm',import.meta.url));
const workerUrl=new URL('../web/wasm/worker.js',import.meta.url);
const requested=process.env.ESP32SIM_M5_EDGE_PRODUCT_IDS?.split(',').map(s=>s.trim()).filter(Boolean);
const selected=id=>!requested||requested.includes(id);
const binary=b=>b.buffer.slice(b.byteOffset,b.byteOffset+b.byteLength);
export function productionWorker(){
 const script=`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(m,t)=>parentPort.postMessage(m,t);await import(${JSON.stringify(workerUrl.href)});parentPort.on('message',data=>globalThis.onmessage({data}));`;
 const thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(script))),messages=[],waiters=[];let serial='',rings=[],diagnostics=[],frames=[];
 thread.on('message',m=>{messages.push(m);if(m.text){try{const v=JSON.parse(m.text);if(v.t==='serial')serial+=v.data||'';if(v.t==='ring')rings.push(v.leds);if(v.t==='hardware-diagnostics')diagnostics.push(v);}catch{}}if(m.bin){const b=new Uint8Array(m.bin);if(b[0]===1){const v=new DataView(m.bin);frames.push({width:v.getUint16(1,true),height:v.getUint16(3,true),first:v.getUint16(5,true)});}}
  for(const w of [...waiters])if(messages.length>w.after&&w.predicate(m)){clearTimeout(w.timer);waiters.splice(waiters.indexOf(w),1);w.resolve(m);}});
 thread.on('error',e=>{for(const w of waiters.splice(0)){clearTimeout(w.timer);w.reject(e);}});
 const wait=(predicate,after=0)=>{const found=messages.slice(after).find(predicate);if(found)return Promise.resolve(found);return new Promise((resolve,reject)=>{const w={predicate,after,resolve,reject};w.timer=setTimeout(()=>{waiters.splice(waiters.indexOf(w),1);reject(Error('Production M5 Worker timeout; serial='+JSON.stringify(serial.slice(-400))+' messages='+JSON.stringify(messages.slice(-4))));},20000);waiters.push(w);});};
 return {messages,get serial(){return serial;},get rings(){return rings;},get diagnostics(){return diagnostics;},get frames(){return frames;},wait,request(data,predicate){const after=messages.length;thread.postMessage(data);return wait(predicate,after);},clear(){serial='';rings=[];diagnostics=[];frames=[];},close:()=>thread.terminate()};
}
async function start(h,board,image,{flash=4,psram=0}={}){
 const created=await h.request({op:'create',board,flash_mb:flash,psram_mb:psram,jit:false,appDirect:true},m=>Object.hasOwn(m,'created'));assert.equal(created.created,true,board);
 const loaded=await h.request({op:'load',kind:3,data:binary(image)},m=>m.loaded===3);assert.equal(loaded.ok,true,board);
 const started=await h.request({op:'start',appDirect:true},m=>Object.hasOwn(m,'started'));assert.equal(started.started,true,board);return created;
}
let inputId=0;
async function input(h,data){const id=++inputId;return h.request({op:'text',inputId:id,data:JSON.stringify(data)},m=>m.inputQueued===id);}
const ssrs=[['m5stack-u139','m5-acssr'],['m5stack-u140','m5-dcssr']];
for(const [product,board]of ssrs)if(selected(product))test(product+' guest GPIO drives actual SSR control output and reads physical button',async t=>{
 const h=productionWorker();try{
  await h.request({op:'init',wasm:binary(wasm)},m=>m.ready===true);
  await start(h,board,m5SsrProbe());await h.wait(()=>h.serial.includes('SSR KEY UP\n'));
  await input(h,{t:'btn',pin:3,v:1});await h.wait(()=>h.serial.includes('SSR KEY DOWN\n')&&h.rings.some(r=>r[1]?.[0]===255));
  const after=h.serial.length;await input(h,{t:'btn',pin:3,v:0});await h.wait(()=>h.serial.slice(after).includes('SSR KEY UP\n')&&h.rings.at(-1)?.[1]?.every(v=>v===0));
  await h.request({op:'stop'},m=>Object.hasOwn(m,'stopped'));t.diagnostic(JSON.stringify({product,guestSerial:h.serial,relayColors:h.rings.map(r=>r[1])}));
  for(const options of [{pin:5},{mux:false},{enabled:false}]){
   h.clear();await start(h,board,m5SsrProbe(options));await h.wait(()=>h.serial.includes('SSR KEY UP\n'));await input(h,{t:'btn',pin:3,v:1});await h.wait(()=>h.serial.includes('SSR KEY DOWN\n'));
   assert(!h.rings.some(r=>r[1]?.[0]===255),'wrong GPIO/IO_MUX/output-enable cannot close SSR');await h.request({op:'stop'},m=>Object.hasOwn(m,'stopped'));
  }
 }finally{await h.close();}
});

const allProducts=['m5stack-u139','m5stack-u140','m5stack-u215','m5stack-u195','m5stack-m141','m5stack-u133','m5stack-k140','m5stack-a172','m5stack-k162'];
if(requested)for(const id of requested)assert(allProducts.includes(id),'Unknown edge product filter '+id);
const hex=v=>(v>>>0).toString(16).padStart(8,'0');
async function settle(h,at){await h.wait(m=>{if(!m.text)return false;try{const v=JSON.parse(m.text);return v.t==='stat'&&v.time>=at+0.04;}catch{return false;}});}
async function stop(h){await h.request({op:'stop'},m=>Object.hasOwn(m,'stopped'));}
for(const [product,board]of[['m5stack-u195','m5-unit-gateway-h2'],['m5stack-m141','m5-module-gateway-h2']])if(selected(product))test(product+' real guest UART host link requires RX23 TX24 and baud',async t=>{
 const h=productionWorker();try{
  await h.request({op:'init',wasm:binary(wasm)},m=>m.ready===true);const created=await start(h,board,m5GatewayH2Probe(),{flash:2});assert.equal(created.networkAvailable,false);assert.equal(created.flashStorage,false);
  await h.wait(()=>h.serial.includes('H2 HOST READY\n'));await input(h,{t:'key',src:'uart0',data:'A'});
  await h.wait(()=>h.serial.includes('H2 RX 00000041\n')&&h.diagnostics.some(d=>d.state?.uartTxTailHex?.endsWith(Buffer.from('H2 RX 00000041\n').toString('hex'))));
  t.diagnostic(JSON.stringify({product,guestSerial:h.serial,physicalUartOutput:h.diagnostics.at(-1)?.state}));await stop(h);
  for(const options of[{rx:22},{tx:25},{enabled:false},{div:694}]){
   h.clear();await start(h,board,m5GatewayH2Probe(options),{flash:2});await h.wait(()=>h.serial.includes('H2 HOST READY\n'));
   const receipt=await input(h,{t:'key',src:'uart0',data:'B'});await settle(h,receipt.time);await stop(h);
   if(options.rx===22||options.div===694)assert(!h.serial.includes('H2 RX '),'wrong RX or baud cannot feed the guest FIFO');
   if(options.tx===25||options.enabled===false||options.div===694)assert.equal(h.diagnostics.at(-1)?.state?.uartTxCount,'0','wrong TX/drive/baud cannot reach the actual host output');
  }
 }finally{await h.close();}
});
if(selected('m5stack-u215'))test('m5stack-u215 production Worker closes and guest scans every one of42 physical switches',async t=>{
 const h=productionWorker();try{
  await h.request({op:'init',wasm:binary(wasm)},m=>m.ready===true);await start(h,'m5-cardkb2',m5CardKb2Probe());await h.wait(()=>h.serial.includes('KB ROW3 00000000\n'));
  let keys=0;for(let row=0;row<4;row++)for(let column=0;column<11;column++){
   if(column===10&&[0,3].includes(row))continue;const before=h.serial.length;
   await input(h,{t:'matrix-key',row,column,pressed:1});await h.wait(()=>h.serial.slice(before).includes(`KB ROW${row} ${hex(1<<CARDKB2_COLUMNS[column])}\n`));
   const after=h.serial.length;await input(h,{t:'matrix-key',row,column,pressed:0});await h.wait(()=>h.serial.slice(after).includes(`KB ROW${row} 00000000\n`));keys++;
  }
  for(const[row,column]of[[0,10],[3,10],[4,0],[0,11]]){const before=h.serial.length,receipt=await input(h,{t:'matrix-key',row,column,pressed:1});await settle(h,receipt.time);assert.equal(h.serial.length,before,'absent/invalid switch must not produce a guest scan transition');}
  await stop(h);t.diagnostic(JSON.stringify({product:'m5stack-u215',physicalKeysScanned:keys,serialTail:h.serial.slice(-180)}));
  for(const options of[{badMux:true},{enabled:false}]){h.clear();await start(h,'m5-cardkb2',m5CardKb2Probe(options));await h.wait(()=>h.serial.includes('KB ROW3 00000000\n'));const receipt=await input(h,{t:'matrix-key',row:0,column:0,pressed:1});await settle(h,receipt.time);assert(!h.serial.includes('KB ROW0 00000010\n'));await stop(h);}
 }finally{await h.close();}
});
if(selected('m5stack-u133'))test('m5stack-u133 production Worker thermocouple stimulus crosses converter SPI and guest signed decode',async t=>{
 const h=productionWorker();try{
  await h.request({op:'init',wasm:binary(wasm)},m=>m.ready===true);await start(h,'m5-kmeter',m5KmeterProbe());await h.wait(()=>h.serial.includes('KMETER FRAME 00011901 HOTQ 00000000\n'));
  for(const temperatureC of[123.25,-30.5,1350]){const at=h.serial.length;await input(h,{t:'thermocouple-environment',connected:1,temperatureC});const quarter=Math.round(temperatureC*4),word=(((quarter&0x3fff)<<18)|(400<<4))>>>0;await h.wait(()=>h.serial.slice(at).includes(`KMETER FRAME ${hex(word)} HOTQ ${hex(quarter)}\n`));}
  const at=h.serial.length;await input(h,{t:'thermocouple-environment',connected:0,temperatureC:25});await h.wait(()=>h.serial.slice(at).includes('KMETER FRAME 00011901 HOTQ 00000000\n'));await stop(h);t.diagnostic(JSON.stringify({product:'m5stack-u133',guestSerial:h.serial}));
  for(const options of[{miso:4},{clock:8}]){h.clear();await start(h,'m5-kmeter',m5KmeterProbe(options));await h.wait(()=>h.serial.includes('KMETER FRAME ffffffff HOTQ ffffffff\n'));await stop(h);}
 }finally{await h.close();}
});
if(selected('m5stack-k140'))test('m5stack-k140 guest reads G7 and emits actual relay coil pulses',async t=>{
 const h=productionWorker();try{
  await h.request({op:'init',wasm:binary(wasm)},m=>m.ready===true);await start(h,'m5-switchc6',m5SwitchC6Probe());await h.wait(()=>h.serial.includes('SWITCH OFF\n'));
  await input(h,{t:'btn',pin:7,v:1});await h.wait(()=>h.serial.includes('SWITCH ON\n')&&h.rings.some(r=>r[0]?.[1]===255));const after=h.serial.length;
  await input(h,{t:'btn',pin:7,v:0});await h.wait(()=>h.serial.slice(after).includes('SWITCH OFF\n')&&h.rings.at(-1)?.[0]?.every(v=>v===0));await stop(h);t.diagnostic(JSON.stringify({product:'m5stack-k140',guestSerial:h.serial,latchStates:h.rings}));
  for(const options of[{onPin:13},{mux:false},{enabled:false}]){h.clear();await start(h,'m5-switchc6',m5SwitchC6Probe(options));await h.wait(()=>h.serial.includes('SWITCH OFF\n'));const receipt=await input(h,{t:'btn',pin:7,v:1});await h.wait(()=>h.serial.includes('SWITCH ON\n'));await settle(h,receipt.time);assert(!h.rings.some(r=>r[0]?.[1]===255));await stop(h);}
 }finally{await h.close();}
});
async function sdio(h,command,arg){const id=++inputId;return h.request({op:'sdio-host-command',requestId:id,command,arg:arg>>>0},m=>m.sdioReply===id);}
if(selected('m5stack-a172'))test('m5stack-a172 production Worker SDIO host mailbox reaches and returns actual C6 guest data',async t=>{
 const h=productionWorker();try{
  await h.request({op:'init',wasm:binary(wasm)},m=>m.ready===true);await start(h,'m5-stamp-addon-c6',m5A172Probe());await h.wait(()=>h.serial.includes('A172 MAILBOX 41313732\n'));
  for(const[cmd,arg]of[[5,0xff8000],[3,0],[7,1<<16],[52,(1<<31)|(2<<9)|2]])assert.equal((await sdio(h,cmd,arg)).ok,true);
  assert.equal((await sdio(h,52,(1<<28)|(0x78<<9))).response&255,0x32);const before=h.serial.length;assert.equal((await sdio(h,52,(1<<31)|(1<<28)|(0x79<<9)|0x5a)).ok,true);await h.wait(()=>h.serial.slice(before).includes('A172 MAILBOX 41315a32\n'));
  t.diagnostic(JSON.stringify({product:'m5stack-a172',guestSerial:h.serial,hostReadLowByte:0x32}));await stop(h);
  h.clear();await start(h,'m5-stamp-addon-c6',m5A172Probe({badPin:19}));await h.wait(()=>h.serial.includes('A172 MAILBOX 41313732\n'));assert.equal((await sdio(h,5,0xff8000)).ok,false,'wrong native SDIO clock disconnects external host');await stop(h);
 }finally{await h.close();}
});
if(selected('m5stack-k162'))test('m5stack-k162 production Worker guest CHSC6540 read changes actual SPI framebuffer',async t=>{
 const h=productionWorker();try{
  await h.request({op:'init',wasm:binary(wasm)},m=>m.ready===true);await start(h,'m5-tough-c5',m5ToughC5Probe(),{flash:16,psram:8});await h.wait(()=>h.serial.includes('TOUGH C5 READY\n')&&h.frames.some(f=>f.first===0xf800));
  await input(h,{t:'touch',x:123,y:87,down:1});await h.wait(()=>h.serial.includes('TOUGH TOUCH 00000001 X 0000007b Y 00000057\n')&&h.frames.at(-1)?.first===0x07e0);
  const at=h.serial.length;await input(h,{t:'touch',x:123,y:87,down:0});await h.wait(()=>h.serial.slice(at).includes('TOUGH TOUCH 00000000')&&h.frames.at(-1)?.first===0xf800);await stop(h);
  assert(h.frames.every(f=>f.width===320&&f.height===240));t.diagnostic(JSON.stringify({product:'m5stack-k162',guestSerial:h.serial,frameFirstPixels:h.frames.map(f=>hex(f.first))}));
  h.clear();await start(h,'m5-tough-c5',m5ToughC5Probe({wrongClock:true}),{flash:16,psram:8});await h.wait(()=>h.serial.includes('TOUGH C5 READY\n'));const receipt=await input(h,{t:'touch',x:123,y:87,down:1});await h.wait(()=>h.serial.includes('TOUGH TOUCH 00000001 X 0000007b Y 00000057\n'));await settle(h,receipt.time);assert(!h.frames.some(f=>f.first===0x07e0),'touch response cannot paint the display with the SPI clock disconnected');await stop(h);
 }finally{await h.close();}
});
