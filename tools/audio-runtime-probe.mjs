// Opt-in browser worker probe. Supply firmware URL/hash and local WASM/ROM URLs;
// remote firmware is fetched into RAM only. No synthetic guest inputs or patches.
import {createJitHost} from '../web/wasm/jit.mjs';
const enc=new TextEncoder(),dec=new TextDecoder();
const sha=async bytes=>Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',bytes)),n=>n.toString(16).padStart(2,'0')).join('');
const concatenate=parts=>{const bytes=new Uint8Array(parts.reduce((n,p)=>n+p.length,0));let at=0;for(const p of parts){bytes.set(p,at);at+=p.length;}return bytes;};
onmessage=async({data:options})=>{
  let w,emu;
  try {
    const {firmwareUrl,firmwareSha256,wasmUrl,romUrl,jit=false,seconds=15}=options;
    if(!/^https:\/\//.test(firmwareUrl)||!/^[0-9a-f]{64}$/.test(firmwareSha256)||!Number.isInteger(seconds)||seconds<3||seconds>120)throw Error('Supply explicit pinned firmware and 3..120 guest seconds.');
    const load=async url=>{const response=await fetch(url);if(!response.ok)throw Error(`HTTP ${response.status}`);return new Uint8Array(await response.arrayBuffer());};
    const [firmware,wasm,rom]=await Promise.all([load(firmwareUrl),load(wasmUrl),load(romUrl)]);
    if(await sha(firmware)!==firmwareSha256)throw Error('Firmware SHA mismatch');
    const provenance={firmwareSha256,wasmSha256:await sha(wasm),romSha256:await sha(rom)};
    const logs=[],blockJit=createJitHost(()=>w);
    ({instance:{exports:w}}=await WebAssembly.instantiate(wasm,{env:{...blockJit.imports,host_log:(p,n)=>logs.push(dec.decode(new Uint8Array(w.memory.buffer,p,n)))}}));
    const mem=()=>new Uint8Array(w.memory.buffer);
    const withBytes=(bytes,fn)=>{const p=w.esp32sim_alloc(bytes.length);mem().set(bytes,p);try{return fn(p,bytes.length);}finally{w.esp32sim_free(p,bytes.length);}};
    emu=withBytes(enc.encode('stopwatch'),(p,n)=>w.esp32sim_new(p,n,16,8));
    if(!emu)throw Error('Create failed');
    if(withBytes(rom,(p,n)=>w.esp32sim_load(emu,0,p,n)))throw Error('ROM load failed');
    if(withBytes(firmware,(p,n)=>w.esp32sim_load(emu,5,p,n)))throw Error('Flash load failed');
    if(withBytes(enc.encode('ssid=esp32sim,psk=esp32sim-pass'),(p,n)=>w.esp32sim_wifi(emu,p,n)))throw Error('WiFi setup failed');
    w.esp32sim_audio_output_streaming(emu,1);w.esp32sim_set_jit(emu,jit?1:0);
    if(w.esp32sim_boot(emu,0))throw Error('ROM boot failed');
    const hz=w.esp32sim_cpu_hz(emu),target=seconds*hz,pcm=[],intervals=[],started=performance.now();
    let serial='',frame=null,frames=0,pcmFrames=0,nonzero=0,peak=0,lastSecond=0,lastWall=started,lastCycles=0,previousPcmWall=null;
    const arrivalGaps=[];const rates=new Set();
    const drain=()=>{const count=w.esp32sim_out_take(emu);for(let i=0;i<count;i++){
      const kind=w.esp32sim_out_kind(emu,i),p=w.esp32sim_out_ptr(emu,i),n=w.esp32sim_out_len(emu,i),bytes=mem().subarray(p,p+n);
      if(kind===1){const value=JSON.parse(dec.decode(bytes));if(value.t==='serial'&&value.src==='uart0')serial+=value.data;}
      else if(bytes[0]===1){frame=bytes.slice();frames++;}
      else if(bytes[0]===2){const view=new DataView(bytes.buffer,bytes.byteOffset,bytes.byteLength);rates.add(view.getUint32(1,true));const samples=bytes.slice(5);pcm.push(samples);pcmFrames+=samples.length/2;const now=performance.now();if(previousPcmWall!==null)arrivalGaps.push(now-previousPcmWall);previousPcmWall=now;for(let at=5;at<bytes.length;at+=2){const v=view.getInt16(at,true);if(v)nonzero++;peak=Math.max(peak,Math.abs(v));}}
    }};
    while(w.esp32sim_cycles(emu)<target){
      const current=w.esp32sim_cycles(emu),rc=w.esp32sim_run(emu,Math.min(2000000,target-current),1700000000000+current/hz*1000);
      if(rc)throw Error(`Guest stopped ${rc}`);drain();
      const cycles=w.esp32sim_cycles(emu),second=Math.floor(cycles/hz);
      if(second>lastSecond){const now=performance.now();intervals.push({guestSeconds:cycles/hz,wallSeconds:(now-started)/1000,speed:(cycles-lastCycles)/hz/((now-lastWall)/1000)});lastSecond=second;lastWall=now;lastCycles=cycles;postMessage({type:'progress',jit,guestSeconds:cycles/hz,wallSeconds:(now-started)/1000,insns:w.esp32sim_insns(emu),pcmFrames});await new Promise(resolve=>setTimeout(resolve,0));}
    }
    w.esp32sim_audio_output_flush(emu);drain();const elapsedMs=performance.now()-started;
    arrivalGaps.sort((a,b)=>a-b);
    const result={jit,seconds,elapsedMs,guestCycles:w.esp32sim_cycles(emu),insns:w.esp32sim_insns(emu),computeRatio:seconds/(elapsedMs/1000),intervals,frames,frameSha256:frame?await sha(frame):null,serialSha256:await sha(enc.encode(serial)),serialBytes:enc.encode(serial).length,pcmFrames,pcmNonzero:nonzero,pcmPeak:peak,pcmRates:[...rates],pcmSha256:await sha(concatenate(pcm)),arrivalGapMs:{median:arrivalGaps[Math.floor(arrivalGaps.length*.5)]??null,p95:arrivalGaps[Math.floor(arrivalGaps.length*.95)]??null,max:arrivalGaps.at(-1)??null},jitStats:blockJit.stats,checks:{codecOpen:serial.includes('Open codec device OK'),softApStarted:serial.includes('Web server started'),panic:serial.includes('Guru Meditation Error')||logs.some(v=>v.includes('panic'))},provenance};
    postMessage({type:'result',result});
  }catch(error){postMessage({type:'error',message:error.message});}
  finally{if(emu)w.esp32sim_delete(emu);}
};
