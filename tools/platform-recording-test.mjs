import assert from 'node:assert/strict';
import {encodeRecording,decodeRecording,validateRecording,MAX_RECORDING_BYTES} from '../web/platform/recording.js';
import {installAutomation} from '../web/platform/automation.js';
import {withPlatformTestFixtures,DIAGNOSTIC_FIRMWARE_ID} from './fixtures/platform-catalog.mjs';

// Deterministic transport harness: these tests validate recorder boundaries, lifecycle
// and ordering. They do not claim to execute or verify ESP32 firmware.
const sleep=ms=>new Promise(resolve=>setTimeout(resolve,ms));
const scene={schemaVersion:'1',productId:'m5stack-stopwatch',firmware:{manifestUrl:'https://example.org/demo.json',sha256:'a'.repeat(64)},accessories:[],keyMapping:{KeyA:'button-a'},view:{mode:'studio',deviceView:'3d'}};
const firmware={source:'remote',manifestUrl:scene.firmware.manifestUrl,manifestHash:'a'.repeat(64),artifacts:[{kind:'app',url:'https://example.org/app.bin',sha256:'b'.repeat(64),bytes:1024}],appDirect:false,stubs:[]};
function harness(extraControls=[]){
  const listeners=new Set();let time=0,status='ready',configured=structuredClone(scene),loaded=structuredClone(firmware),blocked=null;
  const calls=[], counters={resets:0,loads:0},product={id:scene.productId,controls:[{id:'button-a',type:'button',pin:2},...extraControls],display:{width:466,height:466,framebufferWidth:468,framebufferHeight:466}};
  const snapshot=()=>structuredClone({status,scene:configured,firmware:loaded,product,storage:runtime.storage,stats:{time},serialByPort:{},frame:{width:468,height:466}});
  const emit=e=>{for(const listener of [...listeners])listener(e);};
  const setStatus=s=>{status=s;emit({type:'state',state:snapshot()});};
  const timer=setInterval(()=>{if(status==='running'){time+=0.01;emit({type:'stats',stats:{time}});}},2);
  const runtime={version:'1.0.0',calls,counters,blockLoad:false,
    subscribe(fn){listeners.add(fn);return()=>listeners.delete(fn);},observe:async()=>snapshot(),capabilities:async()=>({}),
    async configure(next){configured=structuredClone(next);loaded=null;setStatus('configured');return snapshot();},
    async loadFirmware(){counters.loads++;setStatus('loading');if(runtime.blockLoad)await new Promise((resolve,reject)=>{blocked={resolve,reject};});loaded=structuredClone(firmware);setStatus('ready');return snapshot();},
    async cancelPendingLoad(){if(blocked){const pending=blocked;blocked=null;pending.reject(Object.assign(new Error('Cancelled fetch'),{code:'FETCH_ABORTED'}));setStatus('configured');}return snapshot();},
    async reset(){counters.resets++;time=0;calls.length=0;setStatus('loading');setStatus('ready');setStatus('running');return snapshot();},
    async start(){setStatus('running');return snapshot();},async pause(){setStatus('paused');return snapshot();},
    async input(event){
      if(!['running','paused'].includes(status))throw Object.assign(new Error('Not running'),{code:'NOT_RUNNING'});
      if(event.id==='bad')throw Object.assign(new Error('Invalid button'),{code:'UNSUPPORTED_CONTROL'});
      time+=0.001;calls.push(structuredClone(event));
      const receipt={delivery:'queued',guestTime:time,frame:{width:468,height:466}};
      emit({type:'input',event:structuredClone(event),receipt});return {accepted:true,...receipt};
    },
    async capture(){return {width:468,height:466,pixels:new Uint8ClampedArray([1,2,3,255]),frameNumber:1};},
    async dispose(){clearInterval(timer);setStatus('disposed');return snapshot();},
  };
  return runtime;
}
const runtime=harness(),target={location:{href:'https://example.org/platform/index.html'}},api=installAutomation(runtime,{target});
let recording;
try{
  await api.recordingStart();assert.equal(runtime.counters.resets,1,'Recording starts from a reset machine');
  await assert.rejects(()=>api.share(),{code:'RECORDING_NOT_SHAREABLE'});
  await assert.rejects(()=>api.input({type:'button',id:'bad',pressed:true}),{code:'UNSUPPORTED_CONTROL'});
  await api.input({type:'serial',port:'uart1',data:'not recordable'});
  await api.input({type:'button',id:'button-a',pressed:true,ignoredScript:'never stored'});
  await api.input({type:'touch',x:467,y:465,down:true});
  await api.input({type:'touch',x:20,y:50,down:true});
  recording=await api.recordingStop();
  assert.equal(recording.events.length,5,'Only delivered button/touch actions and successful cleanup releases are recorded');
  assert.equal(recording.events[1].x,1);assert.equal(recording.events[1].y,1);
  assert.ok(!JSON.stringify(recording).includes('ignoredScript'));
  assert.equal(recording.events.at(-1).down,false);assert.equal(recording.events.at(-2).pressed,false);
  assert.equal(validateRecording(recording).valid,true);
  assert.deepEqual(decodeRecording(encodeRecording(recording)),recording);
  const shared=new URL(await api.share({mode:'embed'}));
  assert.ok(new URLSearchParams(shared.hash.slice(1)).has('recording'));
  console.log('ok recording: reset, successful acknowledgements only, button semantics, normalized drag and cleanup releases');

  const replay=await api.recordingPlay(recording,{timeoutMs:2000});
  assert.equal(replay.result,'completed');assert.equal(replay.bitExact,false);
  assert.equal(replay.events.length,recording.events.length);
  assert.deepEqual(runtime.calls.map(e=>e.type),recording.events.map(e=>e.type));
  assert.ok(replay.events.every(e=>Number.isFinite(e.actualGuestSeconds)&&e.actualGuestSeconds>=e.scheduledGuestSeconds));
  assert.equal(replay.frameComparison,'equal');
  console.log('ok replay: reset, ordered guest-clock scheduling, actual timestamp receipts and final-frame comparison');

  runtime.storage={pristine:false,status:'saved'};
  const resetsBeforeStorageGuard=runtime.counters.resets;
  await assert.rejects(()=>api.recordingStart(),{code:'RECORDING_STORAGE_NOT_PRISTINE'});
  await assert.rejects(()=>api.recordingPlay(recording),{code:'RECORDING_STORAGE_NOT_PRISTINE'});
  assert.equal(runtime.counters.resets,resetsBeforeStorageGuard,'Persisted private data must not be reset or silently used as a shareable recording initial state');
  runtime.storage={pristine:true,status:'clean'};
  await api.recordingPlay(recording,{timeoutMs:2000});
  console.log('ok recording storage boundary: changed device data rejects before reboot; pristine data remains replayable');


  for(const modify of [r=>{r.events[0].type='serial';},r=>{r.events[1].x=2;},r=>{r.duration=121;},r=>{r.events[1].at=-1;},r=>{r.execute='code';},r=>{r.display.width=100000;},r=>{r.events.at(-1).down=true;},r=>{r.firmware.artifacts[0].sha256='bad';},r=>{r.scene.firmware.sha256='';},r=>{r.events=Array.from({length:501},()=>({at:0,type:'button',id:'button-a',pressed:false}));},r=>{r.scene.keyMapping.KeyA='x'.repeat(40000);}]){
    const invalid=structuredClone(recording);modify(invalid);
    const before=runtime.counters.loads;await assert.rejects(()=>api.recordingPlay(invalid));assert.equal(runtime.counters.loads,before,'Untrusted recordings reject before downloads');
  }
  assert.throws(()=>decodeRecording('a'.repeat(MAX_RECORDING_BYTES*2)));
  assert.throws(()=>decodeRecording(btoa('{"schemaVersion":"1","__proto__":{}}')));
  assert.equal(validateRecording(recording,{scene:{...scene,productId:'other'}}).valid,false);
  const newView={...scene,view:{mode:'embed',deviceView:'2d'},keyMapping:{Space:'button-a'}};
  assert.equal(validateRecording(recording,{scene:newView}).valid,true,'Semantic actions survive presentation/key remapping');
  const differentArtifact=structuredClone(recording);differentArtifact.firmware.artifacts[0].sha256='c'.repeat(64);
  const resetsBeforeMismatch=runtime.counters.resets;
  await assert.rejects(()=>api.recordingPlay(differentArtifact),{code:'RECORDING_FIRMWARE_MISMATCH'});
  assert.equal(runtime.counters.resets,resetsBeforeMismatch,'Changed downloaded artifact hashes never execute');
  console.log('ok recording boundaries: fields, sizes, timestamps, touch bounds, held state, artifact hashes and scene mismatch');

  const slow=structuredClone(recording);slow.events=[{at:0,type:'button',id:'button-a',pressed:true},{at:10,type:'button',id:'button-a',pressed:false}];slow.duration=10;
  const playing=api.recordingPlay(slow,{timeoutMs:5000});
  while(runtime.calls.length===0||!(await api.recordingObserve()).status.includes('playing'))await sleep(2);
  await api.input({type:'button',id:'button-a',pressed:true});
  const cancelled=await playing;assert.equal(cancelled.result,'cancelled');
  assert.deepEqual(runtime.calls.slice(-2),[{type:'button',id:'button-a',pressed:false},{type:'button',id:'button-a',pressed:true}],'Human takeover releases replay-held buttons before delivering the new input');
  await api.input({type:'button',id:'button-a',pressed:false});
  await assert.rejects(()=>api.recordingPlay(slow,{timeoutMs:100}),{code:'RECORDING_TIMEOUT'});
  assert.equal(runtime.calls.at(-1).pressed,false,'Guest-clock timeout releases held input');
  console.log('ok human takeover: aborts guest-clock wait, releases held input, delivers new action');

  await api.recordingStart();await api.input({type:'button',id:'button-a',pressed:true});
  await runtime.pause();
  for(let i=0;i<100&&(await api.recordingObserve()).status==='recording';i++)await sleep(2);
  assert.equal(runtime.calls.at(-1).pressed,false,'Direct runtime pause cleans up held input');
  assert.equal((await api.recordingObserve()).status,'completed');

  await api.configure(scene);runtime.blockLoad=true;
  const preparing=api.recordingPlay(recording,{timeoutMs:1000});
  while((await runtime.observe()).status!=='loading')await sleep(2);
  const resetsBefore=runtime.counters.resets;
  await api.recordingTakeOver();
  assert.equal((await preparing).result,'cancelled');
  assert.equal(runtime.counters.resets,resetsBefore,'Cancelled download never reaches reset or playback');
  assert.equal((await runtime.observe()).status,'configured');runtime.blockLoad=false;
  await api.recordingClear();assert.equal((await api.recordingObserve()).recording,null);
  assert.ok(!new URLSearchParams(new URL(await api.share()).hash.slice(1)).has('recording'),'Clearing UI/agent recording also clears shared replay payload');
  const beforeDispose=api.recordingPlay(slow,{timeoutMs:5000});
  while((await api.recordingObserve()).status!=='playing')await sleep(2);
  await api.dispose();assert.equal((await beforeDispose).result,'cancelled');
  console.log('ok lifecycle: pause releases, preparing download cancels, no execution after cancellation');
}finally{await api.dispose();}
assert.equal((await runtime.observe()).status,'disposed');

