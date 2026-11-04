import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {runInNewContext} from 'node:vm';
import * as THREE from '../web/platform/vendor/three.module.min.js';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDefaultScene} from '../web/platform/catalog.js';
import {parseMotorSnapshot,STAMPFLY_MOTORS} from '../web/platform/motor-state.js';
import {createMotorRotors,applyMotorValues} from '../web/platform/motor-view.js';
import {createManufacturerModel} from '../web/platform/manufacturer-model.js';
import {createPacing} from '../web/wasm/pacing.mjs';
import {applyExperiments} from '../web/wasm/experiments.mjs';

const publicCatalog=Object.fromEntries(await Promise.all(['products','accessories','firmwares'].map(async name=>[name,JSON.parse(await readFile(new URL(`../web/platform/catalog/${name}.json`,import.meta.url))).items])));
const catalog=structuredClone(publicCatalog),original=catalog.products.find(p=>p.id==='m5stack-k138');
original.runtime={board:'stampfly',flashMb:8,psramMb:0,status:'partial'};
// These transport fixtures do not certify any real firmware or catalog support.
catalog.products=catalog.products.filter(p=>p.id!=='m5stack-k138-v11');
catalog.products.push({...structuredClone(original),id:'m5stack-k138-v11',runtime:{...original.runtime,board:'stampfly-v11'}});
const motion=(cycle=240000000,version=1)=>({t:'motion',type:'stampfly-motors',clockHz:240000000,cycle,version,model:'latched-pwm',flightSimulated:false,
  motors:STAMPFLY_MOTORS.map(({id,gpio},i)=>({id,gpio,duty:(i+1)/4,frequencyHz:150000}))});
function fixture(){
 const workers=[],events=[],messages=[];
 const workerFactory=()=>{const w={terminate(){},postMessage(m){messages.push(m);let r;
  if(m.op==='init')r={ready:true};if(m.op==='create')r={created:true,flashStorage:false};if(m.op==='load')r={loaded:m.at===undefined?m.kind:`at${m.at}`,ok:true};
  if(m.op==='start')r={started:true};if(m.op==='resume')r={resumed:true};if(m.op==='stop')r={stopped:0};
  if(m.op==='imu')r={imuApplied:m.inputId,ok:true,time:0};
  if(r)queueMicrotask(()=>w.onmessage?.({data:r}));}};workers.push(w);return w;};
 const runtime=createPlatformRuntime({catalog,workerFactory,onEvent:e=>events.push(e),fetchImpl:async()=>new Response(new Uint8Array([1,2,3])),baseUrl:'https://fixture.invalid/platform/',assetBaseUrl:'https://fixture.invalid/'});
 const bytes=new Uint8Array(0x10100),view=new DataView(bytes.buffer);
 for(const at of [0,0x10000]){bytes[at]=0xe9;bytes[at+1]=1;bytes[at+12]=9;view.setUint32(at+28,4,true);}bytes[0x8000]=0xaa;bytes[0x8001]=0x50;view.setUint32(0x8004,0x10000,true);view.setUint32(0x8008,0x100000,true);
 return {runtime,events,workers,messages,load:()=>runtime.loadFirmware({file:new Blob([bytes])}),send:(value,w=workers.at(-1))=>w.onmessage({data:{text:JSON.stringify(value)}})};
}

test('motor snapshots enforce wiring, bounded outputs and the explicit guest clock without granting flight state',()=>{
 const value=motion(),parsed=parseMotorSnapshot(value);assert.equal(parsed.motorOutput.guestTime,1);assert.deepEqual(parsed.motors,value.motors);
 parsed.motors[0].duty=0;assert.equal(value.motors[0].duty,.25);
 for(const change of [v=>v.type='servos',v=>v.model='rpm',v=>v.flightSimulated=true,v=>v.clockHz=0,v=>v.clockHz=80e6,v=>v.cycle=-1,v=>v.cycle=Number.MAX_SAFE_INTEGER+1,v=>v.version=NaN,
  v=>v.motors.pop(),v=>v.motors[1].id='front-left',v=>v.motors[0].gpio=42,v=>v.motors[0].duty=-.1,v=>v.motors[0].duty=1.01,v=>v.motors[0].duty=null,
  v=>v.motors[0].frequencyHz=-1,v=>v.motors[0].frequencyHz=Infinity,v=>v.motors[0].frequencyHz=0]){const bad=motion();change(bad);assert.equal(parseMotorSnapshot(bad),null);}
 const off=motion();off.motors.forEach(m=>{m.duty=0;m.frequencyHz=0;});assert.ok(parseMotorSnapshot(off));
 const extra=motion();extra.motors[0].rpm=10000;extra.position=[1,2,3];assert.equal(parseMotorSnapshot(extra).motors[0].rpm,undefined);
});

