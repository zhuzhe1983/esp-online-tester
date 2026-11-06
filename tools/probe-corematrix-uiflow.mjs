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
if(!file||!/^[a-f0-9]{64}$/.test(sha256||''))throw Error('Supply <local-original-corematrix.bin> <sha256> [--checks matrix,buttons,imu,sd,buzzer,power,grove,network,ble,sleep] [--receipt file.json].');
const option=name=>{const i=args.indexOf(name);return i<0?undefined:args[i+1];};
const checks=(option('--checks')||'matrix,buttons,imu,sd,buzzer,power,grove,network,sleep').split(',');
assert.ok(checks.every(name=>['matrix','buttons','imu','sd','buzzer','power','grove','network','ble','sleep'].includes(name)),'Unknown check name');
const root=new URL('../',import.meta.url),origin='https://corematrix-fixture.invalid',baseUrl=origin+'/platform/';
const hash=bytes=>createHash('sha256').update(bytes).digest('hex');
const wasmSha256=await readFile(new URL('web/wasm/esp32sim.wasm',root)).then(hash);
const bytes=await readFile(file);assert.equal(hash(bytes),sha256,'Original firmware SHA-256 mismatch');
const catalog={};for(const name of ['products','accessories','firmwares'])catalog[name]=JSON.parse(await readFile(new URL(`web/platform/catalog/${name}.json`,root))).items;
const fetchImpl=async(value)=>{const u=new URL(value);assert.equal(u.origin,origin,'This probe uses local assets only');try{return new Response(await readFile(new URL('web'+u.pathname,root)));}catch{return new Response('',{status:404});}};
const workerFactory=url=>{const real=new URL('web'+url.pathname,root);const thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(String(real))});parentPort.on('message',data=>globalThis.onmessage({data}));`)));const adapter={postMessage:(message,...args)=>thread.postMessage(message.op==='create'?{...message,jit:false}:message,...args),terminate:()=>thread.terminate()};thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',error=>adapter.onerror?.(error));return adapter;};
const runtime=createPlatformRuntime({catalog,fetchImpl,workerFactory,baseUrl,assetBaseUrl:origin+'/',deviceStorage:createDeviceStorage({indexedDB:null,dbName:'corematrix-isolated-test'})});
const repl=createCoreMatrixRepl(runtime),frames=[],serialEvidence=[],outcomes=[],inputs=[],imuSamples=[],audioSamples=[],networkSamples=[],bleSamples=[];
const exec=async source=>{const out=await repl.exec(source);serialEvidence.push(...out.trim().split(/\r?\n/).filter(line=>/^CM_/.test(line)));return out;};
const capture=async label=>{await repl.advance(.025);const f=await runtime.capture({format:'rgba'});assert.equal(f.width,16);assert.equal(f.height,16);const pixels=new Uint8Array(f.pixels);const state=await runtime.observe();assert.equal(state.leds.length,256);for(const pixel of state.leds){assert.equal(pixel[0],pixel[1],'White LED output has equalR/G');assert.equal(pixel[0],pixel[2],'White LED output has equalR/B');}frames.push({label,width:f.width,height:f.height,rgbaSha256:hash(pixels)});return pixels;};
const lit=pixels=>Array.from({length:256},(_,i)=>i).filter(i=>pixels[i*4]>0||pixels[i*4+1]>0||pixels[i*4+2]>0);
const expectLit=(pixels,indices)=>{assert.deepEqual(lit(pixels),indices.toSorted((a,b)=>a-b));for(let i=0;i<256;i++){assert.ok(Math.abs(pixels[4*i]-pixels[4*i+1])<=4,'White RGB565 frame has at most4levels R/G quantization difference');assert.equal(pixels[4*i],pixels[4*i+2],'White matrix has equal R/B');}};
let failure=null;
try {
 const scene=createDefaultScene(catalog,'m5stack-c158');scene.firmware=null;
 if(checks.includes('grove'))scene.accessories=[{id:'hub',catalogId:'m5stack-unit-hub-u006',parameters:{},wiring:{mode:'physical',interfaceId:'grove'}},{id:'env',catalogId:'m5stack-unit-env4-u001d',parameters:{temperatureC:22,humidityPercent:60,pressureHpa:990},wiring:{mode:'physical',viaAccessoryId:'hub',portId:'port-1'}},{id:'light',catalogId:'m5stack-unit-dlight-u136',parameters:{lux:1000},wiring:{mode:'physical',viaAccessoryId:'hub',portId:'port-2'}}];
 await runtime.configure(scene);await runtime.configureNetwork({autoProvision:false});await runtime.loadFirmware({file:new File([bytes],'original-corematrix.bin')});await runtime.start();
 await repl.wait(state=>/MicroPython|UIFLOW|UIFlow|M5STACK|M5Stack|CoreMatrix/.test(state.serial)&&state.stats.time>1);
 await repl.enter();
 const ready=await exec(await readFile(new URL('tools/fixtures/corematrix-uiflow/exercise.py',root),'utf8'));
 assert.match(ready,/CM_READY 152 16 16/);
 await assert.rejects(()=>repl.exec('raise ValueError("CM_EXPECTED_ERROR")'),/CM_EXPECTED_ERROR/);
 outcomes.push('official-image-boot-and-MicroPython-guest-execution');
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
  assert.match(await exec('cm_buzzer_pwm()'),/CM_PWM 1200 34816/);await runtime.readAudio();await repl.advance(.1);
  const audio=await runtime.readAudio(),samples=audio.chunks.flatMap(chunk=>Array.from(chunk.samples));assert.ok(samples.some(x=>x!==0),'IOE1 PWM must produce real audio output');
  const active=samples.slice(samples.findIndex(x=>x!==0));assert.ok(active.every(x=>x!==0),'Enabled PWM must have continuous bipolar output');
  let edges=0;for(let i=1;i<active.length;i++)if((active[i]>=0)!==(active[i-1]>=0))edges++;
  const rates=[...new Set(audio.chunks.map(chunk=>chunk.sampleRate))];assert.deepEqual(rates,[48000]);const measuredHz=edges*48000/(2*active.length);assert.ok(Math.abs(measuredHz-1200)<15,'PWM audio frequency must follow guest setting');
  audioSamples.push({phase:'pwm1200',frames:samples.length,activeFrames:active.length,sampleRate:48000,signEdges:edges,measuredHz,pcmSha256:hash(new Int16Array(samples))});
  await exec('cm_buzzer_power(False)');await repl.advance(.04);await runtime.readAudio();await repl.advance(.1);const silent=await runtime.readAudio();assert.ok(silent.chunks.every(chunk=>chunk.samples.every(x=>x===0)),'Buzzer supply-off must suppress fresh output: '+JSON.stringify(silent.chunks.map(c=>({guestTime:c.guestTime,frames:c.samples.length,active:c.samples.filter(x=>x!==0).length}))));
  await exec('cm_buzzer_power(True);cm_buzzer()');await repl.advance(.1);const tone=await runtime.readAudio();assert.ok(tone.chunks.some(chunk=>chunk.samples.some(x=>x!==0)),'Official M5.Speaker.tone must generate guest audio');
  await exec('cm_buzzer_power(False)');outcomes.push('guest-IOE1-PWM-and-M5Speaker-real-audio-with-power-gate');
 }
 if(checks.includes('power')) {const power=await exec('for _ in range(3):\n cm_power()\n time.sleep_ms(1300)');assert.match(power,/CM_POWER 3900 5000 True/);outcomes.push('guest-power-register-read-after-battery-detection');}
 if(checks.includes('grove')) {
  // The shared exercise already initialized M5. The standalone Grove fixture
  // calls begin for its own native entry; do not initialize the board twice.
  const source=(await readFile(new URL('tools/fixtures/corematrix-uiflow/grove.py',root),'utf8')).replace('\nM5.begin()\n','\n');
  const result=await exec(source);
  for(const marker of ['CM_GROVE_SAMPLE','CM_GROVE_WRONG_ADDRESS_PINS_NACK','CM_GROVE_POWER_RESET_PASS'])assert.ok(result.includes(marker));
  outcomes.push('routed-Grove-ENV4-DLight-Hub-wrong-pins-address-and-switched-power');
 }
 if(checks.includes('network')) {
  const source=await readFile(new URL('tools/fixtures/corematrix-uiflow/softap.py',root),'utf8');
  const start=await repl.submit(source);
  await repl.wait(state=>state.serial.slice(start).includes('CM_SOFTAP_READY'));
  await repl.wait(state=>state.network?.interfaces?.[1]?.leaseReady);
  const response=await runtime.requestNetworkHttp({interface:1,guestPort:18080,path:'/corematrix',method:'GET'});
  assert.equal(response.status,200);assert.equal(response.body,'COREMATRIX_HTTP_'.repeat(256));
  const done=await repl.wait(state=>state.serial.slice(start).includes('CM_SOFTAP_HTTP_PASS')&&state.serial.slice(start).includes('\x04\x04>'));
  assert.ok(!done.serial.slice(start).includes('Traceback'));serialEvidence.push('CM_SOFTAP_HTTP_PASS');
  networkSamples.push({protocol:'SoftAP-DHCP-TCP-HTTP',responseBytes:response.body.length,responseSha256:hash(response.body),state:await runtime.readNetwork()});
  await exec('ap.active(False)');outcomes.push('browser-SoftAP-DHCP-TCP-HTTP-guest-response-byte-comparison');
 }
 if(checks.includes('ble')) {
  const spec='name=CoreMatrix-peer,addr=12:34:56:78:9a:bc,type=public,rssi=-42';
  await runtime.configureBle({nearby:[spec]});
  await assert.rejects(()=>runtime.configureBle({nearby:['name=bad,type=unknown']}),{code:'BLE_CONFIG_REJECTED'});
  await assert.rejects(()=>runtime.configureBle({nearby:[spec,spec]}),{code:'BLE_CONFIG_REJECTED'});
  await assert.rejects(()=>runtime.configureBle({nearby:['name=x\nname=y']}),{code:'INVALID_BLE'});
  const out=await exec(await readFile(new URL('tools/fixtures/corematrix-uiflow/ble.py',root),'utf8'));
  const scans=JSON.parse(out.match(/CM_BLE_SCAN_JSON ([^\r\n]+)/)?.[1]||'null');assert.ok(scans?.length>0,'Real guest scan callback must receive nearby advertising bytes');
  for(const sample of scans)assert.deepEqual(sample,[0,'123456789abc',2,-42,'0201061009436f72654d61747269782d70656572']);
  const radio=await runtime.readBle();assert.ok(radio.packets.length>=6,'At least two complete three-channel advertising events');
  for(const [i,packet] of radio.packets.entries()) {assert.equal(packet.channel,37+i%3);assert.equal(packet.pduHex.slice(0,4),'200e');assert.equal(packet.pduHex.slice(16),'0201060409433631');}
  const starts=radio.packets.filter(p=>p.channel===37).map(p=>p.timestampUs),intervals=starts.slice(1).map((t,i)=>(t-starts[i])>>>0);assert.ok(intervals.every(us=>us>=100000&&us<=110000));
  await runtime.configureBle({nearby:[]});const empty=await exec('cm_ble_scan();print("CM_BLE_EMPTY",len(cm_ble_scans));b.active(False)');assert.match(empty,/CM_BLE_EMPTY 0/);
  bleSamples.push({scanCallbacks:scans,advertisements:radio.packets,intervalsUs:intervals,removedPeersProduceNoScanResult:true});
  outcomes.push('BLE-original-controller-scan-callbacks-and-three-channel-DMA-advertisements');
 }
 if(checks.includes('sleep')) {
  const light=await exec('import machine,time\nt=time.ticks_ms()\nmachine.lightsleep(200)\nprint("CM_LIGHT_SLEEP",time.ticks_diff(time.ticks_ms(),t))');
  const elapsed=Number(light.match(/CM_LIGHT_SLEEP (\d+)/)?.[1]);assert.ok(elapsed>=180&&elapsed<=220,'Timer light sleep must resume at 200 ms');
  outcomes.push('PMU-timed-light-sleep-resumes');
  const start=await repl.submit('import machine\nprint("CM_DEEP_START")\nmachine.deepsleep(200)\nprint("CM_DEEP_MUST_NOT_RETURN")');
  const reboot=await repl.wait(state=>{const out=state.serial.slice(start);return out.includes('ESP-ROM:esp32c61')&&out.includes('>>>');});
  assert.ok(!reboot.serial.slice(start).includes('CM_DEEP_MUST_NOT_RETURN'),'Deep sleep must reboot instead of returning');
  await repl.enter();assert.match(await exec('import machine\nprint("CM_DEEP_RESET",machine.reset_cause()==machine.DEEPSLEEP_RESET)'),/CM_DEEP_RESET True/);
  outcomes.push('PMU-timed-deep-sleep-ROM-reboot-and-guest-reset-cause');
 }
 if(checks.includes('sd')) {
  for(const kind of ['reset','reload']) {
   if(kind==='reset')await runtime.reset();else {await runtime.loadFirmware({file:new File([bytes],'original-corematrix.bin')});await runtime.start();}
   await repl.wait(state=>state.serial.includes('MicroPython')&&state.stats.time>1);await repl.enter();
   await exec(await readFile(new URL('tools/fixtures/corematrix-uiflow/exercise.py',root),'utf8'));
   assert.match(await exec('cm_sd(False)'),/CM_SD PERSISTED_READ/);outcomes.push('SPI-SD-file-preserved-after-'+kind);
  }
 }
 await runtime.pause();
} catch(error) {failure=String(error.message);process.exitCode=1;}
finally {
 const state=await runtime.observe().catch(()=>({}));
 const receipt={checkedAt:new Date().toISOString(),productId:'m5stack-c158',source:{sha256,bytes:bytes.length,kind:'caller-supplied-local-official-image'},wasmSha256,backend:'production-page-worker-WASM-interpreter',firmwarePatched:false,guestProgram:'RAM-only MicroPython raw-REPL source; original boot image and filesystem are unchanged.',status:failure?'failed':'pass',requestedChecks:checks,checks:outcomes,frames,inputs,imuSamples,audioSamples,networkSamples,bleSamples,serialEvidence,guestSeconds:state.stats?.time,...(failure?{guestSerialTail:state.serial?.slice(-3000)}:{}),hardwareDiagnostics:state.hardwareDiagnostics,audioState:state.audio,sdState:state.sdCard,failure,limitations:['No physical device tested.','Buzzer check measures digital PWM frequency; audio timbre and physical loudness are not calibrated. Power readback is not analog calibration.','SD uses an isolated virtual card; reset and firmware reload are tested, not abrupt host/browser crashes.','Source-program checks do not prove all unmodified UIFlow application features or physical wireless connectivity.']};
 if(option('--receipt'))await writeFile(option('--receipt'),JSON.stringify(receipt,null,2)+'\n');
 console.log(JSON.stringify(receipt,null,2));await runtime.dispose();
}
