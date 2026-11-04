import test from 'node:test';
import assert from 'node:assert/strict';
import * as THREE from '../web/platform/vendor/three.module.min.js';
import {createStampflyDemoController,isStampflyDemo,parseStampflyDemoLine,stampflyCommand} from '../web/platform/stampfly-demo-controls.js';
import {flightViewPose,applyFlightPose} from '../web/platform/flight-view.js';
import {STAMPFLY_MOTORS} from '../web/platform/motor-state.js';

// Transport doubles exercise UI boundaries, not firmware execution acceptance.
const initial=()=>({status:'running',scene:{productId:'m5stack-k138'},product:{runtime:{board:'stampfly'}},firmware:{metadata:{source:'esp-app-description',name:'stampfly_uart_demo',version:'0.1.0'}},motors:[],motorOutput:null});
const telemetry=(ms=1)=>`SF1 STATE STOP ${ms} 0 0 0 0 0 0 0 0 1 0\n`;
function fixture(value=initial()) {
  let state=value;const listeners=new Set(),inputs=[],timers=new Map(),configs=[];let id=0,resets=0;
  const emit=e=>listeners.forEach(listener=>listener(e));
  const runtime={subscribe(fn){listeners.add(fn);return()=>listeners.delete(fn);},async observe(){return state;},async input(e){inputs.push(e);return{delivery:'queued'};},async reset(){resets++;},async configureFlight(e){configs.push(e);}};
  const c=createStampflyDemoController({runtime,setTimer:fn=>{timers.set(++id,fn);return id;},clearTimer:i=>timers.delete(i)});
  const set=patch=>{state={...state,...patch};emit({type:'state',state});};set({});
  const serial=(data,source='uart0')=>emit({type:'serial',source,data});
  const motors=(duties,cycle=1)=>emit({type:'motors',motors:STAMPFLY_MOTORS.map((m,i)=>({...m,duty:duties[i],frequencyHz:150000})),motorOutput:{cycle,guestTime:cycle/240e6}});
  return{c,emit,set,serial,motors,inputs,timers,configs,get resets(){return resets;}};
}
test('demo identity and bounded protocol do not treat arbitrary local/factory firmware as a controller',()=>{
  assert.ok(isStampflyDemo(initial()));
  for(const change of [s=>s.product.runtime.board='stamps3',s=>s.firmware.metadata.name='factory',s=>s.firmware.metadata.source='manifest',s=>s.firmware=null,s=>s.firmware.metadata.version='unknown']){const s=initial();change(s);assert.equal(isStampflyDemo(s),false);}
  assert.deepEqual(parseStampflyDemoLine('SF1 READY stampfly-uart-demo 0.1.0'),{type:'ready'});
  assert.equal(parseStampflyDemoLine('SF1 READY other 0.1.0'),null);
  assert.equal(parseStampflyDemoLine(telemetry().trim()).tofValid,false);
  assert.equal(parseStampflyDemoLine(telemetry().trim().replace(' 0 1 0',' 2 1 0')),null);
  assert.equal(stampflyCommand('MOTOR',[.1,.2,.3,.4]),'SF1 MOTOR 0.1 0.2 0.3 0.4\n');
  for(const values of [[NaN,0,0,0],[1.1,0,0,0],[-1,0,0,0],[0,0]])assert.throws(()=>stampflyCommand('MOTOR',values));
  assert.throws(()=>stampflyCommand('TAKEOFF',[0]));assert.throws(()=>stampflyCommand('TARGET',[.3,11,0,0]));
});
test('queued UART and ACK cannot substitute for fresh actual PWM; chunks and ports stay distinct',async()=>{
  const f=fixture();try{
    await assert.rejects(f.c.send('MOTOR',[.2,.2,.2,.2]));
    f.serial('SF1 READY stampfly-uart-demo 0.1.0\n','usb');assert.equal(f.c.snapshot().ready,false);
    f.serial('SF1 READY stampfly-uart-');f.serial('demo 0.1.0\nSF1 STA');f.serial('TE STOP 1 0 0 0 0 0 0 0 0 1 0\n');
    assert.equal(f.c.snapshot().telemetry.imuValid,true);
    f.motors([0,0,0,0],10);await f.c.send('MOTOR',[.2,.2,.2,.2]);
    assert.equal(f.inputs.length,1);assert.equal(f.c.snapshot().pending.acknowledged,false);
    f.serial('SF1 ACK MOTOR\n');assert.equal(f.c.snapshot().pending.acknowledged,true);
    f.motors([.2,.2,.2,.2],10);assert.ok(f.c.snapshot().pending,'old output is insufficient');
    f.motors([.2,.2,.2,.2],11);assert.equal(f.c.snapshot().pending,null);
    await f.c.send('MOTOR',[.4,.4,.4,.4]);await f.c.stop();
    f.serial('SF1 ACK MOTOR\n');assert.equal(f.c.snapshot().pending.acknowledged,false);
    f.motors([0,0,0,0],12);f.serial('SF1 ACK STOP\n');assert.equal(f.c.snapshot().pending,null);
    assert.match(f.c.snapshot().feedback,/归零/);
  }finally{f.c.dispose();}assert.equal(f.timers.size,0);
});
test('pause cancels pending confirmation, stop resets paused demo, reset and device switch revoke READY',async()=>{
  const f=fixture();try{
    f.serial('SF1 READY stampfly-uart-demo 0.1.0\n');await f.c.send('MOTOR',[.2,.2,.2,.2]);
    f.set({status:'paused'});assert.equal(f.c.snapshot().pending,null);await assert.rejects(f.c.send('MOTOR',[0,0,0,0]));
    await f.c.stop();assert.equal(f.resets,1);assert.equal(f.c.snapshot().ready,false);
    f.set({status:'ready'});await f.c.configureFlight(true);assert.deepEqual(f.configs,[{enabled:true}]);
    f.set({status:'running'});f.serial('SF1 ACK MOTOR\n');assert.equal(f.c.snapshot().ready,false);
    f.serial('SF1 READY stampfly-uart-demo 0.1.0\n');f.set({scene:{productId:'m5stack-k138-v11'},product:{runtime:{board:'stampfly-v11'}}});
    assert.equal(f.c.snapshot().matched,true);assert.equal(f.c.snapshot().ready,false);
    f.set({firmware:{metadata:{name:'factory'}}});f.serial('SF1 READY stampfly-uart-demo 0.1.0\n');await assert.rejects(f.c.stop());assert.equal(f.c.snapshot().ready,false);
  }finally{f.c.dispose();}
});
test('takeoff remains pending until a fresh physical snapshot leaves the ground; timeout is honest',async()=>{
  const f=fixture();try{
    f.serial('SF1 READY stampfly-uart-demo 0.1.0\n');await assert.rejects(f.c.send('TAKEOFF',[.3]));
    f.emit({type:'flight',flight:{enabled:true,cycle:10,heightM:0,contact:true}});await f.c.send('TAKEOFF',[.3]);
    f.serial('SF1 ACK TAKEOFF\n');assert.ok(f.c.snapshot().pending);
    f.emit({type:'flight',flight:{enabled:true,cycle:11,heightM:.1,contact:false}});assert.equal(f.c.snapshot().pending,null);
    await f.c.send('LAND');f.serial('SF1 ACK LAND\n');assert.ok(f.c.snapshot().pending);
    [...f.timers.values()][0]();assert.equal(f.c.snapshot().pending,null);assert.match(f.c.snapshot().feedback,/未确认/);
  }finally{f.c.dispose();}
});
const pose=()=>({enabled:true,model:'stampfly-rigid-body-v1',worldFrame:'NED',bodyFrame:'FRD',positionM:[.1,.2,-.3],quaternionXyzw:[0,0,0,1],guestTime:2,epoch:1});
test('flight uses physical NED/FRD meters, preserves native local geometry and resets independently of camera orbit',()=>{
  const orbit=new THREE.Group(),flight=new THREE.Group(),assembly=new THREE.Group(),part=new THREE.Mesh(new THREE.BoxGeometry(8,2,1));
  orbit.rotation.set(.3,.4,.5);orbit.add(flight);flight.add(assembly);part.position.set(20,5,7);assembly.add(part);
  const before=part.matrix.clone(),positions=part.geometry.attributes.position.array.slice();
  assert.equal(applyFlightPose(flight,'stampfly',pose()),true);assert.deepEqual(flight.position.toArray(),[200,100,300]);
  assert.deepEqual(part.position.toArray(),[20,5,7]);assert.deepEqual(part.geometry.attributes.position.array,positions);assert.deepEqual(part.matrix.elements,before.elements);
  const p=pose();p.quaternionXyzw=new THREE.Quaternion().setFromAxisAngle(new THREE.Vector3(1,0,0),Math.PI/2).toArray();
  const q=new THREE.Quaternion(...flightViewPose('stampfly-v11',p).quaternion),up=new THREE.Vector3(0,0,1).applyQuaternion(q);assert.ok(up.distanceTo(new THREE.Vector3(1,0,0))<1e-9);
  const orbitBefore=orbit.quaternion.toArray();applyFlightPose(flight,'stampfly',null);assert.deepEqual(flight.position.toArray(),[0,0,0]);assert.deepEqual(flight.quaternion.toArray(),[0,0,0,1]);assert.deepEqual(orbit.quaternion.toArray(),orbitBefore);
  for(const change of [s=>s.enabled=false,s=>s.positionM[0]=NaN,s=>s.quaternionXyzw=[0,0,0,0],s=>s.worldFrame='ENU',s=>s.guestTime=-1]){const v=pose();change(v);assert.equal(flightViewPose('stampfly',v),null);}
  assert.equal(flightViewPose('stamps3',pose()),null);
});
