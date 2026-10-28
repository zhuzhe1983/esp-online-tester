// Opt-in live microphone latency bound; ordinary agent PCM remains lossless.
export function realtimeInputPlan({queuedFrames,capacityFrames,sampleRate},frames,maxLatencyMs=250) {
  if(!Number.isInteger(sampleRate)||sampleRate<8000||sampleRate>96000||!Number.isInteger(frames)||frames<1||!Number.isInteger(queuedFrames)||queuedFrames<0||!Number.isInteger(capacityFrames)||capacityFrames<1||!Number.isFinite(maxLatencyMs)||maxLatencyMs<80||maxLatencyMs>2000)throw new Error('Invalid live audio queue bounds');
  const limit=Math.min(capacityFrames,Math.floor(sampleRate*maxLatencyMs/1000)),skipFrames=Math.max(0,frames-limit),keptFrames=frames-skipFrames;
  const clear=queuedFrames+keptFrames>limit;
  return {clear,skipFrames,droppedFrames:(clear?queuedFrames:0)+skipFrames};
}
