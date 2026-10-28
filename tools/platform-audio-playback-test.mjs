import test from 'node:test';
import assert from 'node:assert/strict';
import {PcmPlaybackQueue} from '../web/platform/audio-playback.js';

const packet = (frames, value=1234) => ({sampleRate:8000,samples:new Int16Array(frames).fill(value)});
const render = (queue, frames=80) => {
  const output = [new Float32Array(frames)];
  const state = queue.render(output);
  return {samples:output[0],state};
};

test('prebuffer waits for its target and preserves every queued sample', () => {
  const q = new PcmPlaybackQueue({renderRate:8000,prebufferSeconds:0.2});
  q.enqueue(packet(800));
  for(let i=0;i<10;i++) assert.ok(render(q).samples.every(v=>v===0));
  assert.equal(q.snapshot().renderedFrames,0); assert.equal(q.snapshot().underflows,0);
  q.enqueue(packet(800,2345));
  const out=render(q,1600);
  assert.equal(out.state.buffering,false); assert.equal(out.state.renderedFrames,1600);
  assert.ok(out.samples.subarray(0,800).every(v=>v===1234/32768));
  assert.ok(out.samples.subarray(800).every(v=>v===2345/32768));
  assert.ok(out.state.queuedSeconds<1e-9);
});

test('short clips and tails play within a bounded wait without an end marker', () => {
  const q = new PcmPlaybackQueue({renderRate:8000,prebufferSeconds:0.2,maxPrebufferWaitSeconds:0.35});
  for(let i=0;i<100;i++)render(q); // Quiet time before the first clip is not prebuffer time.
  q.enqueue(packet(160));
  for(let i=0;i<35;i++)assert.equal(render(q).state.renderedFrames,0);
  assert.equal(render(q).state.renderedFrames,80); assert.equal(render(q).state.renderedFrames,160);
  assert.equal(render(q).state.underflows,1);
  q.enqueue(packet(80,2345));
  assert.equal(render(q).state.renderedFrames,160,'after starvation a short tail buffers again');
  q.flush(); const last=render(q);
  assert.equal(last.state.renderedFrames,240);assert.ok(last.samples.every(v=>v===2345/32768));
  for(let i=0;i<100;i++)render(q);
  assert.equal(q.snapshot().underflows,2,'one empty interval is counted once, not once per quantum');
});

test('zero prebuffer retains immediate playback and clear removes queued samples only', () => {
  const q = new PcmPlaybackQueue({renderRate:8000});
  q.enqueue(packet(80));assert.equal(render(q).state.renderedFrames,80);
  assert.equal(render(q).state.underflows,1);
  q.enqueue(packet(80));assert.equal(render(q).state.renderedFrames,160);
  q.enqueue(packet(80));q.clear();assert.ok(render(q).samples.every(v=>v===0));
  assert.equal(q.snapshot().renderedFrames,160);assert.equal(q.snapshot().underflows,1);
});

test('rebuffering reduces repeated starvation under deterministic publisher delay without dropping PCM', t => {
  const run=prebufferSeconds=>{
    const q=new PcmPlaybackQueue({renderRate:8000,prebufferSeconds});
    const expected=[],actual=[];let next=0;
    // 20 ms blocks arrive every 22 ms, with an additional 8 ms every fifth block.
    // This intentionally includes sustained supply below real time: buffering can
    // group the gaps, but cannot claim to repair insufficient emulation throughput.
    const arrivals=Array.from({length:100},(_,i)=>i*22+Math.floor(i/5)*8);
    for(let ms=0;ms<4000;ms+=10){
      while(next<arrivals.length&&arrivals[next]<=ms){
        const samples=Int16Array.from({length:160},(_,i)=>1+((next*160+i)%30000));
        assert.equal(q.enqueue({samples,sampleRate:8000}).accepted,true);
        expected.push(...samples);next++;
      }
      const out=render(q);
      for(const value of out.samples)if(value)actual.push(Math.round(value*32768));
    }
    assert.equal(next,100);assert.deepEqual(actual,expected);
    assert.equal(q.snapshot().renderedFrames,16000);
    return q.snapshot();
  };
  const immediate=run(0),buffered=run(0.2);
  t.diagnostic(`Immediate underflows: ${immediate.underflows}; 200 ms prebuffer: ${buffered.underflows}; both render all 16000 source frames.`);
  assert.ok(immediate.underflows>=20,JSON.stringify(immediate));
  assert.ok(buffered.underflows<=4,JSON.stringify(buffered));
  assert.ok(buffered.underflows<immediate.underflows/5);
});

