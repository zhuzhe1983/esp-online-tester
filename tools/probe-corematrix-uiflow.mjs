#!/usr/bin/env node
// Execute an explicitly supplied original CoreMatrix image with the production
// worker; the only added program is source uploaded through the guest USB REPL.
import assert from 'node:assert/strict';
import {readFile,writeFile} from 'node:fs/promises';
import {Worker} from 'node:worker_threads';
import {createHash} from 'node:crypto';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDeviceStorage} from '../web/platform/device-storage.js';
import {createDefaultScene} from '../web/platform/catalog.js';
import {createCoreMatrixRepl} from './fixtures/corematrix-uiflow/raw-repl.mjs';
const args=process.argv.slice(2),[file,sha256]=args;
if(!file||!/^[a-f0-9]{64}$/.test(sha256||''))throw Error('Supply <local-original-corematrix.bin> <sha256> [--checks matrix,buttons,imu,sd,buzzer,power] [--receipt file.json].');
const option=name=>{const i=args.indexOf(name);return i<0?undefined:args[i+1];};
const checks=(option('--checks')||'matrix,buttons,imu,sd,buzzer,power').split(',');
assert.ok(checks.every(name=>['matrix','buttons','imu','sd','buzzer','power'].includes(name)),'Unknown check name');
const root=new URL('../',import.meta.url),origin='https://corematrix-fixture.invalid',baseUrl=origin+'/platform/';
const hash=bytes=>createHash('sha256').update(bytes).digest('hex');
const bytes=await readFile(file);assert.equal(hash(bytes),sha256,'Original firmware SHA-256 mismatch');
const catalog={};for(const name of ['products','accessories','firmwares'])catalog[name]=JSON.parse(await readFile(new URL(`web/platform/catalog/${name}.json`,root))).items;
const fetchImpl=async(value)=>{const u=new URL(value);assert.equal(u.origin,origin,'This probe uses local assets only');try{return new Response(await readFile(new URL('web'+u.pathname,root)));}catch{return new Response('',{status:404});}};
const workerFactory=url=>{const real=new URL('web'+url.pathname,root);const thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(String(real))});parentPort.on('message',data=>globalThis.onmessage({data}));`)));const adapter={postMessage:(message,...args)=>thread.postMessage(message.op==='create'?{...message,jit:false}:message,...args),terminate:()=>thread.terminate()};thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',error=>adapter.onerror?.(error));return adapter;};
const runtime=createPlatformRuntime({catalog,fetchImpl,workerFactory,baseUrl,assetBaseUrl:origin+'/',deviceStorage:createDeviceStorage({indexedDB:null,dbName:'corematrix-isolated-test'})});
const repl=createCoreMatrixRepl(runtime),frames=[],serialEvidence=[],outcomes=[],inputs=[],imuSamples=[],audioSamples=[];
const exec=async source=>{const out=await repl.exec(source);serialEvidence.push(...out.trim().split(/\r?\n/).filter(line=>/^CM_/.test(line)));return out;};
const capture=async label=>{await repl.advance(.025);const f=await runtime.capture({format:'rgba'});assert.equal(f.width,16);assert.equal(f.height,16);const pixels=new Uint8Array(f.pixels);const state=await runtime.observe();assert.equal(state.leds.length,256);for(const pixel of state.leds){assert.equal(pixel[0],pixel[1],'White LED output has equalR/G');assert.equal(pixel[0],pixel[2],'White LED output has equalR/B');}frames.push({label,width:f.width,height:f.height,rgbaSha256:hash(pixels)});return pixels;};
const lit=pixels=>Array.from({length:256},(_,i)=>i).filter(i=>pixels[i*4]>0||pixels[i*4+1]>0||pixels[i*4+2]>0);
const expectLit=(pixels,indices)=>{assert.deepEqual(lit(pixels),indices.toSorted((a,b)=>a-b));for(let i=0;i<256;i++){assert.ok(Math.abs(pixels[4*i]-pixels[4*i+1])<=4,'White RGB565 frame has at most4levels R/G quantization difference');assert.equal(pixels[4*i],pixels[4*i+2],'White matrix has equal R/B');}};
let failure=null;
try {
 const scene=createDefaultScene(catalog,'m5stack-c158');scene.firmware=null;await runtime.configure(scene);await runtime.loadFirmware({file:new File([bytes],'original-corematrix.bin')});await runtime.start();
 await repl.wait(state=>/MicroPython|UIFLOW|UIFlow|M5STACK|M5Stack|CoreMatrix/.test(state.serial)&&state.stats.time>1);
 await repl.enter();
 const ready=await exec(await readFile(new URL('tools/fixtures/corematrix-uiflow/exercise.py',root),'utf8'));
 assert.match(ready,/CM_READY 152 16 16/);outcomes.push('official-image-boot-and-MicroPython-guest-execution');
 if(checks.includes('matrix')) {
  await exec('cm_matrix()');const full=await capture('full-white');expectLit(full,Array.from({length:256},(_,i)=>i));
  const points=[[0,0],[3,2],[4,3],[7,4],[8,5],[11,6],[12,7],[15,15]],expected=points.map(([x,y])=>y*16+x);
  await exec(`cm_matrix(${JSON.stringify(points)})`);const sparse=await capture('asymmetric-pixels');expectLit(sparse,expected);
  await exec('cm_brightness(16)');const dim=await capture('low-duty');expectLit(dim,expected);assert.ok(dim[0]<sparse[0],'Firmware PWM command must change brightness');
  await exec('cm_brightness(0)');expectLit(await capture('brightness-off'),[]);
  await exec('cm_brightness(255)');expectLit(await capture('brightness-restore'),expected);
  assert.match(await exec('cm_wrong_address()'),/CM_WRONG_ADDRESS_NACK/);expectLit(await capture('wrong-address-unchanged'),expected);
  await exec('cm_led_power(False)');expectLit(await capture('supply-off'),[]);
  await exec('cm_led_power(True)');expectLit(await capture('supply-restored-before-write'),[]);
  await exec(`cm_reinitialize_matrix();cm_matrix(${JSON.stringify(points)})`);expectLit(await capture('supply-restored-new-frame'),expected);
  outcomes.push('real-TM1680-256-white-pixels-coordinate-halves-PWM-power-and-wrong-address');
 }
 if(checks.includes('buttons')) {
  assert.match(await exec('cm_buttons()'),/CM_BUTTONS 0 0 0/);
  for(const [i,id] of ['button-a','button-b','button-c'].entries()) {
   inputs.push(await runtime.input({type:'button',id,pressed:true}));const expected=[0,0,0];expected[i]=1;assert.ok((await exec('cm_buttons()')).includes('CM_BUTTONS '+expected.join(' ')));
   inputs.push(await runtime.input({type:'button',id,pressed:false}));assert.match(await exec('cm_buttons()'),/CM_BUTTONS 0 0 0/);
  }
  outcomes.push('PM1-three-buttons-press-and-release');
 }
 if(checks.includes('imu')) {
  for(const quaternion of [[0,0,0,1],[0,0,Math.SQRT1_2,Math.SQRT1_2],[Math.SQRT1_2,0,0,Math.SQRT1_2]]) {
   await runtime.setImu({enabled:true,source:'agent',quaternion});await repl.advance(.1);
   const out=await exec('cm_imu()'),sample=JSON.parse(out.match(/CM_IMU_JSON ([^\r\n]+)/)?.[1]||'null');assert.equal(sample?.enabled,true);
   const raw=Array.from({length:3},(_,i)=>{const word=sample.raw[2*i]|sample.raw[2*i+1]<<8;return (word>=32768?word-65536:word)*(2<<sample.range)/32768;});
   const corrected=[raw[1],-raw[0],raw[2]];for(let i=0;i<3;i++)assert.ok(Math.abs(sample.accel[i]-corrected[i])<.003,'Pinned M5Unified CoreMatrix raw-to-API IMU transform');
   imuSamples.push({quaternion,...sample,rawAccelG:raw,expectedApiAccelG:corrected});
  }
  assert.equal(new Set(imuSamples.map(s=>JSON.stringify(s.raw))).size,3,'All three injected orientations must change BMI270 guest bytes');outcomes.push('BMI270-three-axes-raw-and-M5Unified-correction');
 }
 if(checks.includes('sd')) {assert.match(await exec('cm_sd()'),/CM_SD WRITE_READ/);outcomes.push('SPI-SD-FAT-write-and-read');}
 if(checks.includes('buzzer')) {
  await runtime.audioConfigure({output:{enabled:true}});await runtime.readAudio();
  assert.match(await exec('cm_buzzer_pwm()'),/CM_PWM 1200 34816/);await repl.advance(.1);
  const audio=await runtime.readAudio(),samples=audio.chunks.flatMap(chunk=>Array.from(chunk.samples));assert.ok(samples.some(x=>x!==0),'IOE1 PWM must produce real audio output');
  let edges=0;for(let i=1;i<samples.length;i++)if((samples[i]>=0)!==(samples[i-1]>=0))edges++;
  const rates=[...new Set(audio.chunks.map(chunk=>chunk.sampleRate))];assert.deepEqual(rates,[48000]);const measuredHz=edges*48000/(2*samples.length);assert.ok(Math.abs(measuredHz-1200)<100,'PWM audio frequency must follow guest setting');
  audioSamples.push({phase:'pwm1200',frames:samples.length,sampleRate:48000,signEdges:edges,measuredHz,pcmSha256:hash(new Int16Array(samples))});
  await exec('cm_buzzer_power(False)');await runtime.readAudio();await repl.advance(.1);const silent=await runtime.readAudio();assert.ok(silent.chunks.every(chunk=>chunk.samples.every(x=>x===0)),'Buzzer supply-off must suppress fresh output');
  await exec('cm_buzzer_power(True);cm_buzzer()');await repl.advance(.1);const tone=await runtime.readAudio();assert.ok(tone.chunks.some(chunk=>chunk.samples.some(x=>x!==0)),'Official M5.Speaker.tone must generate guest audio');
  await exec('cm_buzzer_power(False)');outcomes.push('guest-IOE1-PWM-and-M5Speaker-real-audio-with-power-gate');
 }
 if(checks.includes('power')) {assert.match(await exec('cm_power()'),/CM_POWER/);outcomes.push('guest-power-register-read');}
 await runtime.pause();
} catch(error) {failure=String(error.message);process.exitCode=1;}
finally {
 const state=await runtime.observe().catch(()=>({}));
 const receipt={checkedAt:new Date().toISOString(),productId:'m5stack-c158',source:{sha256,bytes:bytes.length,kind:'caller-supplied-local-official-image'},wasmSha256:await readFile(new URL('web/wasm/esp32sim.wasm',root)).then(hash).catch(()=>null),backend:'production-page-worker-WASM-interpreter',firmwarePatched:false,guestProgram:'RAM-only MicroPython raw-REPL source; original boot image and filesystem are unchanged.',status:failure?'failed':'pass',requestedChecks:checks,checks:outcomes,frames,inputs,imuSamples,audioSamples,serialEvidence,guestSeconds:state.stats?.time,failure,limitations:['No physical device tested.','Buzzer check measures digital PWM frequency; audio timbre and physical loudness are not calibrated. Power readback is not analog calibration.','SD check writes and reads an isolated virtual card; reset persistence requires a separate run.','Source-program checks do not prove all unmodified UIFlow application features or wireless connectivity.']};
 if(option('--receipt'))await writeFile(option('--receipt'),JSON.stringify(receipt,null,2)+'\n');
 console.log(JSON.stringify(receipt,null,2));await runtime.dispose();
}
