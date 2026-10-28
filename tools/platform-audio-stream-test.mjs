import assert from 'node:assert/strict';
import {test} from 'node:test';
import {createAudioStream,decodeAudio,validatePcmInput} from '../web/platform/audio-stream.js';
const output=(rate,samples)=>{const b=new ArrayBuffer(5+samples.length*2),v=new DataView(b);v.setUint8(0,2);v.setUint32(1,rate,true);samples.forEach((s,i)=>v.setInt16(5+i*2,s,true));return b;};
test('PCM validates complete frames and exact signed little-endian values',()=>{
  const p=validatePcmInput({sampleRate:24000,channels:2,samples:[-32768,32767,-1,0]});
  assert.equal(p.frames,2);assert.deepEqual([...new Uint8Array(p.data)],[0,128,255,127,255,255,0,0]);
  assert.throws(()=>validatePcmInput({sampleRate:24000,channels:2,samples:[1]}),{code:'INVALID_AUDIO'});
  assert.throws(()=>validatePcmInput({sampleRate:24000,samples:[NaN]}),{code:'INVALID_AUDIO'});
  const chunk=decodeAudio(output(24000,[-32768,0,32767]));assert.equal(chunk.sampleRate,24000);assert.deepEqual([...chunk.samples],[-32768,0,32767]);
  assert.throws(()=>decodeAudio(new ArrayBuffer(6)),{code:'INVALID_AUDIO_OUTPUT'});
});
test('output capture is bounded with explicit loss, while live output remains complete',()=>{
  const events=[],stream=createAudioStream({emit:e=>events.push(e),capacitySeconds:1});stream.reset(true);
  const samples=Array(8000).fill(1234);stream.receive(output(8000,samples),1);stream.receive(output(8000,samples),2);
  const captured=stream.read({clear:false});assert.equal(captured.chunks.length,1);assert.equal(captured.droppedFrames,8000);assert.equal(captured.overflowed,true);assert.equal(captured.chunks[0].sequence,2);
  assert.equal(events.filter(e=>e.type==='audio').reduce((n,e)=>n+e.samples.length,0),16000);
  captured.chunks[0].samples[0]=0;assert.equal(stream.read().chunks[0].samples[0],1234);assert.equal(stream.read().droppedFrames,0);
  stream.reset(false);assert.equal(stream.snapshot().available,false);assert.equal(stream.read().chunks.length,0);
});
test('input format is stable, partial acceptance is explicit and replacement releases previous port',async()=>{
  const calls=[];let accept=2,running=true;
  const stream=createAudioStream({emit(){},isRunning:()=>running,request:async(action,options)=>{calls.push({action,...options});return {state:{queuedFrames:accept},acceptedFrames:accept,guestTime:1};}});stream.reset(true);
  await stream.configure({input:{enabled:true,port:0,sampleRate:16000,channels:1}});
  assert.equal((await stream.push({sampleRate:16000,samples:new Int16Array([1,2])})).acceptedFrames,2);
  await assert.rejects(stream.push({sampleRate:24000,samples:new Int16Array([1])}),{code:'AUDIO_FORMAT_MISMATCH'});
  await assert.rejects(stream.configure({input:{enabled:false,port:1}}),{code:'AUDIO_PORT_MISMATCH'});assert.equal(stream.snapshot().input.enabled,true);
  accept=1;await assert.rejects(stream.push({sampleRate:16000,samples:new Int16Array([1,2])}),error=>error.code==='AUDIO_INPUT_OVERFLOW'&&error.details.acceptedFrames===1&&error.details.requestedFrames===2);
  running=false;await assert.rejects(stream.push({sampleRate:16000,samples:new Int16Array([1])}),{code:'NOT_RUNNING'});
  await stream.configure({input:{enabled:true,port:1,sampleRate:16000,channels:1}});
  assert.deepEqual(calls.slice(-2).map(c=>[c.action,c.port]),[['disable',0],['configure',1]]);
  await stream.configure({input:{enabled:false}});assert.equal(stream.snapshot().input.enabled,false);
});
