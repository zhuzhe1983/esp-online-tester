#!/usr/bin/env node
// Actual UART-controlled guest closed loop. Host observes plant truth but never supplies control outputs.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import {createHash} from 'node:crypto';
import {Worker} from 'node:worker_threads';
const arg=k=>process.argv[process.argv.indexOf(k)+1];
for(const k of ['--firmware','--assets-root'])assert(process.argv.includes(k),k+' is required');
const root=pathToFileURL(resolve(arg('--assets-root'))+'/'),origin='https://stampfly-flight-demo.invalid',bytes=new Uint8Array(await readFile(arg('--firmware'))),hash=b=>createHash('sha256').update(b).digest('hex');
const {createPlatformRuntime}=await import(new URL('web/platform/runtime.js',root));
const {createDefaultScene}=await import(new URL('web/platform/catalog.js',root));
const {createDeviceStorage}=await import(new URL('web/platform/device-storage.js',root));
const catalog={};for(const k of ['products','accessories','firmwares'])catalog[k]=JSON.parse(await readFile(new URL(`web/platform/catalog/${k}.json`,root))).items;
const fetchImpl=async value=>{const u=new URL(value);assert.equal(u.origin,origin);try{return new Response(await readFile(new URL('web'+u.pathname,root)));}catch{return new Response('',{status:404});}};
const scenario=process.argv.includes('--scenario')?arg('--scenario'):'controls';
assert(['controls','bounds','tof-loss'].includes(scenario),'Unknown scenario');
const runnerUrl=new URL('web/wasm/stampfly-flight-runner.js',root),runnerSource=await readFile(runnerUrl,'utf8');
const workerUrl=new URL('web/wasm/worker.js',root),workerSource=await readFile(workerUrl,'utf8');
const faultFind='const s=plant.sensors();',faultReplacement='const s=plant.sensors();if(next/clockHz>=10)s.bottomDistanceM=null;';
let workerModule=String(workerUrl);
if(scenario==='tof-loss'){
 assert.equal(runnerSource.split(faultFind).length,2,'Fault insertion must match exactly once');
 const faultRunner=runnerSource.replace(faultFind,faultReplacement).replace(/from (['\"])(\.\/[^'\"]+)\1/g,(_,q,path)=>'from '+JSON.stringify(String(new URL(path,runnerUrl))));
 const faultUrl='data:text/javascript,'+encodeURIComponent(faultRunner);
 workerModule='data:text/javascript,'+encodeURIComponent(workerSource.replace(/from (['\"])(\.\/[^'\"]+)\1/g,(_,q,path)=>'from '+JSON.stringify(path==='./stampfly-flight-runner.js'?faultUrl:String(new URL(path,workerUrl)))));
}
const workerFactory=url=>{assert.equal(String(new URL('web'+url.pathname,root)),String(workerUrl));const real=workerModule,thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(d,t)=>parentPort.postMessage(d,t);await import(${JSON.stringify(String(real))});parentPort.on('message',d=>globalThis.onmessage({data:d}));`)));const w={postMessage:(...a)=>thread.postMessage(...a),terminate:()=>thread.terminate()};thread.on('message',data=>w.onmessage?.({data}));thread.on('error',e=>w.onerror?.(e));return w;};
const api=createPlatformRuntime({catalog,fetchImpl,workerFactory,baseUrl:origin+'/platform/',assetBaseUrl:origin+'/',deviceStorage:createDeviceStorage({indexedDB:null,dbName:'stampfly-flight-demo-proof'})});
const delay=ms=>new Promise(r=>setTimeout(r,ms)),uart=s=>s.serialByPort.uart0||'';
const receipt={checkedAt:new Date().toISOString(),method:'full-ROM guest UART/SPI/I2C/LEDC with opt-in rigid-body model',firmwareSha256:hash(bytes),wasmSha256:hash(await readFile(new URL('web/wasm/esp32sim.wasm',root))),product:process.argv.includes('--product')?arg('--product'):'m5stack-k138',scenario,sourceHashes:{worker:hash(workerSource),runner:hash(runnerSource),plant:hash(await readFile(new URL('web/wasm/stampfly-flight.js',root))),parameters:hash(await readFile(new URL('web/wasm/stampfly-parameters.js',root)))},samples:[],commands:[],status:'running'};
if(scenario==='tof-loss')receipt.testOnlyStimulus={atGuestTime:10,bottomTarget:null,find:faultFind,replace:faultReplacement,modifiedRunnerSha256:hash(runnerSource.replace(faultFind,faultReplacement)),scope:'Test-only ESM transform; unchanged firmware, WASM, ST algorithm, physics and guest PWM. Production assets are not modified.'};
let lastSample=-1;
const euler=q=>{const [x,y,z,w]=q;return [Math.atan2(2*(w*x+y*z),1-2*(x*x+y*y)),Math.asin(Math.max(-1,Math.min(1,2*(w*y-z*x))))].map(v=>v*180/Math.PI);};
function windowSamples(end,seconds){const values=receipt.samples.filter(s=>s.time>=end-seconds&&s.time<=end);assert(values.length>=5,'Too few actual published samples');return values;}
function heightGate(label,end,target){const values=windowSamples(end,4),heights=values.map(s=>s.height),maximumError=Math.max(...heights.map(h=>Math.abs(h-target))),span=Math.max(...heights)-Math.min(...heights);assert(maximumError<.06,label+' height error '+maximumError);assert(span<.06,label+' height span '+span);assert(values.every(s=>!s.contact&&s.duties.every(d=>d>0)),label+' lost flight');receipt.checks.push({label,target,maximumError,span,sampleCount:values.length});}
function attitudeGate(label,end,roll,pitch,yaw){const values=windowSamples(end,1),errors=values.map(s=>{const [r,p]=euler(s.attitude);return [Math.abs(r-roll),Math.abs(p-pitch),Math.abs(s.omega[2]-yaw)];}),maximumErrors=[0,1,2].map(i=>Math.max(...errors.map(e=>e[i])));assert(maximumErrors[0]<2&&maximumErrors[1]<2&&maximumErrors[2]<(yaw===0?5:10),label+' attitude errors '+maximumErrors);assert(values.every(s=>Math.abs(s.omega[0])<20&&Math.abs(s.omega[1])<20),label+' residual roll/pitch rate');assert(values.every(s=>Math.abs(s.height-.6)<.12),label+' height excursion');receipt.checks.push({label,target:[roll,pitch,yaw],maximumErrors,sampleCount:values.length});}
receipt.checks=[];
async function observe(){const s=await api.observe();if(s.flight&&s.flight.guestTime>=lastSample+.1){lastSample=s.flight.guestTime;receipt.samples.push({time:s.flight.guestTime,height:s.flight.heightM,position:s.flight.positionM,velocity:s.flight.velocityMps,attitude:s.flight.quaternionXyzw,omega:s.flight.angularVelocityDpsFRD,contact:s.flight.contact,duties:s.flight.motors.map(m=>m.duty),bottomDistance:s.flight.sensors.bottomDistanceM,guest:uart(s).split(/\r?\n/).filter(l=>l.startsWith('SF1 STATE ')).at(-1),sensor:uart(s).split(/\r?\n/).filter(l=>l.startsWith('SF1 SENSOR ')).at(-1)});}return s;}
async function wait(label,predicate){const end=Date.now()+180000;while(Date.now()<end){const s=await observe();if(predicate(s))return s;if(s.status==='error')throw Error(JSON.stringify(s.lastError));await delay(10);}throw Error(label+' timeout');}
async function until(time){return wait('guest time '+time,s=>s.flight?.guestTime>=time);}
async function command(line,expected){const at=uart(await observe()).length;await api.input({type:'serial',port:'uart0',data:line+'\n'});const s=await wait(line,s=>uart(s).slice(at).includes(expected||'SF1 ACK '+line.split(' ')[1])||uart(s).slice(at).includes('SF1 ERR '));if(uart(s).slice(at).includes('SF1 ERR '))throw Error('Guest rejected '+line+': '+uart(s).slice(at));process.stderr.write(line+' @ '+s.flight.guestTime+' h='+s.flight.heightM+'\n');receipt.commands.push({line,time:s.flight.guestTime,response:uart(s).slice(at).split(/\r?\n/).filter(l=>/^SF1 (ACK|ERR)/.test(l))});return s;}
try{
 const scene=createDefaultScene(catalog,receipt.product);scene.firmware=null;scene.flight={enabled:true,model:'stampfly-rigid-body-v1'};await api.configure(scene);await api.loadFirmware({file:new File([bytes],'stampfly-uart-demo.bin')});await api.start();
 let s=await wait('READY and fresh IMU',s=>uart(s).includes('SF1 READY stampfly-uart-demo 0.1.0')&&/SF1 STATE STOP [^\n]+ 1 [01]/.test(uart(s)));assert(Math.abs(s.flight.heightM)<1e-8);assert(s.flight.motors.every(m=>m.duty===0));
 s=await command('SF1 TAKEOFF '+(scenario==='bounds'?'0.1':'0.35'));let end=s.flight.guestTime+12;
 if(scenario==='tof-loss'){
   s=await until(9.5);assert(s.flight.heightM>.25&&s.flight.motors.every(m=>m.duty>.4),'Feedback must be established before target loss');assert(/SF1 STATE AUTO .* 1 1$/.test(uart(s).split(/\r?\n/).filter(l=>l.startsWith('SF1 STATE ')).at(-1)),'Both sensors must be valid before target loss');
   const stopped=await wait('actual ToF loss stops guest',s=>s.flight.guestTime>10&&s.flight.motors.every(m=>m.duty===0)&&uart(s).split(/\r?\n/).filter(l=>l.startsWith('SF1 STATE ')).at(-1)?.startsWith('SF1 STATE STOP '));
   assert(stopped.flight.guestTime<=10.8,'ToF loss did not stop within expiry plus reporting interval');receipt.checks.push({label:'ToF target removed after established hover; actual ST no-object result stops routed PWM',stopAt:stopped.flight.guestTime});await until(stopped.flight.guestTime+2);
 }else{
   await until(end);heightGate('takeoff settle',end,scenario==='bounds'?.1:.35);
   s=await command('SF1 TARGET '+(scenario==='bounds'?'1.5':'0.6')+' 0 0 0');end=s.flight.guestTime+(scenario==='bounds'?16:10);await until(end);heightGate('height target settle',end,scenario==='bounds'?1.5:.6);
   if(scenario==='controls'){
     s=await command('SF1 TARGET 0.6 5 -3 20');end=s.flight.guestTime+3;await until(end);attitudeGate('combined tilt/yaw',end,5,-3,20);
     s=await command('SF1 TARGET 0.6 0 0 0');end=s.flight.guestTime+4;await until(end);attitudeGate('return level',end,0,0,0);
   }
   s=await command('SF1 LAND');await until(s.flight.guestTime+(scenario==='bounds'?14:8));
 }
 await api.pause();s=await observe();receipt.final={status:s.status,flight:s.flight,serial:uart(s).split(/\r?\n/).filter(l=>l.startsWith('SF1 '))};
 if(scenario==='tof-loss'){assert(receipt.final.serial.some(l=>/SF1 SENSOR .* tof_status=255 tof_mm=-1$/.test(l)),'Actual ST no-object telemetry must be observed');assert(/SF1 STATE STOP .* 1 0$/.test(receipt.final.serial.filter(l=>l.startsWith('SF1 STATE ')).at(-1)),'Only ToF feedback should have expired');}
 assert(s.flight.contact,'Final vehicle must contact the ground');assert(s.flight.motors.every(m=>m.duty===0),'Final PWM must be exactly zero');assert(receipt.final.serial.filter(l=>l.startsWith('SF1 STATE ')).at(-1).startsWith('SF1 STATE STOP '),'Final guest mode must be STOP');receipt.checks.push({label:'landed/contact with guest STOP and exact four-channel zero PWM'});receipt.status='passed';
 console.log(JSON.stringify(receipt,null,2));
}catch(e){receipt.status='failed';receipt.error={message:e.message,code:e.code};try{const s=await api.observe();receipt.failureState={flight:s.flight,serial:uart(s).split(/\r?\n/).filter(l=>l.startsWith('SF1 '))};}catch{}console.log(JSON.stringify(receipt,null,2));process.exitCode=1;}finally{await api.dispose();}
