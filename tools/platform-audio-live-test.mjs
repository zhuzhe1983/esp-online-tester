import test from 'node:test';
import assert from 'node:assert/strict';
import {realtimeInputPlan} from '../web/wasm/audio-input-policy.js';
import {PcmCaptureBuffer} from '../web/platform/audio-worklet.js';
import {createAudioStream} from '../web/platform/audio-stream.js';

test('realtime queue retains bounded recent frames and accounts for all removed input',()=>{
  assert.deepEqual(realtimeInputPlan({queuedFrames:2000,capacityFrames:96000,sampleRate:48000},3840),{clear:false,skipFrames:0,droppedFrames:0});
  assert.deepEqual(realtimeInputPlan({queuedFrames:11000,capacityFrames:96000,sampleRate:48000},3840),{clear:true,skipFrames:0,droppedFrames:11000});
  assert.deepEqual(realtimeInputPlan({queuedFrames:500,capacityFrames:16000,sampleRate:8000},3000),{clear:true,skipFrames:1000,droppedFrames:1500});
  assert.throws(()=>realtimeInputPlan({queuedFrames:0,capacityFrames:0,sampleRate:0},10));
});
test('capture under backpressure keeps newest samples, stays usable and reports loss',()=>{
  const c=new PcmCaptureBuffer({sampleRate:8000,blockFrames:4,maxQueuedSeconds:0.001,overflowPolicy:'drop-oldest'});
  c.append([Float32Array.from([.1,.2,.3,.4,.5,.6,.7,.8])]);
  assert.equal(c.append([Float32Array.from([-.1,-.2,-.3,-.4])]),true);
  assert.equal(c.droppedFrames,4);assert.deepEqual([...c.take()],[16384,19660,22937,26214]);
  assert.deepEqual([...c.take()],[-3277,-6554,-9830,-13107]);assert.equal(c.overflow,false);
  c.append([new Float32Array(12).fill(1)]);assert.equal(c.droppedFrames,8);assert.equal(c.length,8);
});
test('agent PCM is lossless by default and live loss is explicit in receipts',async()=>{
  const calls=[],a=createAudioStream({emit(){},isRunning:()=>true,request:async(op,packet)=>{calls.push({op,packet});return op==='configure'?{state:{configured:true}}:{acceptedFrames:2,skippedFrames:1,droppedFrames:9,state:{queuedFrames:2}};}});
  a.reset(true);await a.configure({input:{enabled:true,sampleRate:8000}});
  const r=await a.push({sampleRate:8000,samples:Int16Array.of(1,2,3),overflowPolicy:'drop-oldest'});
  assert.equal(r.droppedFrames,9);assert.equal(r.input.droppedFrames,9);assert.equal(calls.at(-1).packet.overflowPolicy,'drop-oldest');
  await assert.rejects(a.push({sampleRate:8000,samples:Int16Array.of(1),overflowPolicy:'bad'}),{code:'INVALID_AUDIO'});
  assert.deepEqual(a.read({clear:false,includeChunks:false}).chunks,[]);
});
