// Browser device IO stays separate from the runtime's PCM stream and offline capture.
const MAX_SECONDS = 2;
const fail = (code, message) => Object.assign(new Error(message), {code});
const describe = error => ({code:error?.code || error?.name || 'AUDIO_ERROR', message:error?.message || String(error)});
export function validatePcm({samples, sampleRate, channels = 1}, {output = false} = {}) {
  if (!(samples instanceof Int16Array) || !Number.isInteger(sampleRate) || sampleRate < (output?1000:8000) || sampleRate > (output?400000:96000) || ![1,2].includes(channels) || !samples.length || samples.length % channels) throw fail('INVALID_PCM', 'Audio requires nonempty interleaved PCM16, a supported sample rate, one or two channels.');
  return {samples, sampleRate, channels};
}

export function encodePcm16Wav(packet) {
  const {samples, sampleRate, channels} = validatePcm(packet,{output:true});
  if (samples.byteLength > 128 * 1024 * 1024) throw fail('AUDIO_TOO_LARGE', 'WAV export is limited to 128 MiB of PCM.');
  const bytes = new Uint8Array(44 + samples.byteLength), view = new DataView(bytes.buffer);
  const text = (at, value) => { for (let i = 0; i < value.length; i++) bytes[at + i] = value.charCodeAt(i); };
  text(0,'RIFF'); view.setUint32(4, bytes.length - 8, true); text(8,'WAVE'); text(12,'fmt ');
  view.setUint32(16,16,true); view.setUint16(20,1,true); view.setUint16(22,channels,true);
  view.setUint32(24,sampleRate,true); view.setUint32(28,sampleRate * channels * 2,true);
  view.setUint16(32,channels * 2,true); view.setUint16(34,16,true); text(36,'data'); view.setUint32(40,samples.byteLength,true);
  for (let i = 0; i < samples.length; i++) view.setInt16(44 + i * 2,samples[i],true);
  return bytes;
}

export function decodePcm16Wav(input) {
  const bytes = input instanceof Uint8Array ? input : input instanceof ArrayBuffer ? new Uint8Array(input) : null;
  if (!bytes || bytes.length < 44 || bytes.length > 128 * 1024 * 1024 + 4096) throw fail('INVALID_WAV', 'Expected a bounded PCM16 WAV file.');
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength), text = at => String.fromCharCode(...bytes.subarray(at,at + 4));
  const end = view.getUint32(4,true) + 8;
  if (text(0) !== 'RIFF' || text(8) !== 'WAVE' || end > bytes.length || end < 12) throw fail('INVALID_WAV', 'Invalid RIFF/WAVE header.');
  let format, data;
  for (let at = 12; at + 8 <= end;) {
    const size = view.getUint32(at + 4,true), start = at + 8;
    if (start + size > end) throw fail('INVALID_WAV', 'Truncated WAV chunk.');
    if (text(at) === 'fmt ') {
      if (size < 16 || view.getUint16(start,true) !== 1 || view.getUint16(start + 14,true) !== 16) throw fail('UNSUPPORTED_WAV', 'Only uncompressed PCM16 WAV is supported.');
      format = {channels:view.getUint16(start + 2,true), sampleRate:view.getUint32(start + 4,true), alignment:view.getUint16(start + 12,true), byteRate:view.getUint32(start + 8,true)};
    } else if (text(at) === 'data') {
      if (data) throw fail('UNSUPPORTED_WAV', 'Multiple WAV data chunks are unsupported.');
      data = bytes.subarray(start,start + size);
    }
    at = start + size + (size & 1);
  }
  if (!format || !data || data.length % 2 || format.alignment !== format.channels * 2 || format.byteRate !== format.sampleRate * format.alignment) throw fail('INVALID_WAV', 'Missing or inconsistent PCM16 WAV format/data.');
  const samples = new Int16Array(data.length / 2), pcm = new DataView(data.buffer,data.byteOffset,data.byteLength);
  for (let i = 0; i < samples.length; i++) samples[i] = pcm.getInt16(i * 2,true);
  return validatePcm({...format,samples},{output:true});
}

