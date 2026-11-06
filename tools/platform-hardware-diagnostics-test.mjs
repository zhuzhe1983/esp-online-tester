import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {runInNewContext} from 'node:vm';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDefaultScene} from '../web/platform/catalog.js';
import {parseHardwareDiagnostics} from '../web/platform/hardware-diagnostics.js';
import {createPacing} from '../web/wasm/pacing.mjs';
import {applyExperiments} from '../web/wasm/experiments.mjs';

const catalog=Object.fromEntries(await Promise.all(['products','accessories','firmwares'].map(async name=>[name,JSON.parse(await readFile(new URL(`../web/platform/catalog/${name}.json`,import.meta.url))).items])));
const coreMatrixSnapshot=()=>({schemaVersion:1,board:'corematrix',state:{controllerPowered:true,matrix:{powered:true,enabled:true,pwmSteps:16,frames:12,bytesWritten:512},buzzer:{frequencyHz:1000,duty4095:2048,supplyEnabled:true},grove5vEnabled:false,sdPresent:true}});
test('CoreMatrix observation preserves independent supplies and validates bounded live output',()=>{
  const input=coreMatrixSnapshot(),parsed=parseHardwareDiagnostics(input);assert.deepEqual(parsed,input);
  input.state.matrix.powered=false;assert.equal(parsed.state.matrix.powered,true,'retained output is a copy');
  for(const change of [s=>s.state.controllerPowered=1,s=>s.state.matrix.pwmSteps=17,s=>s.state.matrix.bytesWritten=-1,s=>s.state.buzzer.frequencyHz=NaN,s=>s.state.buzzer.duty4095=4096,s=>delete s.state.sdPresent]) {
    const s=coreMatrixSnapshot();change(s);assert.equal(parseHardwareDiagnostics(s),null);
  }
  const off=coreMatrixSnapshot();off.state.controllerPowered=false;off.state.matrix.powered=false;off.state.buzzer.supplyEnabled=false;
  assert.deepEqual(parseHardwareDiagnostics(off),off,'power off retains the actual register configuration, not fabricated zero settings');
});
const snapshot=()=>({schemaVersion:1,board:'stopwatch',state:{powerLedEnabled:false,downloadLocked:true,pm1ConfiguredHz:100000,pm1LastBusHz:400000,
  display:{sleepOut:true,displayOn:true,brightness:160,brightnessControlEnabled:null,visible:true}},
  findings:[{code:'PM1_I2C_CLOCK_MISMATCH',severity:'warning',count:2,summary:'Clock mismatch',detail:'Host 400000 Hz, PM1 100000 Hz'},
    {code:'PM1_DOWNLOAD_LOCK',severity:'warning',count:1,summary:'Download lock',detail:'DL_LOCK set'},
    {code:'PM1_POWER_LED_DISABLED',severity:'info',count:1,summary:'Power LED disabled',detail:'LED_EN cleared'}],limits:['This is not physical hardware validation.']});

test('diagnostic snapshot validates bounded fields and preserves unknown brightness control',()=>{
  const input=snapshot(),result=parseHardwareDiagnostics(input);assert.deepEqual(result,input);
  result.state.display.visible=false;result.findings[0].detail='edited';result.limits.push('edited');assert.equal(input.state.display.visible,true);assert.notEqual(input.findings[0].detail,'edited');assert.equal(input.limits.length,1);
  const mutations=[v=>v.schemaVersion=2,v=>v.board='stackchan',v=>v.state.powerLedEnabled=1,v=>v.state.pm1LastBusHz=NaN,
    v=>v.state.pm1ConfiguredHz=-1,v=>v.state.pm1LastBusHz=0x1_0000_0000,v=>v.state.display.brightness=256,v=>v.state.display.brightnessControlEnabled='unknown',v=>v.state.display=null,
    v=>v.findings.push(v.findings[0]),v=>v.findings[0].count=Infinity,v=>v.findings[0].count=-1,v=>v.findings[0].severity='success',
    v=>v.findings[0].code='x'.repeat(65),v=>v.findings[0].summary='x'.repeat(201),v=>v.findings[0].detail='x'.repeat(1537),
    v=>v.limits=Array(9).fill('x'),v=>v.limits[0]='x'.repeat(513),v=>v.findings=Array(17).fill(v.findings[0])];
  for(const change of mutations){const invalid=snapshot();change(invalid);assert.equal(parseHardwareDiagnostics(invalid),null);}
  const extra=snapshot();extra.state.untrusted='discard';extra.t='hardware-diagnostics';assert.equal(parseHardwareDiagnostics(extra).t,undefined);assert.equal(parseHardwareDiagnostics(extra).state.untrusted,undefined);
  const unknown=snapshot();unknown.state.pm1LastBusHz=null;assert.equal(parseHardwareDiagnostics(unknown).state.pm1LastBusHz,null);
  const misconfigured=snapshot();misconfigured.state.pm1LastBusHz=40_000_000;
  assert.equal(parseHardwareDiagnostics(misconfigured).state.pm1LastBusHz,40_000_000,'a bad guest divider must not suppress the warning snapshot');
});