{
 const matrixRuntime=harness([{id:'key-r0-c0',type:'matrix-key',row:0,column:0},{id:'key-r1-c2',type:'matrix-key',row:1,column:2}]);
 const matrixApi=installAutomation(matrixRuntime,{target:{location:{href:'https://example.org/platform/'}}});
 try {
  await matrixApi.recordingStart();await matrixApi.press('key-r0-c0',{durationMs:0});await matrixApi.input({type:'matrix-key',row:1,column:2,pressed:true});
  const keys=await matrixApi.recordingStop();assert.equal(keys.events.length,4);assert.ok(keys.events.every(e=>e.type==='matrix-key'));assert.equal(keys.events.at(-1).pressed,false);
  assert.deepEqual(decodeRecording(encodeRecording(keys)),keys);
  await matrixApi.recordingPlay(keys,{timeoutMs:2000});assert.ok(matrixRuntime.calls.every(e=>e.type==='matrix-key'),'Matrix keys never become serial bytes or GPIO buttons');
  const absent=structuredClone(keys);absent.events.forEach(e=>e.column=3);await assert.rejects(()=>matrixApi.recordingPlay(absent),{code:'RECORDING_CONTROL'});
  const invalid=structuredClone(keys);invalid.events[0].row=4;assert.equal(validateRecording(invalid).valid,false);
  console.log('ok matrix keyboard recording: control-ID press translation, hold cleanup, roundtrip/replay, absent-key and coordinate bounds');
 } finally {await matrixApi.dispose();}
}

