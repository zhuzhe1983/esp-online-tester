// Fixed-rate PCM rendering. Buffering changes start latency, never sample rate or pitch.
export class PcmPlaybackQueue {
  constructor({renderRate, maxQueuedSeconds = 2, prebufferSeconds = 0, overflowPolicy = 'error',
    maxPrebufferWaitSeconds = Math.max(0.35, prebufferSeconds * 1.5)} = {}) {
    if (!Number.isInteger(renderRate) || renderRate < 1000 || renderRate > 400000
      || !Number.isFinite(maxQueuedSeconds) || maxQueuedSeconds <= 0 || maxQueuedSeconds > 10
      || !Number.isFinite(prebufferSeconds) || prebufferSeconds < 0 || prebufferSeconds > maxQueuedSeconds
      || !Number.isFinite(maxPrebufferWaitSeconds) || maxPrebufferWaitSeconds <= 0 || maxPrebufferWaitSeconds > 15
      || !['error','drop-oldest'].includes(overflowPolicy))
      throw new RangeError('Invalid PCM playback rate or buffering limits.');
    this.renderRate = renderRate; this.maxQueuedSeconds = maxQueuedSeconds;
    this.overflowPolicy = overflowPolicy;
    // Zero retains the low-level immediate-playback contract. Browser devices opt in.
    this.prebufferSeconds = prebufferSeconds; this.maxPrebufferWaitSeconds = maxPrebufferWaitSeconds;
    this.chunks = []; this.seconds = 0; this.started = false; this.empty = false;
    this.buffering = prebufferSeconds > 0; this.waitedSeconds = 0; this.forceDrain = false;
    this.underflows = 0; this.overflows = 0; this.renderedFrames = 0;
    this.droppedFrames = 0; this.droppedSeconds = 0;
  }
  enqueue({samples, sampleRate, channels = 1}) {
    if (!(samples instanceof Int16Array) || !Number.isInteger(sampleRate) || sampleRate < 1000 || sampleRate > 400000 || ![1, 2].includes(channels) || !samples.length || samples.length % channels) return {accepted:false, code:'INVALID_PCM'};
    let duration = samples.length / channels / sampleRate;
    if (this.seconds + duration > this.maxQueuedSeconds + 1e-9) {
      this.overflows++;
      if (this.overflowPolicy === 'error') return {accepted:false, code:'AUDIO_OUTPUT_OVERFLOW'};
      this.discardOldest(Math.min(this.seconds,this.seconds + duration - this.maxQueuedSeconds));
      if (duration > this.maxQueuedSeconds + 1e-9) {
        const discarded = samples.length / channels - Math.floor(this.maxQueuedSeconds * sampleRate + 1e-9);
        // Copy the retained tail so a huge incoming block does not pin its discarded buffer.
        samples = samples.slice(discarded * channels);
        this.droppedFrames += discarded; this.droppedSeconds += discarded / sampleRate;
        duration = samples.length / channels / sampleRate;
      }
    }
    if (!samples.length) return {accepted:true,droppedFrames:this.droppedFrames,droppedSeconds:this.droppedSeconds};
    this.chunks.push({samples, sampleRate, channels, frames:samples.length / channels, offset:0});
    this.seconds += duration;
    return {accepted:true,droppedFrames:this.droppedFrames,droppedSeconds:this.droppedSeconds};
  }
  discardOldest(seconds) {
    let remaining = seconds;
    while (remaining > 1e-12 && this.chunks.length) {
      const chunk = this.chunks[0], rawFrames = Math.min(chunk.frames - chunk.offset,remaining * chunk.sampleRate);
      const rounded = Math.round(rawFrames), frames = Math.abs(rawFrames - rounded) < 1e-9 ? rounded : rawFrames;
      const duration = frames / chunk.sampleRate;
      chunk.offset += frames; remaining -= duration;
      // During fixed-ratio conversion a partially rendered source frame can remain.
      // Frames are source-frame equivalents; seconds are the exact unplayed duration.
      this.droppedFrames += frames; this.droppedSeconds += duration;
      this.seconds = Math.max(0,this.seconds - duration);
      if (chunk.offset >= chunk.frames - 1e-9) this.chunks.shift();
    }
  }
  clear() {
    this.chunks = []; this.seconds = 0; this.started = false; this.empty = false;
    this.buffering = this.prebufferSeconds > 0; this.waitedSeconds = 0; this.forceDrain = false;
  }
  // At a known pause/end boundary, play even a short tail without another buffering wait.
  // The bounded timer also releases tails when the guest supplies no explicit end marker.
  flush() { this.forceDrain = true; this.buffering = false; this.waitedSeconds = 0; }
  snapshot() {
    return {queuedSeconds:this.seconds, buffering:this.buffering, prebufferSeconds:this.prebufferSeconds,
      waitedSeconds:this.waitedSeconds, underflows:this.underflows, overflows:this.overflows, renderedFrames:this.renderedFrames,
      droppedFrames:this.droppedFrames,droppedSeconds:this.droppedSeconds};
  }
  render(output) {
    for (const channel of output) channel.fill(0);
    const count = output[0]?.length || 0;
    if (this.buffering && this.chunks.length) {
      // Count worklet output time only after the first queued sample. Quiet time before
      // an utterance must not exhaust the short-tail wait before any audio arrives.
      if (this.forceDrain || this.seconds + 1e-9 >= this.prebufferSeconds
        || this.waitedSeconds + 1e-9 >= this.maxPrebufferWaitSeconds) {
        this.buffering = false; this.waitedSeconds = 0;
      } else { this.waitedSeconds += count / this.renderRate; return this.snapshot(); }
    }
    if (this.buffering) return this.snapshot();
    let consumedSeconds = 0;
    for (let frame = 0; frame < count; frame++) {
      const chunk = this.chunks[0];
      if (!chunk) {
        if (this.started && !this.empty) { this.underflows++; this.empty = true; }
        this.buffering = this.prebufferSeconds > 0; this.waitedSeconds = 0; this.forceDrain = false;
        break;
      }
      this.started = true; this.empty = false;
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
    return this.snapshot();
  }
}
