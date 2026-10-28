import test from 'node:test';
import assert from 'node:assert/strict';
import {PcmPlaybackQueue, PcmCaptureBuffer} from '../web/platform/audio-worklet.js';
import {createBrowserAudio,encodePcm16Wav,decodePcm16Wav} from '../web/platform/audio.js';

const tick = () => new Promise(resolve => setImmediate(resolve));
function fakeRuntime() {
  const listeners = new Set(), calls = [], packets = [];
  return {calls,packets,subscribe(fn){listeners.add(fn);return () => listeners.delete(fn);},emit(event){for(const fn of listeners)fn(event);},
    async audioConfigure(value){calls.push(value);},async pushAudio(value){packets.push(value);}};
}
class FakeContext {
  static instances = [];
  constructor() { this.sampleRate = 48000; this.state = 'suspended'; this.destination = {}; this.audioWorklet = {addModule:async () => {}}; FakeContext.instances.push(this); }
  async resume() { this.state = 'running'; }
  async close() { this.state = 'closed'; }
  createMediaStreamSource() { return {connect(){},disconnect(){}}; }
}
class FakeNode {
  static instances = [];
  constructor(context,name) { this.name = name; this.context = context; this.sent = []; this.port = {postMessage:value => this.sent.push(value),onmessage:null}; FakeNode.instances.push(this); }
  connect() { this.connected = true; }
  disconnect() { this.connected = false; }
  async receive(data) { await this.port.onmessage?.({data}); }
}
function fakeStream() {
  const track = {stopped:false,stop(){this.stopped = true;}};
  return {track,getTracks:() => [track]};
}
const create = (runtime,options = {}) => createBrowserAudio({runtime,AudioContextClass:FakeContext,AudioWorkletNodeClass:FakeNode,...options});

test('fixed-ratio PCM preserves sample duration and pitch across chunk boundaries', () => {
  const sourceRate = 16000, renderRate = 48000, hz = 500;
  const samples = Int16Array.from({length:sourceRate},(_,i) => Math.round(20000 * Math.sin(2 * Math.PI * hz * i / sourceRate)));
  const queue = new PcmPlaybackQueue({renderRate});
  for(let i = 0;i < samples.length;i += 160) assert.equal(queue.enqueue({samples:samples.slice(i,i+160),sampleRate:sourceRate}).accepted,true);
  const channels = [new Float32Array(renderRate),new Float32Array(renderRate)];
  const result = queue.render(channels);
  assert.equal(result.renderedFrames,renderRate); assert.ok(result.queuedSeconds < 1e-9);
  assert.deepEqual(channels[0],channels[1]);
  const crossings = channels[0].reduce((n,value,index) => n + (index > 0 && channels[0][index-1] <= 0 && value > 0),0);
  assert.equal(crossings,hz);
  queue.render([new Float32Array(128)]); assert.equal(queue.underflows,1);
  queue.render([new Float32Array(128)]); assert.equal(queue.underflows,1,'one gap is not counted once per render quantum');
});

test('bounded playback rejects whole blocks and keeps previously queued audio intact', () => {
  const queue = new PcmPlaybackQueue({renderRate:8000,maxQueuedSeconds:0.1});
  const samples = new Int16Array(800).fill(1234);
  assert.equal(queue.enqueue({samples,sampleRate:8000}).accepted,true);
  assert.equal(queue.enqueue({samples:new Int16Array(1),sampleRate:8000}).code,'AUDIO_OUTPUT_OVERFLOW');
  const output = [new Float32Array(800)]; queue.render(output);
  assert.ok(output[0].every(value => value === 1234 / 32768)); assert.equal(queue.overflows,1);
  assert.equal(queue.enqueue({samples:new Int16Array(2),sampleRate:NaN}).code,'INVALID_PCM');
});

test('source-rate changes and stereo lanes retain total audio duration', () => {
  const queue = new PcmPlaybackQueue({renderRate:48000});
  queue.enqueue({samples:new Int16Array(8000).fill(8192),sampleRate:8000});
  queue.enqueue({samples:Int16Array.from({length:48000},(_,i) => i%2 ? -16384 : 16384),sampleRate:24000,channels:2});
  const output = [new Float32Array(96000),new Float32Array(96000)];
  assert.equal(queue.render(output).renderedFrames,96000);
  assert.equal(output[0][48000],0.5); assert.equal(output[1][48000],-0.5);
});

test('capture conversion and backpressure retain sample order with an explicit overflow', () => {
  const capture = new PcmCaptureBuffer({sampleRate:8000,blockFrames:4,maxQueuedSeconds:0.001});
  assert.equal(capture.append([Float32Array.of(-1,-0.5,0,0.5)]),true);
  assert.deepEqual([...capture.take()],[-32768,-16384,0,16384]);
  assert.equal(capture.append([new Float32Array(8).fill(1)]),true);
  assert.equal(capture.append([Float32Array.of(0)]),false);
  assert.deepEqual([...capture.take()],[32767,32767,32767,32767]);
});

test('PCM16 WAV round trips actual interleaved samples and rejects truncated/corrupt formats', () => {
  const packet = {sampleRate:44100,channels:2,samples:Int16Array.of(-32768,32767,-42,42,0,0)};
  assert.deepEqual(decodePcm16Wav(encodePcm16Wav(packet)),packet);
  const broken = encodePcm16Wav(packet); broken[34] = 32;
  assert.throws(() => decodePcm16Wav(broken),{code:'UNSUPPORTED_WAV'});
  assert.throws(() => decodePcm16Wav(broken.subarray(0,45)),{code:'INVALID_WAV'});
});