if(process.argv.includes('--real')){
  const {readFile,writeFile}=await import('node:fs/promises');
  const {createHash}=await import('node:crypto');
  const {Worker}=await import('node:worker_threads');
  const {createPlatformRuntime}=await import('../web/platform/runtime.js');
  const {createDefaultScene}=await import('../web/platform/catalog.js');
  const root=new URL('../',import.meta.url),publicCatalog={};
  for(const name of ['products','accessories','firmwares'])publicCatalog[name]=JSON.parse(await readFile(new URL(`web/platform/catalog/${name}.json`,root),'utf8')).items;
  const catalog=await withPlatformTestFixtures(publicCatalog,'https://platform.example/platform/index.html');
  const fileFetch=async value=>{
    const url=new URL(value),path=url.protocol==='file:'?url:new URL(`web${url.pathname}`,root);
    try{return new Response(await readFile(path));}catch(error){
      if(process.env.ESP32SIM_ROM_DIR&&url.pathname.endsWith('_rom.elf')){const {resolve,basename}=await import('node:path');return new Response(await readFile(resolve(process.env.ESP32SIM_ROM_DIR,basename(url.pathname))));}
      throw error;
    }
  };
  const workerFactory=url=>{
    const local=url.protocol==='file:'?url:new URL(`web${url.pathname}`,root);
    const source=`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(m,t)=>parentPort.postMessage(m,t);await import(${JSON.stringify(local.href)});parentPort.on('message',data=>globalThis.onmessage({data}));`;
    const thread=new Worker(new URL(`data:text/javascript,${encodeURIComponent(source)}`));
    const adapter={postMessage:(...args)=>thread.postMessage(...args),terminate:()=>thread.terminate()};
    thread.on('message',data=>adapter.onmessage?.({data}));thread.on('error',error=>adapter.onerror?.(error));return adapter;
  };
  const realRuntime=createPlatformRuntime({catalog,fetchImpl:fileFetch,workerFactory,baseUrl:'https://platform.example/platform/index.html',assetBaseUrl:'https://platform.example/'});
  const real=installAutomation(realRuntime,{target:{location:{href:'https://platform.example/platform/index.html'}}});
  const advance=async delta=>{
    const target=(await real.observe()).stats.time+delta,start=Date.now();
    while(((await real.observe()).stats.time||0)<target){if(Date.now()-start>30000)throw new Error('Guest clock did not advance in real recording test');await sleep(5);}
  };
  try{
    const selected=createDefaultScene(catalog,'m5stack-stopwatch');
    selected.firmware=structuredClone(catalog.firmwares.find(item=>item.id===DIAGNOSTIC_FIRMWARE_ID).source);
    selected.firmware.manifestUrl=new URL(selected.firmware.manifestUrl,'https://platform.example/platform/recording.js').href;
    await real.configure(selected);await real.loadFirmware();await real.recordingStart();
    await real.waitFor({port:'uart0',serialIncludes:'PLATFORM_READY stopwatch',timeoutMs:30000});
    const button=real.waitFor({port:'uart0',serialIncludes:'BUTTON_A',fromNow:true,timeoutMs:10000});
    await real.press('button-a',{durationMs:30});await button;
    const buttonB=real.waitFor({port:'uart0',serialIncludes:'BUTTON_B',fromNow:true,timeoutMs:10000});
    await real.press('button-b',{durationMs:30});await buttonB;
    const tap=async points=>{
      const observed=real.waitFor({port:'uart0',serialIncludes:'TOUCH down',fromNow:true,timeoutMs:10000});
      for(const [x,y]of points){await real.input({type:'touch',x,y,down:true});await advance(0.06);}
      await observed;const [x,y]=points.at(-1);await real.input({type:'touch',x,y,down:false});await advance(0.06);
    };
    await tap([[234,233]]);
    await tap([[400,233],[320,233],[240,233],[160,233],[60,233]]);
    await tap([[60,233],[160,233],[240,233],[320,233],[400,233]]);
    const captured=await real.recordingStop();
    const originalSerial=(await real.observe()).serialByPort.uart0||'';
    assert.ok(originalSerial.includes('BUTTON_A')&&originalSerial.includes('BUTTON_B'),'Original short presses must both reach firmware');
    assert.equal(captured.events.filter(item=>item.type==='touch'&&item.down).length,11);
    const encoded=encodeRecording(captured);assert.deepEqual(decodeRecording(encoded),captured);
    const played=await real.recordingPlay(decodeRecording(encoded),{timeoutMs:30000});
    assert.equal(played.result,'completed');assert.equal(played.events.length,captured.events.length);
    for(let i=1;i<captured.events.length;i++)assert.ok(played.events[i].actualGuestSeconds-played.events[i-1].actualGuestSeconds+1e-8>=captured.events[i].at-captured.events[i-1].at,'Late delivery must not collapse the original guest-time interval');
    const final=await real.observe(),serial=final.serialByPort.uart0||'';
    assert.ok(serial.includes('BUTTON_A')&&serial.includes('BUTTON_B'),'Both replayed short presses must reach the actual diagnostic firmware');
    assert.ok((serial.match(/TOUCH down/g)||[]).length>=3,'Tap and both trajectories must reach the real touch peripheral');
    const checksum=data=>createHash('sha256').update(data).digest('hex');
    const sourceSha256=Object.fromEntries(await Promise.all(['web/platform/recording.js','web/platform/automation.js','tools/platform-recording-test.mjs'].map(async path=>[path,checksum(await readFile(new URL(path,root)))])));
    const evidence={schemaVersion:'1',command:'node tools/platform-recording-test.mjs --real --receipt <output.json>',backend:'actual esp32sim WASM worker',sourceSha256,firmware:final.firmware,wasmSha256:checksum(await readFile(new URL('web/wasm/esp32sim.wasm',root))),recording:captured,recordingSha256:checksum(JSON.stringify(captured)),eventCount:captured.events.length,gestureEvidence:{tap:true,leftwardTrajectory:true,rightwardTrajectory:true,originalFirmwareTouchDownCount:(originalSerial.match(/TOUCH down/g)||[]).length,firmwareTouchDownCount:(serial.match(/TOUCH down/g)||[]).length,firmwareNavigationSemantics:'not implemented by diagnostic; not claimed'},replay:played,originalSerialSha256:checksum(originalSerial),serialSha256:checksum(serial),assertions:['Actual firmware received original and replayed BUTTON_A and BUTTON_B short presses','Actual firmware reported touch-down for tap and both trajectories','Every nonzero original guest interval was preserved or extended, never collapsed','All replay inputs have worker-acknowledged guest timestamps']};
    const receiptArg=process.argv.indexOf('--receipt');if(receiptArg!==-1){assert.ok(process.argv[receiptArg+1]);await writeFile(process.argv[receiptArg+1],JSON.stringify(evidence,null,2)+'\n');}
    console.log(`ok REAL WASM recording/replay: ${captured.events.length} events; BUTTON_A/B short presses + tap + left/right touch trajectories; guest intervals preserved; navigation semantics not claimed; frame=${played.frameComparison}`);
  }finally{await real.dispose();}
}
