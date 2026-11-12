import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createSceneImuLink,imuModelForBoard} from '../web/platform/imu.js';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDefaultScene} from '../web/platform/catalog.js';

const q = [0,0,0,1], tilted = [0,0,Math.SQRT1_2,Math.SQRT1_2];
function fixture(send) {
  const state = {session:1,available:true,status:'running',mode:'3d',preview:false,flight:false,recording:false,hidden:false};
  const sent = [], errors = [];
  const link = createSceneImuLink({getContext:()=>state,send:async sample=>{sent.push(sample);await send?.(sample);},onError:e=>errors.push(e)});
  return {state,sent,errors,link};
}

test('link starts off; disabled drag creates no input and never disables the agent sensor', async()=>{
  const f=fixture();assert.equal(f.link.enabled,false);
  await f.link.offer({quaternion:tilted,rotating:true});assert.deepEqual(f.sent,[]);
  assert.equal(f.link.toggle(),true);await f.link.offer({quaternion:q});
  assert.equal(f.sent[0].enabled,true);assert.deepEqual(f.sent[0].angularVelocityDps,[0,0,0]);
  f.link.toggle();await f.link.idle();await f.link.offer({quaternion:tilted,rotating:true});
  assert.equal(f.sent.length,1);assert.ok(f.sent.every(sample=>sample.enabled!==false));
});

test('slow input delivery coalesces movement and off flushes zero gyro after the in-flight write', async()=>{
  let release;const first=new Promise(resolve=>{release=resolve;});let n=0;
  const f=fixture(async()=>{if(++n===1)await first;});f.link.toggle();
  const moving=f.link.offer({quaternion:q,rotating:true});
  void f.link.offer({quaternion:tilted,rotating:true});void f.link.offer({quaternion:[1,0,0,0],rotating:true});
  f.link.toggle();void f.link.offer({quaternion:tilted,rotating:true});release();await moving;await f.link.idle();
  assert.deepEqual(f.sent,[{source:'scene',enabled:true,quaternion:q},{source:'scene',angularVelocityDps:[0,0,0]}]);
  assert.deepEqual(f.errors,[]);
});

test('replaced sessions discard queued motion and never send a cleanup into the new device', async()=>{
  let release;const wait=new Promise(resolve=>{release=resolve;});const f=fixture(()=>wait);f.link.toggle();
  void f.link.offer({quaternion:q,rotating:true});void f.link.offer({quaternion:tilted,rotating:true});
  f.state.session++;f.link.sync({reset:true});release();await f.link.idle();
  assert.equal(f.sent.length,1);assert.equal(f.link.enabled,false);
});

test('an explicit agent sample takes over without zeroing its independently supplied gyro',async()=>{
  const f=fixture();f.link.toggle();await f.link.offer({quaternion:q,rotating:true});f.link.invalidate();
  await f.link.offer({quaternion:tilted,rotating:true});await f.link.idle();
  assert.equal(f.link.enabled,false);assert.equal(f.sent.length,1);
});

test('entering an appearance preview clears the old gesture without accepting preview movement',async()=>{
  let release;const wait=new Promise(resolve=>{release=resolve;});let n=0;
  const f=fixture(async()=>{if(++n===1)await wait;});f.link.toggle();void f.link.offer({quaternion:q,rotating:true});
  f.state.preview=true;f.link.sync();void f.link.offer({quaternion:tilted,rotating:true});release();await f.link.idle();
  assert.deepEqual(f.sent.at(-1),{source:'scene',angularVelocityDps:[0,0,0]});assert.equal(f.sent.length,2);
});

test('pause, recording, previews, 2D and flight prevent re-enabling and clear the choice', async()=>{
  for(const blocked of [{status:'paused'},{status:'loading'},{mode:'2d'},{preview:true},{recording:true},{flight:true},{available:false}]) {
    const f=fixture();f.link.toggle();await f.link.offer({quaternion:q});Object.assign(f.state,blocked);
    assert.ok(f.link.sync().reason);assert.equal(f.link.enabled,false);assert.equal(f.link.toggle(),false);
    await f.link.offer({quaternion:tilted,rotating:true});assert.equal(f.sent.length,1);
  }
});

test('blur or hidden tab clears angular velocity without changing acceleration or enabling after flight', async()=>{
  const f=fixture();f.link.toggle();await f.link.offer({quaternion:tilted,rotating:true});f.state.hidden=true;
  await f.link.offer({quaternion:q,rotating:true});await f.link.stop();
  assert.equal(f.sent.length,2);assert.deepEqual(f.sent[1],{source:'scene',angularVelocityDps:[0,0,0]});
  f.state.flight=true;f.link.sync();await f.link.idle();assert.equal(f.sent.length,2);
  f.state.flight=false;f.state.hidden=false;f.link.sync();assert.equal(f.link.enabled,false);
});

test('all host-fed runtime IMUs accept the same documented virtual axes, with no mounting claim',async()=>{
  const catalog={};for(const name of ['products','accessories','firmwares'])catalog[name]=JSON.parse(await readFile(new URL(`../web/platform/catalog/${name}.json`,import.meta.url),'utf8')).items;
  const runtime=createPlatformRuntime({catalog});const seen=new Set();
  try {
    for(const product of catalog.products.filter(p=>imuModelForBoard(p.runtime?.board))) {
      if(seen.has(product.runtime.board))continue;seen.add(product.runtime.board);
      await runtime.configure(createDefaultScene(catalog,product.id));
      const cap=(await runtime.capabilities()).imu;assert.equal(cap.sceneOrientation,true);assert.equal(cap.physicalMountingVerified,false);assert.equal(cap.sceneLinkDefault,false);
      assert.equal((await runtime.observe()).imu.source,'agent');
      const result=await runtime.setImu({source:'scene',quaternion:tilted});assert.ok(Math.abs(result.accelG[0]-1)<1e-8);assert.deepEqual(result.gyroDps,[0,0,0]);
    }
    assert.equal(seen.size,12);
    await runtime.configure({...createDefaultScene(catalog,'m5stack-k138'),flight:{enabled:true,model:'stampfly-rigid-body-v1'}});
    assert.equal((await runtime.capabilities()).imu.inputOwner,'flight');
    await assert.rejects(runtime.setImu({source:'scene',quaternion:q}),{code:'FLIGHT_OWNS_IMU'});
  }finally{await runtime.dispose();}
});
