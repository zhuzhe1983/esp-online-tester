import {PcmPlaybackQueue} from './audio-playback.js';
export {PcmPlaybackQueue} from './audio-playback.js';

export class PcmCaptureBuffer {
  constructor({sampleRate, blockFrames = Math.round(sampleRate / 50), maxQueuedSeconds = 2, overflowPolicy = 'error'} = {}) {
    this.sampleRate = sampleRate; this.blockFrames = blockFrames; this.overflowPolicy = overflowPolicy; this.droppedFrames = 0;
    this.capacity = Math.max(blockFrames, Math.ceil(sampleRate * maxQueuedSeconds));
    this.buffer = new Int16Array(this.capacity); this.read = 0; this.length = 0; this.overflow = false;
  }
  append(channels) {
    let count = channels[0]?.length || 0, start = 0;
    if (this.overflow) return false;
    if (count > this.capacity - this.length) {
      if(this.overflowPolicy!=='drop-oldest'){this.overflow=true;return false;}
      const old=Math.min(this.length,count-(this.capacity-this.length));this.read=(this.read+old)%this.capacity;this.length-=old;this.droppedFrames+=old;
      if(count>this.capacity){start=count-this.capacity;this.droppedFrames+=start;count=this.capacity;}
    }
    for (let i = 0; i < count; i++) {
      let value = 0;
      for (const channel of channels) value += Number.isFinite(channel[start+i]) ? channel[start+i] : 0;
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
        else if(data.type==='flush')this.queue.flush();
        else if (data.type === 'pcm') this.port.postMessage({type:'accepted', id:data.id, ...this.queue.enqueue(data), queuedSeconds:this.queue.seconds});
      };
    }
    process(_input, output) {
      const previous = this.queue.buffering, state = this.queue.render(output[0] || []);
      if (++this.turn % 32 === 0 || previous !== state.buffering) this.port.postMessage({type:'playback', ...state});
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
        if (samples) { this.credit = 0; this.port.postMessage({type:'pcm', samples, sampleRate, channels:1, droppedFrames:this.buffer.droppedFrames}, [samples.buffer]); }
      }
      return true;
    }
  }
  registerProcessor('esp-platform-playback', PlaybackProcessor);
  registerProcessor('esp-platform-capture', CaptureProcessor);
}
