// Bounded PCM transport shared by browser controls, automation and iframe RPC.
const fail=(code,message)=>Object.assign(new Error(message),{code});
export function validatePcmInput({sampleRate,channels=1,port=0,samples}={}) {
  if(!Number.isInteger(sampleRate)||sampleRate<8000||sampleRate>96000||![1,2].includes(channels)||![0,1].includes(port))throw fail('INVALID_AUDIO','PCM requires 8–96 kHz, one/two channels and I2S port 0/1.');
  if(Array.isArray(samples)) {
    if(samples.length>32768||!samples.every(v=>Number.isInteger(v)&&v>=-32768&&v<=32767))throw fail('INVALID_AUDIO','PCM samples must be signed 16-bit integers.');
    samples=Int16Array.from(samples);
  }
  if(!(samples instanceof Int16Array)||!samples.length||samples.length%channels||samples.length/channels>16384)throw fail('INVALID_AUDIO','Supply 1–16384 complete PCM16 frames.');
  const bytes=new Uint8Array(samples.length*2),view=new DataView(bytes.buffer);
  for(let i=0;i<samples.length;i++)view.setInt16(i*2,samples[i],true);
  return {sampleRate,channels,port,frames:samples.length/channels,data:bytes.buffer};
}

export function decodeAudio(buffer) {
  const bytes=new Uint8Array(buffer),view=new DataView(bytes.buffer,bytes.byteOffset,bytes.byteLength);
  if(bytes.length<7||bytes[0]!==2||(bytes.length-5)%2)throw fail('INVALID_AUDIO_OUTPUT','Malformed emulator PCM output.');
  const sampleRate=view.getUint32(1,true);
  if(sampleRate<1000||sampleRate>400000)throw fail('INVALID_AUDIO_OUTPUT','Invalid emulator PCM sample rate.');
  const samples=new Int16Array((bytes.length-5)/2);
  for(let i=0;i<samples.length;i++)samples[i]=view.getInt16(5+i*2,true);
  return {sampleRate,channels:1,samples};
}

export function createAudioStream({request,emit,isRunning,capacitySeconds=10}={}) {
  let available=false,sequence=0,chunks=[],seconds=0,input={enabled:false},output={enabled:true,receivedFrames:0,droppedFrames:0,overflowed:false};
  const snapshot=()=>({available,input:{...input},output:{...output,queuedSeconds:seconds,queuedChunks:chunks.length,capacitySeconds}});
  const changed=()=>emit({type:'audio-state',audio:snapshot()});
  const reset=(supported=false)=>{available=supported;sequence=0;chunks=[];seconds=0;input={enabled:false};output={enabled:true,receivedFrames:0,droppedFrames:0,overflowed:false};changed();};
  const receive=(buffer,guestTime)=>{
    if(!available||!output.enabled)return;
    const chunk={...decodeAudio(buffer),sequence:++sequence,guestTime};
    output.receivedFrames+=chunk.samples.length;
    // Keep a bounded rolling capture. Loss is explicit in every read and state;
    // live subscribers receive the original chunk independently of this history.
    chunks.push({...chunk,samples:chunk.samples.slice()});seconds+=chunk.samples.length/chunk.sampleRate;
    while(seconds>capacitySeconds&&chunks.length) {
      const old=chunks.shift();seconds-=old.samples.length/old.sampleRate;output.droppedFrames+=old.samples.length;output.overflowed=true;
    }
    seconds=Math.max(0,seconds);emit({type:'audio',...chunk});
    if(output.overflowed)changed();
  };
  const configure=async(options={})=>{
    if(!available)throw fail('AUDIO_UNSUPPORTED','Load an emulator with the PCM streaming ABI first.');
    if(!options||typeof options!=='object'||Object.keys(options).some(k=>!['input','output'].includes(k)))throw fail('INVALID_AUDIO','Unknown audio configuration field.');
    if(options.output!==undefined) {
      if(!options.output||Object.keys(options.output).some(k=>k!=='enabled')||typeof options.output.enabled!=='boolean')throw fail('INVALID_AUDIO','Output expects enabled:boolean.');
    }
    if(options.input!==undefined) {
      const next=options.input;
      if(!next||Object.keys(next).some(k=>!['enabled','port','sampleRate','channels'].includes(k))||typeof next.enabled!=='boolean')throw fail('INVALID_AUDIO','Invalid microphone configuration.');
      const port=next.port??input.port??0;
      if(![0,1].includes(port))throw fail('INVALID_AUDIO','I2S port must be 0 or 1.');
      if(next.enabled) {
        validatePcmInput({...next,port,samples:new Int16Array(next.channels??1)});
        if(input.enabled&&input.port!==port){await request('disable',{port:input.port});input={...input,enabled:false};changed();}
        const receipt=await request('configure',{port,sampleRate:next.sampleRate});
        input={...receipt.state,enabled:true,port,sampleRate:next.sampleRate,channels:next.channels??1,routing:'logical-i2s-port'};
      } else {if(input.enabled&&port!==input.port)throw fail('AUDIO_PORT_MISMATCH','Disable the currently active I2S input port.');const receipt=await request('disable',{port});input={...receipt.state,enabled:false,port};}
    }
    if(options.output!==undefined)output.enabled=options.output.enabled;
    changed();return snapshot();
  };
  const push=async(options)=>{
    if(!available||!input.enabled)throw fail('AUDIO_INPUT_DISABLED','Enable PCM input before submitting frames.');
    if(!isRunning())throw fail('NOT_RUNNING','Run the firmware before submitting PCM input.');
    const packet=validatePcmInput(options);
    if(packet.port!==input.port||packet.sampleRate!==input.sampleRate||packet.channels!==input.channels)throw fail('AUDIO_FORMAT_MISMATCH','PCM format differs from the configured input.');
    const result=await request('push',packet,[packet.data]);input={...input,...result.state};
    if(result.acceptedFrames!==packet.frames)throw Object.assign(fail('AUDIO_INPUT_OVERFLOW',`Accepted ${result.acceptedFrames} of ${packet.frames} frames; queued audio was not replaced.`),{details:{acceptedFrames:result.acceptedFrames,requestedFrames:packet.frames,input:{...input}}});
    return {acceptedFrames:result.acceptedFrames,guestTime:result.guestTime,input:{...input}};
  };
  const refresh=async()=>{if(available&&input.enabled){const result=await request('state',{port:input.port});input={...input,...result.state};}return snapshot();};
  const read=({clear=true}={})=>{
    if(typeof clear!=='boolean')throw fail('INVALID_AUDIO','clear must be boolean.');
    const result={...snapshot().output,input:{...input},chunks:chunks.map(c=>({...c,samples:c.samples.slice()}))};
    if(clear){chunks=[];seconds=0;output.droppedFrames=0;output.overflowed=false;}return result;
  };
  return {snapshot,reset,receive,configure,push,read,refresh};
}
