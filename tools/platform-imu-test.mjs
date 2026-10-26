import test from 'node:test';
import assert from 'node:assert/strict';
import {orientationSample, normalizeOrientation} from '../web/platform/imu.js';

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