test('explicit realtime policy discards only the oldest queued audio and reports its duration', () => {
  const q=new PcmPlaybackQueue({renderRate:8000,maxQueuedSeconds:0.1,overflowPolicy:'drop-oldest'});
  q.enqueue(packet(800,1234));
  const result=q.enqueue(packet(400,2345));
  assert.equal(result.accepted,true);assert.equal(result.droppedFrames,400);assert.equal(result.droppedSeconds,0.05);
  const out=render(q,800);
  assert.ok(out.samples.subarray(0,400).every(v=>v===1234/32768));
  assert.ok(out.samples.subarray(400).every(v=>v===2345/32768));
  assert.equal(out.state.droppedFrames,400);assert.equal(out.state.overflows,1);
});

test('an oversized realtime block retains only its newest bounded tail', () => {
  const q=new PcmPlaybackQueue({renderRate:8000,maxQueuedSeconds:0.1,overflowPolicy:'drop-oldest'});
  q.enqueue(packet(400));
  const input=Int16Array.from({length:1200},(_,i)=>i+1);
  const result=q.enqueue({samples:input,sampleRate:8000});
  assert.equal(result.droppedFrames,800);assert.equal(result.droppedSeconds,0.1);
  assert.equal(q.chunks[0].samples.buffer.byteLength,1600,'discarded prefix does not remain retained through a view');
  const out=render(q,800);
  assert.deepEqual([...out.samples],Array.from(input.subarray(400),v=>v/32768));
  assert.deepEqual(q.enqueue({samples:input,sampleRate:NaN}),{accepted:false,code:'INVALID_PCM'});
  assert.equal(q.snapshot().droppedFrames,800,'invalid input is rejected, not counted as dropped audio');
});

test('realtime dropping accounts for the unplayed fraction of a resampled source frame', () => {
  const q=new PcmPlaybackQueue({renderRate:16000,maxQueuedSeconds:0.1,overflowPolicy:'drop-oldest'});
  q.enqueue(packet(800));render(q,1);
  q.enqueue(packet(1,2345));
  assert.ok(Math.abs(q.snapshot().droppedFrames-0.5)<1e-9);
  assert.ok(Math.abs(q.snapshot().droppedSeconds-1/16000)<1e-12);
  assert.ok(Math.abs(q.snapshot().queuedSeconds-0.1)<1e-12);
  assert.throws(()=>new PcmPlaybackQueue({renderRate:8000,overflowPolicy:'speed-up'}),RangeError);
});

test('rate conversion retains one second and its tone under prebuffered rendering', () => {
  const q=new PcmPlaybackQueue({renderRate:48000,prebufferSeconds:0.2});
  const samples=Int16Array.from({length:16000},(_,i)=>Math.round(20000*Math.sin(2*Math.PI*500*i/16000)));
  for(let i=0;i<samples.length;i+=320)assert.equal(q.enqueue({samples:samples.slice(i,i+320),sampleRate:16000}).accepted,true);
  const out=[new Float32Array(48000),new Float32Array(48000)];
  assert.equal(q.render(out).renderedFrames,48000);assert.deepEqual(out[0],out[1]);
  const crossings=out[0].reduce((n,v,i)=>n+(i>0&&out[0][i-1]<=0&&v>0),0);
  assert.equal(crossings,500);assert.ok(q.snapshot().queuedSeconds<1e-9);
});

test('overflow remains explicit and never discards earlier queued speech', () => {
  const q=new PcmPlaybackQueue({renderRate:8000,maxQueuedSeconds:0.2,prebufferSeconds:0.2});
  assert.equal(q.enqueue(packet(1600)).accepted,true);
  assert.deepEqual(q.enqueue(packet(1,2345)),{accepted:false,code:'AUDIO_OUTPUT_OVERFLOW'});
  const out=render(q,1600);assert.equal(out.state.overflows,1);
  assert.ok(out.samples.every(v=>v===1234/32768));
  assert.throws(()=>new PcmPlaybackQueue({renderRate:NaN}),RangeError);
  assert.throws(()=>new PcmPlaybackQueue({renderRate:8000,maxQueuedSeconds:0.1,prebufferSeconds:0.2}),RangeError);
});