export function createBrowserAudio({runtime, onState = () => {}, AudioContextClass = globalThis.AudioContext || globalThis.webkitAudioContext,
  AudioWorkletNodeClass = globalThis.AudioWorkletNode, mediaDevices = globalThis.navigator?.mediaDevices,
  workletUrl = new URL('./audio-worklet.js', import.meta.url), maxQueuedSeconds = MAX_SECONDS} = {}) {
  if (!Number.isFinite(maxQueuedSeconds) || maxQueuedSeconds < 0.05 || maxQueuedSeconds > 10) throw fail('INVALID_AUDIO_LIMIT','Audio buffering must be 0.05–10 seconds.');
  if (!runtime?.subscribe || typeof runtime.pushAudio !== 'function' || typeof runtime.audioConfigure !== 'function') throw fail('AUDIO_UNSUPPORTED', 'The runtime does not expose PCM audio IO.');
  let disposed = false, context = null, contextReady = null, contextGeneration = 0, microphoneGeneration = 0, speakerGeneration = 0;
  let stream = null, captureNode = null, mediaNode = null, playbackNode = null, nextId = 0;
  const pending = new Map(), pendingStreams = new Set();
  const state = {microphone:'off',speaker:'off',microphoneError:null,speakerError:null,sampleRate:null,inputFrames:0,outputFrames:0,renderedFrames:0,queuedSeconds:0,underflows:0,overflows:0};
  const snapshot = () => ({...state,microphoneError:state.microphoneError && {...state.microphoneError},speakerError:state.speakerError && {...state.speakerError}});
  const emit = () => { if (!disposed) onState(snapshot()); };
  const alive = () => { if (disposed) throw fail('AUDIO_DISPOSED','Browser audio was disposed.'); };
  function ensureContext() {
    alive();
    if (!AudioContextClass || !AudioWorkletNodeClass) return Promise.reject(fail('AUDIO_UNAVAILABLE','This browser does not support AudioWorklet.'));
    if (!context) {
      const created = new AudioContextClass({latencyHint:'interactive'}), generation = contextGeneration;
      context = created;
      // Called synchronously from the button handler, before permission or module awaits.
      const resumed = created.resume();
      contextReady = Promise.all([resumed, created.audioWorklet.addModule(workletUrl.href || String(workletUrl))]).then(() => {
        if (generation !== contextGeneration || disposed) throw fail('AUDIO_CANCELLED','Audio activation was cancelled.');
        state.sampleRate = created.sampleRate; return created;
      }).catch(error => {
        if (context === created) { context = null; contextReady = null; }
        void created.close().catch(() => {}); throw error;
      });
    } else if (context.state === 'suspended') {
      const resumed = context.resume(); contextReady = Promise.all([contextReady,resumed]).then(([value]) => value);
    }
    return contextReady;
  }
  function releaseMicrophone() {
    for (const waiting of pendingStreams) for (const track of waiting.getTracks()) track.stop();
    pendingStreams.clear();
    for (const track of stream?.getTracks() || []) track.stop(); stream = null;
    if (captureNode) { captureNode.port.onmessage = null; captureNode.port.postMessage({type:'stop'}); captureNode.disconnect(); captureNode = null; }
    if (mediaNode) { mediaNode.disconnect(); mediaNode = null; }
  }
  function releaseSpeaker() {
    if (playbackNode) { playbackNode.port.onmessage = null; playbackNode.port.postMessage({type:'clear'}); playbackNode.disconnect(); playbackNode = null; }
    pending.clear(); state.queuedSeconds = 0;
  }
  async function closeIdleContext() {
    if (['on','starting'].includes(state.microphone) || ['on','starting'].includes(state.speaker) || !context) return;
    contextGeneration++; const old = context; context = null; contextReady = null; state.sampleRate = null;
    if (old.state !== 'closed') await old.close().catch(() => {});
  }
  async function disableInput() {
    try { await runtime.audioConfigure({input:{enabled:false,port:0}}); } catch { /* Worker teardown already detached its source. */ }
  }
  async function microphoneError(error) {
    microphoneGeneration++; releaseMicrophone(); state.microphone = 'error'; state.microphoneError = describe(error);
    if (/OVERFLOW|FULL/.test(state.microphoneError.code)) state.overflows++;
    emit(); await disableInput(); await closeIdleContext();
  }
  function speakerError(error) {
    speakerGeneration++; releaseSpeaker(); state.speaker = 'error'; state.speakerError = describe(error);
    if (/OVERFLOW|FULL/.test(state.speakerError.code)) state.overflows++;
    emit(); void closeIdleContext();
  }
  function queueOutput(packet) {
    if (state.speaker !== 'on' || !playbackNode) return;
    try {
      const {samples,sampleRate,channels} = validatePcm(packet,{output:true}), duration = samples.length / channels / sampleRate;
      const inFlight = [...pending.values()].reduce((total,seconds) => total + seconds,0);
      if (inFlight + state.queuedSeconds + duration > maxQueuedSeconds + 1e-9) throw fail('AUDIO_OUTPUT_OVERFLOW','Speaker queue is full; playback stopped. Export the captured PCM for offline listening.');
      const copied = samples.slice(), id = ++nextId; pending.set(id,duration);
      playbackNode.port.postMessage({type:'pcm',id,samples:copied,sampleRate,channels},[copied.buffer]);
      state.outputFrames += samples.length / channels;
    } catch (error) { speakerError(error); }
  }
  async function setSpeaker(enabled) {
    alive();
    if (enabled && state.speaker === 'on') return snapshot();
    const generation = ++speakerGeneration;
    if (!enabled) { releaseSpeaker(); state.speaker = 'off'; state.speakerError = null; emit(); await closeIdleContext(); return snapshot(); }
    state.speaker = 'starting'; state.underflows = 0; state.speakerError = null; emit();
    try {
      const ctx = await ensureContext();
      if (generation !== speakerGeneration || disposed) return snapshot();
      const node = new AudioWorkletNodeClass(ctx,'esp-platform-playback',{numberOfInputs:0,numberOfOutputs:1,outputChannelCount:[2],processorOptions:{maxQueuedSeconds}});
      playbackNode = node;
      node.port.onmessage = ({data}) => {
        if (playbackNode !== node || disposed) return;
        if (data.type === 'accepted') {
          pending.delete(data.id); state.queuedSeconds = data.queuedSeconds;
          if (!data.accepted) { speakerError(fail(data.code || 'AUDIO_OUTPUT_ERROR','Speaker queue rejected PCM; playback stopped.')); return; }
        } else if (data.type === 'playback') {
          state.queuedSeconds = data.queuedSeconds; state.underflows = data.underflows; state.renderedFrames = data.renderedFrames;
        }
        emit();
      };
      node.onprocessorerror = () => { if (playbackNode === node) speakerError(fail('AUDIO_PROCESSOR_ERROR','Speaker AudioWorklet stopped.')); };
      node.connect(ctx.destination); state.speaker = 'on'; emit(); return snapshot();
    } catch (error) {
      if (generation === speakerGeneration && !disposed) speakerError(error);
      throw error;
    }
  }
  async function setMicrophone(enabled) {
    alive();
    if (enabled && state.microphone === 'on') return snapshot();
    const generation = ++microphoneGeneration;
    if (!enabled) { releaseMicrophone(); state.microphone = 'off'; state.microphoneError = null; emit(); await disableInput(); await closeIdleContext(); return snapshot(); }
    state.microphone = 'starting'; state.microphoneError = null; emit();
    let acquired = null;
    try {
      const prepared = ensureContext();
      // Capture permission is requested only by explicit activation, never page startup.
      if (!mediaDevices?.getUserMedia) { await prepared; throw fail('MICROPHONE_UNAVAILABLE','Microphone access requires HTTPS/localhost and browser permission.'); }
      const permission = mediaDevices.getUserMedia({audio:{channelCount:1,echoCancellation:false,noiseSuppression:false,autoGainControl:false},video:false}).then(value => {
        if (generation !== microphoneGeneration || disposed) for (const track of value.getTracks()) track.stop();
        else pendingStreams.add(value);
        return value;
      });
      // Both promises are handled immediately even when either operation fails first.
      const results = await Promise.allSettled([prepared,permission]);
      if (results[1].status === 'fulfilled') acquired = results[1].value;
      if (generation !== microphoneGeneration || disposed) { for (const track of acquired?.getTracks() || []) track.stop(); return snapshot(); }
      const failed = results.find(result => result.status === 'rejected'); if (failed) throw failed.reason;
      const ctx = results[0].value;
      await runtime.audioConfigure({input:{enabled:true,port:0,sampleRate:ctx.sampleRate,channels:1}});
      if (generation !== microphoneGeneration || disposed) { for (const track of acquired.getTracks()) track.stop(); if (disposed || !['on','starting'].includes(state.microphone)) await disableInput(); return snapshot(); }
      stream = acquired; pendingStreams.delete(acquired); acquired = null;
      const node = new AudioWorkletNodeClass(ctx,'esp-platform-capture',{numberOfInputs:1,numberOfOutputs:1,outputChannelCount:[1],processorOptions:{maxQueuedSeconds}});
      captureNode = node; mediaNode = ctx.createMediaStreamSource(stream);
      node.port.onmessage = async ({data}) => {
        if (captureNode !== node || generation !== microphoneGeneration || disposed) return;
        if (data.type === 'error') { await microphoneError(fail(data.code,'Microphone queue is full; capture stopped because the simulator cannot keep up.')); return; }
        if (data.type !== 'pcm') return;
        try {
          const packet = validatePcm(data);
          await runtime.pushAudio({...packet,port:0});
          if (captureNode !== node || generation !== microphoneGeneration || disposed) return;
          state.inputFrames += packet.samples.length / packet.channels;
          node.port.postMessage({type:'ack'}); emit();
        } catch (error) { if (captureNode === node && generation === microphoneGeneration && !disposed) await microphoneError(error); }
      };
      node.onprocessorerror = () => { if (captureNode === node) void microphoneError(fail('AUDIO_PROCESSOR_ERROR','Microphone AudioWorklet stopped.')); };
      for (const track of stream.getTracks()) track.onended = () => { if (generation === microphoneGeneration && !disposed) void microphoneError(fail('MICROPHONE_ENDED','Microphone access ended.')); };
      mediaNode.connect(node); node.connect(ctx.destination); state.microphone = 'on'; emit(); return snapshot();
    } catch (error) {
      for (const track of acquired?.getTracks() || []) track.stop();
      pendingStreams.delete(acquired);
      if (generation === microphoneGeneration && !disposed) await microphoneError(error);
      throw error;
    }
  }
  async function reset({detachInput = true} = {}) {
    microphoneGeneration++; speakerGeneration++; contextGeneration++;
    releaseMicrophone(); releaseSpeaker();
    const old = context; context = null; contextReady = null;
    state.microphone = 'off'; state.speaker = 'off'; state.microphoneError = null; state.speakerError = null; state.sampleRate = null;
    emit(); if (old && old.state !== 'closed') await old.close().catch(() => {});
    if (detachInput) await disableInput();
  }
  const unsubscribe = runtime.subscribe(event => {
    if (event.type === 'audio') queueOutput(event);
    else if (event.type === 'state' && ['idle','configured','loading','error','disposed'].includes(event.state?.status)) void reset({detachInput:false});
    else if (event.type === 'state' && event.state?.status === 'paused' && state.microphone !== 'off') void setMicrophone(false).catch(() => {});
  });
  return {observe:snapshot,setMicrophone,setSpeaker,reset,
    async dispose() { if (disposed) return; unsubscribe(); disposed = true; await reset(); }};
}