test('microphone is explicit, uses PCM backpressure, stops tracks on mute and guest reset', async () => {
  const runtime = fakeRuntime(), streams = []; let requests = 0;
  const audio = create(runtime,{mediaDevices:{async getUserMedia(){requests++;const stream=fakeStream();streams.push(stream);return stream;}}});
  try {
    assert.equal(requests,0); await audio.setMicrophone(true); assert.equal(requests,1);
    const node = FakeNode.instances.at(-1); assert.equal(node.name,'esp-platform-capture');
    const samples = Int16Array.of(123,-123);
    await node.receive({type:'pcm',samples,sampleRate:48000,channels:1});
    assert.deepEqual(runtime.packets[0],{samples,sampleRate:48000,channels:1,port:0});
    assert.deepEqual(node.sent.at(-1),{type:'ack'});
    await audio.setMicrophone(true); await node.receive({type:'pcm',samples,sampleRate:48000,channels:1});
    assert.equal(runtime.packets.length,2,'repeated enable must not invalidate the active source');
    await audio.setMicrophone(false); assert.equal(streams[0].track.stopped,true); assert.equal(node.connected,false);
    await audio.setMicrophone(true); const context = FakeContext.instances.at(-1);
    runtime.emit({type:'state',state:{status:'loading'}}); await tick();
    assert.equal(streams[1].track.stopped,true); assert.equal(audio.observe().microphone,'off'); assert.equal(context.state,'closed');
  } finally { await audio.dispose(); }
});

test('late microphone permission after cancellation releases tracks without attaching input', async () => {
  const runtime = fakeRuntime(); let grant;
  const audio = create(runtime,{mediaDevices:{getUserMedia:() => new Promise(resolve => {grant=resolve;})}});
  const enabling = audio.setMicrophone(true); await tick(); await audio.setMicrophone(false);
  const stream = fakeStream(); grant(stream); await enabling;
  assert.equal(stream.track.stopped,true); assert.equal(runtime.calls.some(value => value.input.enabled),false);
  await audio.dispose();
});

test('PCM input rejection disables capture and records a visible error', async () => {
  const runtime = fakeRuntime(), stream = fakeStream();
  runtime.pushAudio = async () => { throw Object.assign(new Error('guest queue full'),{code:'AUDIO_INPUT_FULL'}); };
  const audio = create(runtime,{mediaDevices:{getUserMedia:async()=>stream}});
  await audio.setMicrophone(true);
  await FakeNode.instances.at(-1).receive({type:'pcm',samples:new Int16Array(480),sampleRate:48000,channels:1});
  assert.equal(audio.observe().microphone,'error'); assert.equal(audio.observe().microphoneError.code,'AUDIO_INPUT_FULL'); assert.equal(stream.track.stopped,true);
  await audio.dispose();
});

test('speaker mute does not disable offline runtime output; backpressure surfaces overflow', async () => {
  const runtime = fakeRuntime(), audio = create(runtime,{maxQueuedSeconds:0.1});
  await audio.setSpeaker(true);
  runtime.emit({type:'audio',sampleRate:8000,channels:1,samples:new Int16Array(800)});
  const node = FakeNode.instances.at(-1); assert.equal(node.sent[0].type,'pcm');
  runtime.emit({type:'audio',sampleRate:8000,channels:1,samples:new Int16Array(1)});
  assert.equal(audio.observe().speaker,'error'); assert.equal(audio.observe().speakerError.code,'AUDIO_OUTPUT_OVERFLOW');
  assert.equal(runtime.calls.some(value => value.output?.enabled === false),false);
  await audio.setSpeaker(false); await audio.dispose(); assert.equal(node.connected,false);
});

test('disposal while permission is pending does not leak a microphone or allow reactivation', async () => {
  const runtime = fakeRuntime(); let grant;
  const audio = create(runtime,{mediaDevices:{getUserMedia:() => new Promise(resolve => {grant=resolve;})}});
  const enabling = audio.setMicrophone(true); await tick(); await audio.dispose();
  const stream = fakeStream(); grant(stream); await enabling;
  assert.equal(stream.track.stopped,true);
  await assert.rejects(audio.setSpeaker(true),{code:'AUDIO_DISPOSED'});
});


test('mute stops a granted stream while input attachment is pending and cannot disable its replacement', async () => {
  const runtime = fakeRuntime(), streams = []; let finishFirst, count = 0;
  runtime.audioConfigure = async value => {
    runtime.calls.push(value);
    if(value.input.enabled && ++count === 1) await new Promise(resolve => {finishFirst = resolve;});
  };
  const audio = create(runtime,{mediaDevices:{async getUserMedia(){const value=fakeStream();streams.push(value);return value;}}});
  const first = audio.setMicrophone(true); await tick();
  await audio.setMicrophone(false);
  assert.equal(streams[0].track.stopped,true,'mute must release an already granted track before the runtime replies');
  await audio.setMicrophone(true); finishFirst(); await first;
  assert.equal(audio.observe().microphone,'on'); assert.equal(streams[1].track.stopped,false);
  assert.equal(runtime.calls.filter(value => !value.input.enabled).length,1,'late cancelled attachment must not disable a new source');
  await audio.dispose();
});
