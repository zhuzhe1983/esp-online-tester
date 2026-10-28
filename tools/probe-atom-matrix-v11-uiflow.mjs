#!/usr/bin/env node
// Official remote UIFlow2-image probe; remote target/hash are explicit, no firmware is published.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {Worker} from 'node:worker_threads';
import {createHash} from 'node:crypto';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDeviceStorage} from '../web/platform/device-storage.js';
import {createDefaultScene} from '../web/platform/catalog.js';
const [url,sha256]=process.argv.slice(2);
if(!/^https:\/\//.test(url||'')||!/^[a-f0-9]{64}$/.test(sha256||''))throw Error('Supply <official-https-url> <sha256>.');
const root=new URL('../',import.meta.url),baseUrl='https://atom-matrix-v11-fixture.invalid/platform/';
const catalog={};for(const key of ['products','accessories','firmwares'])catalog[key]=JSON.parse(await readFile(new URL(`web/platform/catalog/${key}.json`,root))).items;
const fetchImpl=async (value,options)=>{const url=new URL(value);if(url.origin!=='https://atom-matrix-v11-fixture.invalid')return fetch(value,options);try{return new Response(await readFile(new URL('web'+url.pathname,root)));}catch{return new Response('',{status:404});}};
const workerFactory=url=>{const real=new URL('web'+url.pathname,root);const thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(String(real))});parentPort.on('message',data=>globalThis.onmessage({data}));`)));const adapter={postMessage:(message,...args)=>thread.postMessage(message.op==='create'&&process.argv.includes('--no-jit')?{...message,jit:false}:message,...args),terminate:()=>thread.terminate()};thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',e=>adapter.onerror?.(e));return adapter;};
const runtime=createPlatformRuntime({catalog,fetchImpl,workerFactory,baseUrl,assetBaseUrl:'https://atom-matrix-v11-fixture.invalid/',deviceStorage:createDeviceStorage({indexedDB:null,dbName:'atom-matrix-v11-official-probe'})});
const wait=async predicate=>{const end=Date.now()+45000;while(Date.now()<end){const state=await runtime.observe();if(predicate(state))return state;if(state.status==='error')throw Error(JSON.stringify(state.lastError));await new Promise(r=>setTimeout(r,30));}throw Error(`Guest condition timed out: ${(await runtime.observe()).serial.slice(-2000)}`);};
const hash=b=>createHash('sha256').update(b).digest('hex');
const runLine=async(code,expected)=>{const start=(await runtime.observe()).serial.length;await runtime.input({type:'serial',port:'uart0',data:code+'\r\n'});return wait(s=>s.serial.slice(start).includes(expected));};
try {
 const scene=createDefaultScene(catalog,'m5stack-c008-b-v11');scene.firmware={url,sha256};await runtime.configure(scene);await runtime.loadFirmware();await runtime.start();
 await wait(s=>s.serial.includes('M5STACK Atom Matrix')&&s.serial.includes('>>>'));
 await runLine("import M5, time; M5.begin(); print('MATRIX_BEGIN')",'\nMATRIX_BEGIN\r');
 await runLine("M5.update(); time.sleep_ms(30); M5.update(); print('MATRIX_KEY', int(M5.BtnA.isPressed()))",'MATRIX_KEY 0');
 const press=await runtime.input({type:'button',id:'button-a',pressed:true});
 await runLine("from machine import Pin; print('RAW39', Pin(39).value())",'RAW39 0');
 await runLine("M5.update(); time.sleep_ms(30); M5.update(); print('MATRIX_KEY', int(M5.BtnA.isPressed()))",'MATRIX_KEY 1');
 const release=await runtime.input({type:'button',id:'button-a',pressed:false});
 await runLine("M5.update(); time.sleep_ms(30); M5.update(); print('MATRIX_KEY', int(M5.BtnA.isPressed()))",'MATRIX_KEY 0');

 await runLine("print('IMU_AVAILABLE',M5.Imu.isEnabled()); print('ACCEL',M5.Imu.getAccel())",'ACCEL (0.0, 1.0, 0.0)');
 const imu=await runtime.setImu({enabled:true,source:'agent',quaternion:[0,0,Math.SQRT1_2,Math.SQRT1_2]});
 await runLine("M5.update(); print('ROTATED',M5.Imu.getAccel())",'ROTATED (1.0, 0.0, 0.0)');
 await runLine("from machine import I2C; b=I2C(0,scl=Pin(21),sda=Pin(25)); print('BMI_RANGE',b.readfrom_mem(104,65,1))",'\nBMI_RANGE ');
 await runLine("print('BMI_RAW',list(b.readfrom_mem(104,12,6)))",'BMI_RAW [0, 16, 0, 0, 0, 0]');
 await runLine("time.sleep_ms(100); M5.update(); print('ROTATED_LATER',M5.Imu.getAccel())",'ROTATED_LATER (1.0, 0.0, 0.0)');
 await runtime.pause();const state=await runtime.observe();
 assert.equal(state.frame,null);assert.equal(state.leds.length,25);
 console.log(JSON.stringify({checkedAt:new Date().toISOString(),productId:state.product.id,source:{url,sha256},wasmSha256:hash(await readFile(new URL('web/wasm/esp32sim.wasm',root))),backend:'actual-page-worker-wasm',firmwarePatched:false,guestProgram:'Explicit MicroPython REPL expressions call M5.begin/update and read M5.BtnA/M5.Imu, read BMI270 hardware I2C registers, executed by the original official firmware.',inputs:{press,release,imu},leds:state.leds,serialEvidence:state.serial.split(/\r?\n/).filter(l=>/^(MATRIX_KEY |RAW39 |IMU_AVAILABLE |ACCEL |ROTATED |ROTATED_LATER |BMI_RANGE |BMI_RAW |RGB_SENT)/.test(l)),guestSeconds:state.stats.time,checks:['official-micropython-repl','M5.BtnA-released0-pressed1-released0','actual-g27-25-pixel-output','M5.Imu-axis-change-after-quaternion','hardware-I2C-bmi270-range2-raw4096'],status:'pass',limitations:['IR/network transport/full BMI270 features not validated.','No physical hardware; REPL expressions test GPIO input, not a factory button workflow.']},null,2));
}finally {await runtime.dispose();}