test('runtime protects motor observations across malformed messages, pause, reset, stale workers and unrelated boards',async()=>{
 const f=fixture();try{
  await f.runtime.configure(createDefaultScene(catalog,'m5stack-k138'));await f.load();await f.runtime.start();
  assert.equal((await f.runtime.capabilities()).motors.available,true);assert.equal((await f.runtime.capabilities()).motors.flightSimulated,false);
  f.runtime.subscribe(e=>{if(e.type==='motors'&&e.motors.length)e.motors[0].duty=1;});const received=[];f.runtime.subscribe(e=>{if(e.type==='motors')received.push(e);});
  f.send(motion());const state=await f.runtime.observe();assert.equal(state.motors[0].duty,.25);assert.equal(received.at(-1).motors[0].duty,.25);assert.equal(state.frame,null);
  state.motors[0].duty=0;state.motorOutput.guestTime=99;assert.equal((await f.runtime.observe()).motorOutput.guestTime,1);
  f.send(motion());assert.equal(received.length,1,'identical snapshots do not add visual frames');
  f.send(motion(120000000,2));f.send(motion(480000000,0));const bad=motion(480000000,2);bad.motors[0].duty=2;f.send(bad);assert.equal((await f.runtime.observe()).motorOutput.cycle,240000000);
  await assert.rejects(f.runtime.input({type:'motor',id:'front-left',duty:1}),{code:'UNSUPPORTED_INPUT'});
  await f.runtime.pause();assert.equal((await f.runtime.observe()).motors[0].duty,.25,'pause retains measured PWM, presentation freezes separately');
  const old=f.workers.at(-1);await f.runtime.reset();assert.deepEqual((await f.runtime.observe()).motors,[]);assert.equal((await f.runtime.observe()).motorOutput,null);
  f.send(motion(960000000,20),old);assert.deepEqual((await f.runtime.observe()).motors,[]);f.send(motion(0,0));assert.equal((await f.runtime.observe()).motorOutput.version,0);
  await f.runtime.configure(createDefaultScene(catalog,'m5stack-k138-v11'));assert.deepEqual((await f.runtime.observe()).motors,[]);assert.equal((await f.runtime.capabilities()).motors.available,true);await f.load();await f.runtime.start();f.send(motion());assert.equal((await f.runtime.observe()).motors.length,4);
  await f.runtime.configure(createDefaultScene(catalog,'m5stack-k134'));assert.deepEqual((await f.runtime.observe()).motors,[]);assert.equal((await f.runtime.capabilities()).motors.available,false);await f.load();f.send(motion());assert.deepEqual((await f.runtime.observe()).motors,[]);
 }finally{await f.runtime.dispose();}
});

test('both StampFly versions expose explicit BMI270 host input without a flight or scene mounting model',async()=>{
 const f=fixture();try{
  for(const id of ['m5stack-k138','m5stack-k138-v11']) {
   await f.runtime.configure(createDefaultScene(catalog,id));
   const cap=(await f.runtime.capabilities()).imu;assert.equal(cap.model,'bmi270-register-subset');assert.equal(cap.sceneOrientation,false);assert.equal(cap.physicalMountingVerified,false);
   await f.load();const result=await f.runtime.setImu({source:'agent',quaternion:[0,0,Math.SQRT1_2,Math.SQRT1_2]});
   assert.ok(Math.abs(result.accelG[0]-1)<1e-8);assert.ok(Math.abs(result.accelG[1])<1e-8);assert.equal(result.delivery,'register-state');
   assert.deepEqual(f.messages.at(-1).gyroDps,[0,0,0]);assert.equal(f.messages.at(-1).op,'imu');
   await assert.rejects(f.runtime.setImu({source:'scene',quaternion:[0,0,0,1]}),{code:'IMU_SCENE_UNSUPPORTED'});
  }
 }finally{await f.runtime.dispose();}
});

const world=object=>object.getWorldPosition(new THREE.Vector3());
test('rotor pivot preserves assembled geometry and advances only on guest-time PWM observations',()=>{
 const group=new THREE.Group(),blade=new THREE.Mesh(new THREE.BoxGeometry(8,2,.4),new THREE.MeshBasicMaterial());blade.name='blade';blade.position.set(24,10,3);group.add(blade);
 const base=world(blade),rotors=createMotorRotors(group,[{id:'front-left',axis:[0,0,1],pivotMm:[20,10,3],parts:['blade'],direction:-1}]);group.updateMatrixWorld(true);assert.ok(world(blade).distanceTo(base)<1e-8);
 let output=parseMotorSnapshot(motion());applyMotorValues(rotors,output.motors,output.motorOutput,{running:true});assert.ok(world(blade).distanceTo(base)<1e-8);
 output=parseMotorSnapshot(motion(480000000));applyMotorValues(rotors,output.motors,output.motorOutput,{running:true});group.updateMatrixWorld(true);const moved=world(blade);assert.ok(moved.distanceTo(base)>1);assert.ok(Math.abs(moved.distanceTo(new THREE.Vector3(20,10,3))-4)<1e-8);
 const pose=rotors[0].quaternion.clone();applyMotorValues(rotors,output.motors,output.motorOutput,{running:true});assert.ok(rotors[0].quaternion.equals(pose),'same guest time cannot spin');
 applyMotorValues(rotors,output.motors,output.motorOutput,{running:false});output=parseMotorSnapshot(motion(960000000));applyMotorValues(rotors,output.motors,output.motorOutput,{running:false});assert.ok(rotors[0].quaternion.equals(pose),'paused publication cannot spin');
 applyMotorValues(rotors,output.motors,output.motorOutput,{running:true});assert.ok(rotors[0].quaternion.equals(pose),'resume starts from current guest time');
 output.motors[0].duty=0;output.motors[0].frequencyHz=0;applyMotorValues(rotors,output.motors,output.motorOutput,{running:true});output.motorOutput.guestTime+=1;applyMotorValues(rotors,output.motors,output.motorOutput,{running:true});assert.ok(rotors[0].quaternion.equals(pose),'disabled PWM stops on following observations');
 applyMotorValues(rotors,[],null,{running:true});group.updateMatrixWorld(true);assert.ok(world(blade).distanceTo(base)<1e-8,'reset restores exact neutral assembly');
});