// The transport double executes no firmware; its purpose is the public data/lifecycle boundary.
function fixture(){
  const workers=[],events=[];
  const workerFactory=()=>{
    const w={terminate(){},postMessage(m){let response;
      if(m.op==='init')response={ready:true};if(m.op==='create')response={created:true,flashStorage:false};
      if(m.op==='load')response={loaded:m.at===undefined?m.kind:`at${m.at}`,ok:true};
      if(m.op==='start')response={started:true};if(m.op==='resume')response={resumed:true};if(m.op==='stop')response={stopped:0};
      if(m.op==='imu')response={imuApplied:m.inputId,ok:true,time:0};
      if(response)queueMicrotask(()=>w.onmessage?.({data:response}));
    }};workers.push(w);return w;
  };
  const runtime=createPlatformRuntime({catalog,workerFactory,onEvent:e=>events.push(e),fetchImpl:async()=>new Response(new Uint8Array([1,2,3])),baseUrl:'https://fixture.invalid/platform/',assetBaseUrl:'https://fixture.invalid/'});
  const bytes=new Uint8Array(0x10100),view=new DataView(bytes.buffer);
  for(const at of [0,0x10000]){bytes[at]=0xe9;bytes[at+1]=1;bytes[at+12]=9;view.setUint32(at+28,4,true);}
  bytes[0x8000]=0xaa;bytes[0x8001]=0x50;view.setUint32(0x8004,0x10000,true);view.setUint32(0x8008,0x100000,true);
  return {runtime,events,workers,load:()=>runtime.loadFirmware({file:new Blob([bytes])}),send:(value,w=workers.at(-1))=>w.onmessage({data:{text:JSON.stringify({...value,t:'hardware-diagnostics'})}})};
}

