import {readFileSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {pathToFileURL} from 'node:url';
const {createJitHost}=await import(pathToFileURL(process.argv[2]));
const bytes=readFileSync(process.argv[3]);
const every=Number(process.argv[4]||0);let w,gcs=0,peakHeap=0,lastSample=0;
const start=performance.now(),host=createJitHost(()=>w),release=host.imports.host_jit_release;
let nextGc=every;
function sample(label) {const m=process.memoryUsage();peakHeap=Math.max(peakHeap,m.heapUsed);console.log(JSON.stringify({label,seconds:(performance.now()-start)/1000,compiled:host.stats.compiled,released:host.stats.released,heap:m.heapUsed,rss:m.rss,external:m.external,peakHeap,gcs,liveModules:host.stats.compiled-host.stats.released}));}
host.imports.host_jit_release=slot=>{release(slot);if(every&&host.stats.released>=nextGc){global.gc();gcs++;nextGc+=every;}if(host.stats.released>=lastSample+64){lastSample=host.stats.released;sample('progress');}};
w=(await WebAssembly.instantiate(bytes,{env:{...host.imports,host_profile_now:()=>performance.now(),host_log(p,n){console.error(new TextDecoder().decode(new Uint8Array(w.memory.buffer,p,n)));}}})).instance.exports;
console.log(JSON.stringify({node:process.version,v8:process.versions.v8,platform:process.platform,arch:process.arch,wasmSha256:createHash('sha256').update(bytes).digest('hex'),every}));
const waveform=process.argv[5]==='waveform';const count=waveform?w.esp32sim_test_gpio_waveform():w.esp32sim_test_block_jit();sample('complete');console.log(JSON.stringify({count,stats:host.stats,maxRssKiB:process.resourceUsage().maxRSS}));
if(!count||host.stats.failed||!host.stats.compiled||host.stats.compiled!==host.stats.released)throw Error('incomplete test');