test('rotor declaration rejects duplicate ownership, absent geometry and invalid axes',()=>{
 const valid={id:'front-left',axis:[0,0,1],pivotMm:[0,0,0],parts:['blade']};
 for(const definition of [null,{...valid,id:'any'},{...valid,axis:[0,0,0]},{...valid,axis:[Infinity,0,0]},{...valid,pivotMm:[0,NaN,0]},{...valid,direction:0},{...valid,parts:['missing']},{...valid,parts:['blade','blade']}]) {
  const g=new THREE.Group(),b=new THREE.Object3D();b.name='blade';g.add(b);assert.throws(()=>createMotorRotors(g,[definition]),/Invalid/);
 }
});

test('manufacturer loader binds photo surfaces to the declared rotor without modifying source vertices',async()=>{
 const nativeFetch=globalThis.fetch,vertices=new Float32Array([21,10,3,0,0,1,25,10,3,0,0,1,23,12,3,0,0,1]);
 try{
  const metadata={unit:'mm',strideFloats:6,binary:'geometry.bin',parts:[{name:'blade',role:'component',byteOffset:0,vertexCount:3}],presentation:{rotors:[{id:'front-right',axis:[0,0,1],pivotMm:[20,10,3],parts:['blade']}]}};
  globalThis.fetch=async url=>new Response(String(url).endsWith('model.json')?JSON.stringify(metadata):vertices.buffer);
  const model=createManufacturerModel({id:'fixture',model:{url:'./assets/fixture/model.json'}},null);await model.ready;assert.equal(model.group.userData.motorRotors.length,1);assert.deepEqual([...model.parts.blade.geometry.getAttribute('position').data.array], [...vertices]);
  model.group.updateMatrixWorld(true);const vertex=model.parts.blade.localToWorld(new THREE.Vector3(21,10,3));assert.deepEqual(vertex.toArray(),[21,10,3]);model.dispose();
 }finally{globalThis.fetch=nativeFetch;}
});

test('worker observes both StampFly boards at cold publication points without querying unrelated boards',async()=>{
 const source=(await readFile(new URL('../web/wasm/worker.js',import.meta.url),'utf8')).replace(/^import .*;\n/gm,'');
 let wall=0,cycles=0,calls=0,raw=JSON.stringify(motion());const messages=[],timers=[],memory=new WebAssembly.Memory({initial:2}),encoder=new TextEncoder();
 const wasm={memory,esp32sim_alloc:()=>128,esp32sim_free(){},esp32sim_new:()=>1,esp32sim_delete(){},esp32sim_set_jit(){},esp32sim_boot:()=>0,
  esp32sim_cycles:()=>cycles,esp32sim_insns:()=>cycles,esp32sim_cpu_hz:()=>240e6,esp32sim_out_take:()=>0,esp32sim_run(_e,n){cycles+=n;return 0;},
  esp32sim_motion_snapshot(_e,p){calls++;const bytes=encoder.encode(raw);new Uint8Array(memory.buffer).set(bytes,p);return bytes.length;}};
 const context={TextEncoder,TextDecoder,ArrayBuffer,createPacing,applyExperiments,createJitHost:()=>({imports:{}}),performance:{now:()=>wall,timeOrigin:0},Date,postMessage:m=>messages.push(m),setTimeout:(callback,delay)=>timers.push({callback,delay}),WebAssembly:{instantiate:async()=>({instance:{exports:wasm}})}};
 runInNewContext(source,context);const send=data=>context.onmessage({data});await send({op:'init'});
 for(const board of ['stampfly','stampfly-v11']){await send({op:'create',board});await send({op:'start'});await send({op:'stop'});assert.ok(messages.some(m=>m.text?.includes('stampfly-motors')));}
 const previous=calls;await send({op:'create',board:'stamps3'});await send({op:'start'});await send({op:'stop'});assert.equal(calls,previous);
 await send({op:'create',board:'stampfly'});raw='{broken';await send({op:'stop'});assert.equal(messages.at(-1).stopped,0,'bad optional data cannot break stop');
 assert.equal(timers.length,3,'motion observations add no separate animation timers');
});
