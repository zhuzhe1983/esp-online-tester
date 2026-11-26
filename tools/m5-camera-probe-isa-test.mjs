// Production Worker/WASM smoke test for the authored LX7 CRC/hex/timeout code.
// A disconnected NoBoard must keep sentinel RAM. This is explicitly not a
// positive camera capture test; platform-m5-camera-worker-test.mjs covers that.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {Worker} from 'node:worker_threads';
import {crc32} from 'node:zlib';
import {m5CameraCaptureProbe} from './fixtures/m5-camera-capture-probe.mjs';
const workerUrl=new URL('../web/wasm/worker.js',import.meta.url);
const binary=b=>b.buffer.slice(b.byteOffset,b.byteOffset+b.byteLength);
test('authored LX7 prints independently checked CRC32 and RAM bytes through production Worker/WASM',{timeout:30000},async()=>{
 const wasm=await readFile(process.env.ESP32SIM_WASM||new URL('../web/wasm/esp32sim.wasm',import.meta.url));
 const source=`import{parentPort}from'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(m,t)=>parentPort.postMessage(m,t);await import(${JSON.stringify(workerUrl.href)});parentPort.on('message',data=>globalThis.onmessage({data}));`;
 const w=new Worker(new URL('data:text/javascript,'+encodeURIComponent(source))),messages=[],waiters=[];let serial='';
 w.on('message',m=>{messages.push(m);if(m.text){try{const t=JSON.parse(m.text);if(t.t==='serial')serial+=t.data||'';}catch{}}for(const q of [...waiters])if(q.test(m)){clearTimeout(q.timer);waiters.splice(waiters.indexOf(q),1);q.resolve(m);}});
 w.on('error',error=>{for(const q of waiters.splice(0)){clearTimeout(q.timer);q.reject(error);}});
 const wait=fn=>new Promise((resolve,reject)=>{const found=messages.find(fn);if(found){resolve(found);return;}const q={test:fn,resolve,reject};q.timer=setTimeout(()=>{waiters.splice(waiters.indexOf(q),1);reject(Error('LX7 CRC smoke timeout: '+serial.slice(-500)+' '+JSON.stringify(messages.slice(-3))));},20000);waiters.push(q);});
 const send=async(m,fn)=>{messages.length=0;w.postMessage(m);return wait(fn);};
 try{
  await send({op:'init',wasm:binary(wasm)},m=>m.ready===true);
  const create=await send({op:'create',board:'none',flash_mb:16,psram_mb:8,jit:true,appDirect:true},m=>Object.hasOwn(m,'created'));assert.equal(create.created,true);
  const probe=m5CameraCaptureProbe('m5stack-u174',{format:'jpeg',jpegBytes:882,timeoutCycles:1000000});
  const loaded=await send({op:'load',kind:3,data:binary(probe)},m=>m.loaded===3);assert.equal(loaded.ok,true);
  const started=await send({op:'start',appDirect:true},m=>Object.hasOwn(m,'started'));assert.equal(started.started,true);
  const want='CAPTURE EOF=0 HEAD='+'a5'.repeat(16)+' CRC='+(crc32(Buffer.alloc(882,0xa5))>>>0).toString(16).padStart(8,'0')+' END\n';
  await wait(()=>serial.includes(want));assert(serial.includes('CAMERA SRAM READY ov2640 jpeg\n'));assert(serial.includes(want));
 }finally{for(const q of waiters){clearTimeout(q.timer);}await w.terminate();}
});
