// Fixed-rate PCM rendering: sample conversion preserves source duration and pitch.
export class PcmPlaybackQueue {
  constructor({renderRate, maxQueuedSeconds = 2} = {}) {
    this.renderRate = renderRate;
    this.maxQueuedSeconds = maxQueuedSeconds;
    this.chunks = []; this.seconds = 0; this.started = false; this.empty = false;
    this.underflows = 0; this.overflows = 0; this.renderedFrames = 0;
  }
  enqueue({samples, sampleRate, channels = 1}) {
    if (!(samples instanceof Int16Array) || !Number.isInteger(sampleRate) || sampleRate < 8000 || sampleRate > 96000 || ![1, 2].includes(channels) || !samples.length || samples.length % channels) return {accepted:false, code:'INVALID_PCM'};
    const duration = samples.length / channels / sampleRate;
    if (this.seconds + duration > this.maxQueuedSeconds + 1e-9) { this.overflows++; return {accepted:false, code:'AUDIO_OUTPUT_OVERFLOW'}; }
    this.chunks.push({samples, sampleRate, channels, frames:samples.length / channels, offset:0});
    this.seconds += duration; this.started = true; this.empty = false;
    return {accepted:true};
  }
  clear() { this.chunks = []; this.seconds = 0; this.started = false; this.empty = false; }
  render(output) {
    for (const channel of output) channel.fill(0);
    let consumedSeconds = 0;
    const count = output[0]?.length || 0;
    for (let frame = 0; frame < count; frame++) {
      const chunk = this.chunks[0];
      if (!chunk) {
        if (this.started && !this.empty) { this.underflows++; this.empty = true; }
        break;
      }
      const index = Math.floor(chunk.offset), fraction = chunk.offset - index;
      const next = this.chunks[1];
      for (let channel = 0; channel < output.length; channel++) {
        const lane = Math.min(channel, chunk.channels - 1);
        const a = chunk.samples[index * chunk.channels + lane];
        const b = index + 1 < chunk.frames ? chunk.samples[(index + 1) * chunk.channels + lane]
          : next ? next.samples[Math.min(channel, next.channels - 1)] : a;
        output[channel][frame] = (a + (b - a) * fraction) / 32768;
      }
      let stepSeconds = 1 / this.renderRate;
      // Carry fractional source-frame time across chunks, including a source-rate change.
      while (stepSeconds > 0 && this.chunks.length) {
        const current = this.chunks[0], remaining = (current.frames - current.offset) / current.sampleRate;
        const used = Math.min(stepSeconds, remaining);
        current.offset += used * current.sampleRate;
        consumedSeconds += used; stepSeconds -= used;
        if (current.offset >= current.frames - 1e-9) this.chunks.shift();
        else break;
      }
      this.renderedFrames++;
    }
    this.seconds = Math.max(0, this.seconds - consumedSeconds);
    return {queuedSeconds:this.seconds, underflows:this.underflows, overflows:this.overflows, renderedFrames:this.renderedFrames};
  }
}

export class PcmCaptureBuffer {
  constructor({sampleRate, blockFrames = Math.round(sampleRate / 50), maxQueuedSeconds = 2} = {}) {
    this.sampleRate = sampleRate; this.blockFrames = blockFrames;
    this.capacity = Math.max(blockFrames, Math.ceil(sampleRate * maxQueuedSeconds));
    this.buffer = new Int16Array(this.capacity); this.read = 0; this.length = 0; this.overflow = false;
  }
  append(channels) {
    const count = channels[0]?.length || 0;
    if (this.overflow || count > this.capacity - this.length) { this.overflow = true; return false; }
    for (let i = 0; i < count; i++) {
      let value = 0;
      for (const channel of channels) value += Number.isFinite(channel[i]) ? channel[i] : 0;
      value = Math.max(-1, Math.min(1, value / channels.length));
      this.buffer[(this.read + this.length + i) % this.capacity] = Math.round(value < 0 ? value * 32768 : value * 32767);
    }
    this.length += count; return true;
  }
  take() {
    if (this.length < this.blockFrames) return null;
    const result = new Int16Array(this.blockFrames);
    for (let i = 0; i < result.length; i++) result[i] = this.buffer[(this.read + i) % this.capacity];
    this.read = (this.read + result.length) % this.capacity; this.length -= result.length;
    return result;
  }
}

if (typeof registerProcessor === 'function') {
  class PlaybackProcessor extends AudioWorkletProcessor {
    constructor(options) {
      super(); this.queue = new PcmPlaybackQueue({renderRate:sampleRate, ...options.processorOptions}); this.turn = 0;
      this.port.onmessage = ({data}) => {
        if (data.type === 'clear') this.queue.clear();
        else if (data.type === 'pcm') this.port.postMessage({type:'accepted', id:data.id, ...this.queue.enqueue(data), queuedSeconds:this.queue.seconds});
      };
    }
    process(_input, output) {
      const previous = this.queue.underflows, state = this.queue.render(output[0] || []);
      if (++this.turn % 16 === 0 || previous !== state.underflows) this.port.postMessage({type:'playback', ...state});
      return true;
    }
  }
  class CaptureProcessor extends AudioWorkletProcessor {
    constructor(options) {
      super(); this.buffer = new PcmCaptureBuffer({sampleRate, ...options.processorOptions}); this.credit = 1; this.active = true;
      this.port.onmessage = ({data}) => {
        if (data.type === 'ack') this.credit = 1;
        else if (data.type === 'stop') this.active = false;
      };
    }
    process(input, output) {
      // Keep the processor connected, without feeding microphone sound into speakers.
      for (const channel of output[0] || []) channel.fill(0);
      if (!this.active) return true;
      if (!this.buffer.append(input[0] || [])) {
        this.active = false; this.port.postMessage({type:'error', code:'AUDIO_INPUT_OVERFLOW'}); return true;
      }
      if (this.credit) {
        const samples = this.buffer.take();
        if (samples) { this.credit = 0; this.port.postMessage({type:'pcm', samples, sampleRate, channels:1}, [samples.buffer]); }
      }
      return true;
    }
  }
  registerProcessor('esp-platform-playback', PlaybackProcessor);
  registerProcessor('esp-platform-capture', CaptureProcessor);
}