test('runtime keeps findings immutable, ignores malformed/duplicate messages, and clears replaced sessions',async()=>{
  const f=fixture();try{
    await f.runtime.configure(createDefaultScene(catalog,'m5stack-stopwatch'));await f.load();await f.runtime.start();
    assert.equal((await f.runtime.observe()).hardwareDiagnostics,null);assert.equal((await f.runtime.capabilities()).hardwareDiagnostics.available,true);
    f.runtime.subscribe(e=>{if(e.type==='hardware-diagnostics'&&e.hardwareDiagnostics)e.hardwareDiagnostics.state.downloadLocked=false;});
    const received=[];f.runtime.subscribe(e=>{if(e.type==='hardware-diagnostics')received.push(e.hardwareDiagnostics);});
    f.send(coreMatrixSnapshot());assert.equal((await f.runtime.observe()).hardwareDiagnostics,null,'other product diagnostics cannot enter this session');
    f.send(snapshot());assert.equal(received[0].state.downloadLocked,true,'subscriber mutation cannot change another subscriber observation');const state=await f.runtime.observe();assert.deepEqual(state.hardwareDiagnostics,snapshot());assert.equal(state.serial,'');
    state.hardwareDiagnostics.findings[0].count=99;f.events.find(e=>e.type==='hardware-diagnostics').hardwareDiagnostics.state.downloadLocked=false;
    assert.equal((await f.runtime.observe()).hardwareDiagnostics.findings[0].count,2);assert.equal((await f.runtime.observe()).hardwareDiagnostics.state.downloadLocked,true);
    f.send(snapshot());const bad=snapshot();bad.state.display.brightnessControlEnabled='unknown';f.send(bad);
    assert.equal(f.events.filter(e=>e.type==='hardware-diagnostics').length,1);
    const old=f.workers.at(-1);await f.runtime.reset();assert.equal((await f.runtime.observe()).hardwareDiagnostics,null);f.send(snapshot(),old);assert.equal((await f.runtime.observe()).hardwareDiagnostics,null);
    f.send(snapshot());await f.load();assert.equal((await f.runtime.observe()).hardwareDiagnostics,null);
    f.send(snapshot());await f.runtime.configure(createDefaultScene(catalog,'m5stack-k134'));assert.equal((await f.runtime.observe()).hardwareDiagnostics,null);assert.equal((await f.runtime.capabilities()).hardwareDiagnostics.available,false);
    await f.load();f.send(snapshot());assert.equal((await f.runtime.observe()).hardwareDiagnostics,null);
  }finally{await f.runtime.dispose();}
});

test('actual worker observes at cold publication points, throttles, force-refreshes stop and isolates other boards',async()=>{
  const source=(await readFile(new URL('../web/wasm/worker.js',import.meta.url),'utf8')).replace(/^import .*;\n/gm,'');
  let wall=0,cycles=0,calls=0,raw=JSON.stringify(snapshot());const messages=[],timers=[];
  const memory=new WebAssembly.Memory({initial:2}),enc=new TextEncoder();
  const wasm={memory,esp32sim_alloc:()=>128,esp32sim_free(){},esp32sim_new:()=>1,esp32sim_delete(){},esp32sim_set_jit(){},esp32sim_boot:()=>0,
    esp32sim_cycles:()=>cycles,esp32sim_insns:()=>cycles,esp32sim_cpu_hz:()=>240e6,esp32sim_out_take:()=>0,
    esp32sim_run(_emu,n){cycles+=n;return 0;},esp32sim_hardware_diagnostics(_emu,p){calls++;const b=enc.encode(raw);new Uint8Array(memory.buffer).set(b,p);return b.length;}};
  const context={TextEncoder,TextDecoder,ArrayBuffer,createPacing,applyExperiments,createJitHost:()=>({imports:{}}),
    performance:{now:()=>wall,timeOrigin:0},Date,postMessage:m=>messages.push(m),setTimeout:(callback,delay)=>timers.push({callback,delay}),
    WebAssembly:{instantiate:async()=>({instance:{exports:wasm}})}};
  runInNewContext(source,context);const send=data=>context.onmessage({data});
  await send({op:'init'});await send({op:'create',board:'stopwatch'});await send({op:'start'});
  assert.equal(calls,1);assert.equal(timers.length,1,'diagnostics add no separate timer');
  wall=100;runInNewContext('collectHardwareDiagnostics()',context);assert.equal(calls,1);
  wall=250;runInNewContext('collectHardwareDiagnostics()',context);assert.equal(calls,2);
  assert.equal(messages.filter(m=>m.text?.includes('hardware-diagnostics')).length,1,'identical snapshots are not reposted');
  const changed=snapshot();changed.findings[0].count=3;raw=JSON.stringify(changed);wall=260;await send({op:'stop'});assert.equal(calls,3);
  assert.equal(messages.filter(m=>m.text?.includes('hardware-diagnostics')).length,2,'stop bypasses throttling');
  raw='{malformed';await send({op:'stop'});assert.equal(messages.at(-1).stopped,0,'bad diagnostic data cannot stop the guest with an error');
  await send({op:'create',board:'stackchan'});await send({op:'start'});wall=1000;await send({op:'stop'});assert.equal(calls,4,'other boards do not query the diagnostic export');
});
