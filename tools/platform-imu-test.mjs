import test from 'node:test';
import assert from 'node:assert/strict';
import {orientationSample, normalizeOrientation} from '../web/platform/imu.js';
import {readFile} from 'node:fs/promises';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDefaultScene} from '../web/platform/catalog.js';

const close = (a,b) => a.forEach((n,i) => assert.ok(Math.abs(n-b[i]) < 1e-8, `${a} != ${b}`));
test('stationary gravity projects into the six model orientations', () => {
  const h=Math.SQRT1_2;
  for (const [q,expected] of [[[0,0,0,1],[0,1,0]],[[1,0,0,0],[0,-1,0]],[[h,0,0,h],[0,0,-1]],[[-h,0,0,h],[0,0,1]],[[0,0,h,h],[1,0,0]],[[0,0,-h,h],[-1,0,0]]]) {
    const sample=orientationSample(q); close(sample.accelG,expected); close([Math.hypot(...sample.accelG)],[1]); close(sample.gyroDps,[0,0,0]);
  }
});
test('rotation uses the shortest quaternion delta and stops across idle gaps', () => {
  const q=[0,0,Math.sin(Math.PI/40),Math.cos(Math.PI/40)];
  close(orientationSample(q,{previous:[0,0,0,1],elapsedSeconds:.1}).gyroDps,[0,0,90]);
  close(orientationSample(q.map(n=>-n),{previous:[0,0,0,1],elapsedSeconds:.1}).gyroDps,[0,0,90]);
  close(orientationSample(q,{previous:[0,0,0,1],elapsedSeconds:2}).gyroDps,[0,0,0]);
  close(orientationSample(q,{angularVelocityDps:[5000,-5000,0]}).gyroDps,[2000,-2000,0]);
  close(normalizeOrientation([0,0,0,2]),[0,0,0,1]);
  assert.throws(()=>normalizeOrientation([0,0,0,0]),{code:'INVALID_IMU'});
  assert.throws(()=>normalizeOrientation([1e308,1e308,1e308,1e308]),{code:'INVALID_IMU'});
  assert.throws(()=>orientationSample(q,{angularVelocityDps:[NaN,0,0]}),{code:'INVALID_IMU'});
});

test('AtomS3 accepts explicit virtual IMU input without claiming a scene mounting', async () => {
  const catalog={};
  for(const name of ['products','accessories','firmwares'])catalog[name]=JSON.parse(await readFile(new URL(`../web/platform/catalog/${name}.json`,import.meta.url),'utf8')).items;
  const runtime=createPlatformRuntime({catalog});
  try {
    await runtime.configure(createDefaultScene(catalog,'m5stack-c123'));
    const capability=(await runtime.capabilities()).imu;
    assert.equal(capability.available,true);assert.equal(capability.model,'mpu6886-register-subset');
    assert.equal(capability.sceneOrientation,false);assert.equal(capability.physicalMountingVerified,false);
    const sample=await runtime.setImu({quaternion:[0,0,Math.SQRT1_2,Math.SQRT1_2]});
    close(sample.accelG,[1,0,0]);assert.equal(sample.source,'agent');assert.equal(sample.delivery,'configured');
    await assert.rejects(runtime.setImu({source:'scene',quaternion:[0,0,0,1]}),{code:'IMU_SCENE_UNSUPPORTED'});
    close((await runtime.observe()).imu.accelG,[1,0,0]);
    await runtime.configure(createDefaultScene(catalog,'m5stack-papermono'));
    assert.equal((await runtime.observe()).imu,null);
    await assert.rejects(runtime.setImu({quaternion:[0,0,0,1]}),{code:'IMU_UNSUPPORTED'});
    await runtime.configure(createDefaultScene(catalog,'m5stack-stopwatch'));
    assert.equal((await runtime.capabilities()).imu.sceneOrientation,true);
    close((await runtime.setImu({source:'scene',quaternion:[0,0,0,1]})).accelG,[0,1,0]);
  } finally {await runtime.dispose();}
});

test('AtomS3R exposes its own BMI270 virtual input without aliasing AtomS3 or calibrating scene axes', async () => {
  const catalog={};for(const name of ['products','accessories','firmwares'])catalog[name]=JSON.parse(await readFile(new URL(`../web/platform/catalog/${name}.json`,import.meta.url),'utf8')).items;
  const runtime=createPlatformRuntime({catalog});try {
    await runtime.configure(createDefaultScene(catalog,'m5stack-c126'));
    assert.equal((await runtime.observe()).imu.source,'agent');
    const cap=(await runtime.capabilities()).imu;assert.equal(cap.model,'bmi270-register-subset');assert.equal(cap.sceneOrientation,false);
    close((await runtime.setImu({quaternion:[0,0,Math.SQRT1_2,Math.SQRT1_2]})).accelG,[1,0,0]);
    await assert.rejects(runtime.setImu({source:'scene',quaternion:[0,0,0,1]}),{code:'IMU_SCENE_UNSUPPORTED'});
  } finally {await runtime.dispose();}
});
