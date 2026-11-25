#!/usr/bin/env node
// Explicit source-built reference workload, never original-factory acceptance.
// Import/help/pure tests do not create a runtime, worker or WASM instance.
import {createHash,randomUUID} from 'node:crypto';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {resolve,dirname} from 'node:path';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {Worker} from 'node:worker_threads';
import {c5CounterDelta} from './c5-lcd273-acceptance.mjs';

const BOARD='waveshare-c5-lcd273',PRODUCT='waveshare-esp32-c5-lcd-2-73',ORIGIN='https://c5-reference.invalid';
const MIB=1024*1024,COUNTER_HZ=240_000_000,ROM_SHA256='49693d745eb4b4e6e6c77be4f5213765d545d8c7a1d9742a8a619bf596c4e27a';
const FACTORY_SHA256='cf10a4358d03898458c16390f3f16ceb0306befe2260160de0919d755b920942';
const root=resolve(dirname(fileURLToPath(import.meta.url)),'..'),hash=b=>createHash('sha256').update(b).digest('hex');
const fail=(code,message)=>Object.assign(new Error(message),{code});
const check=(ok,code,message)=>{if(!ok)throw fail(code,message);};
const delay=ms=>new Promise(done=>setTimeout(done,ms));
const u64=v=>typeof v==='string'&&/^(0|[1-9][0-9]{0,19})$/.test(v)&&BigInt(v)<=0xffff_ffff_ffff_ffffn;
export const SD_PAYLOAD='ESP32-C5 TF card read/write test\r\nWrite and read verification passed.\r\n';
export const SOURCE={repository:'waveshareteam/ESP32-C5-LCD-2.73',revision:'418de818f4f40c5a04c4292dbe293bfaa1351068',
  sdkRevision:'2c211b236707889e8400c4dc5644dd5c4ee071e0',sdkVersion:'5.5.3',
  sd:{project:'example/ESP-IDF-V553/03_sd_card',tree:'f3a52a48572bffab114f5ce2065ab2fb83c7a7a2',mainSha256:'8c4b53d535a2714c2a82d77eef2133417a99bbc43abc452380e6eae68d021a49'},
  display:{project:'example/ESP-IDF-V553/02_lvgl_demo',tree:'6d9c1ecd1b899ab2e545fa48bfb002f8c6d49bdf',mainSha256:'d7cdaa690e35557ef7f5ee3cb9c25ab4b51f60440f5c109942168dcb7784f3e1'}};
const SD_MARKERS=['TF card mounted at /sdcard','Starting TF card read/write test','Test file: /sdcard/test.txt','Write success: 71 bytes','Read success: 71 bytes','Read content:\n',SD_PAYLOAD.replaceAll('\r',''),'========== TF CARD TEST PASS =========='];
const SD_ERRORS=['TF card mount failed:','TF CARD TEST FAIL','Failed to open file for writing:','Failed to open file for reading:','Write failed: expected','Failed to close file after writing:','Read failed: expected','Data verification failed:'];
const TEST_SHORT_NAME=Buffer.from('TEST    TXT');
function testShortName(bytes,at){
  for(let i=0;i<11;i++){const b=bytes[at+i];if(b>0x7f||(b>=0x61&&b<=0x7a?b-0x20:b)!==TEST_SHORT_NAME[i])return false;}
  return true;
}
export const HELP=`Usage: node tools/c5-lcd273-reference-run.mjs --run --kind sd|display
  --runtime-root <production-checkout> --out <new-private-directory>
  --wasm <path-or-url> --wasm-sha256 <hash> --rom <path-or-url> --rom-sha256 <hash>
  --merged <path-or-url> --merged-sha256 <hash> --sd <64-MiB-img-path-or-url> --sd-sha256 <hash>
  [--build-receipt <path-or-url> --build-receipt-sha256 <hash>]
  [--guest-seconds 15] [--wall-timeout-ms 180000]
Use --check-inputs instead of --run for read/hash checks without execution.
--help and module import do not read inputs or start a runtime/worker/WASM.
Each artifact has a caller-supplied independent digest. ROM must match production MP rev100.
Original FactoryOnly bytes belong to c5-lcd273-acceptance.mjs and are refused here.
referenceObserved describes this source-built example only; originalFactoryAccepted is always false.
The original SD is read only on host. Guest writes affect an isolated clone; sd-after.img is new.
No keys, stubs, app-direct, host framebuffer, firmware edits, speed measurements or browser durability test.
Exit 0: reference facts observed / inputs checked; 1: input/load/ABI error; 2: blocked; 3: partial.
`;

export function parseOptions(args){
  const values=new Set(['kind','runtime-root','out','wasm','wasm-sha256','rom','rom-sha256','merged','merged-sha256','sd','sd-sha256','build-receipt','build-receipt-sha256','guest-seconds','wall-timeout-ms']),flags=new Set(['help','run','check-inputs']),o={};
  for(let i=0;i<args.length;i++){const k=args[i].startsWith('--')?args[i].slice(2):'';check(values.has(k)||flags.has(k),'ARGUMENT',`Unknown argument ${args[i]}`);check(!Object.hasOwn(o,k),'ARGUMENT',`Duplicate --${k}`);
    if(flags.has(k))o[k]=true;else{check(i+1<args.length&&!args[i+1].startsWith('--'),'ARGUMENT',`Missing --${k} value`);o[k]=args[++i];}}
  if(o.help)return o;check(Number(!!o.run)+Number(!!o['check-inputs'])===1,'ARGUMENT','Choose --run or --check-inputs explicitly.');check(['sd','display'].includes(o.kind),'ARGUMENT','Required --kind sd|display');
  for(const k of ['wasm','rom','merged','sd']){check(o[k]&&/^[a-f0-9]{64}$/i.test(o[k+'-sha256']||''),'ARGUMENT',`Required --${k} and --${k}-sha256`);o[k+'-sha256']=o[k+'-sha256'].toLowerCase();}
  check(!!o['build-receipt']===!!o['build-receipt-sha256'],'ARGUMENT','Build receipt path/hash must be paired.');
  if(o['build-receipt']){check(/^[a-f0-9]{64}$/i.test(o['build-receipt-sha256']),'ARGUMENT','Invalid build receipt SHA-256');o['build-receipt-sha256']=o['build-receipt-sha256'].toLowerCase();}
  o['runtime-root']=resolve(o['runtime-root']||root);o.out=resolve(o.out||`output/c5-lcd273-reference-${o.kind}-${new Date().toISOString().replace(/[:.]/g,'-')}`);
  for(const [k,n,min,max]of[['guest-seconds',15,1,120],['wall-timeout-ms',180000,1000,900000]]){o[k]=Number(o[k]??n);check(Number.isFinite(o[k])&&o[k]>=min&&o[k]<=max,'ARGUMENT',`Invalid --${k}`);}return o;
}

async function bytesFrom(source,max){
  let bytes;if(/^https?:\/\//.test(source)){const r=await fetch(source,{signal:AbortSignal.timeout(30000)});check(r.ok,'INPUT_FETCH',`Input HTTP ${r.status}`);const chunks=[],reader=r.body.getReader();let n=0;
    try{while(true){const v=await reader.read();if(v.done)break;n+=v.value.length;check(n<=max,'INPUT_SIZE','Input exceeds byte limit.');chunks.push(v.value);}}finally{await reader.cancel().catch(()=>{});}bytes=Buffer.concat(chunks,n);
  }else bytes=await readFile(source.startsWith('file:')?new URL(source):resolve(source));check(bytes.length<=max,'INPUT_SIZE','Input exceeds byte limit.');return bytes;
}
const normalizeUart=s=>String(s).replace(/\x1b\[[0-?]*[ -/]*[@-~]/g,'').replaceAll('\r','');
export function sdUartFacts(raw){
  const text=normalizeUart(raw);let at=0;const ordered=SD_MARKERS.map(marker=>{const found=text.indexOf(marker,at);if(found<0)return false;at=found+marker.length;return true;});
  const errors=SD_ERRORS.filter(marker=>text.includes(marker));
  return {appMain:text.includes('Calling app_main()'),orderedMarkers:SD_MARKERS.map((marker,i)=>({marker,observed:ordered[i]})),
    guestReportedMount:text.includes(SD_MARKERS[0]),guestReportedWriteReadComparePass:ordered.every(Boolean)&&errors.length===0,errorMarkers:errors,
    fatalRuntimeOutputObserved:/Guru Meditation Error|assert failed:|abort\(\) was called/i.test(text),payloadBytes:Buffer.byteLength(SD_PAYLOAD),payloadSha256:hash(SD_PAYLOAD),transportCompletionProven:false};
}

// Bounded read-only FAT32 root /test.txt inspection. It does not mount a guest FS.
// Supports an MBR FAT32 partition or a FAT32 BPB at sector zero; no name inventory.
export function inspectTestFile(input){
  try{
    const b=Buffer.from(input.buffer,input.byteOffset,input.byteLength),need=(at,n)=>check(Number.isSafeInteger(at)&&at>=0&&n>=0&&at+n<=b.length,'FAT_BOUNDS','FAT32 bounds exceed image.');
    need(0,512);const u16=at=>{need(at,2);return b.readUInt16LE(at);},u32=at=>{need(at,4);return b.readUInt32LE(at);},signature=at=>u16(at+510)===0xaa55;
    const bpb=at=>u16(at+11)===512&&u16(at+17)===0&&u16(at+22)===0&&u32(at+36)>0;
    check(signature(0),'FAT_SIGNATURE','Missing disk signature.');let start=0,limit=b.length/512;
    if(!bpb(0)){const parts=[];for(let i=0;i<4;i++){const at=446+i*16;if([0x0b,0x0c].includes(b[at+4]))parts.push({start:u32(at+8),sectors:u32(at+12)});}
      check(parts.length===1,'FAT_PARTITION','Expected one unambiguous FAT32 partition.');start=parts[0].start;limit=parts[0].sectors;check(start>0&&limit>0&&(start+limit)*512<=b.length,'FAT_PARTITION_BOUNDS','Invalid FAT32 partition bounds.');}
    const boot=start*512;need(boot,512);check(signature(boot)&&bpb(boot),'FAT_BPB','Unsupported FAT32 BPB.');
    const spc=b[boot+13],reserved=u16(boot+14),fats=b[boot+16],fatSectors=u32(boot+36),total=u16(boot+19)||u32(boot+32),flags=u16(boot+40),active=flags&0x80?flags&15:0;
    check(spc>0&&spc<=128&&(spc&(spc-1))===0&&reserved>0&&[1,2].includes(fats)&&active<fats&&total<=limit,'FAT_GEOMETRY','Invalid FAT32 geometry.');
    const metadata=reserved+fats*fatSectors,clusters=Math.floor((total-metadata)/spc),clusterBytes=spc*512,fat=boot+(reserved+active*fatSectors)*512,data=boot+metadata*512;
    check(clusters>0&&clusters+2<=fatSectors*128,'FAT_CAPACITY','FAT cannot address this volume.');need(boot,total*512);
    const chain=(first,maxBytes)=>{let c=first;const seen=new Set(),chunks=[];while(true){check(c>=2&&c<=clusters+1&&!seen.has(c),'FAT_CHAIN','Invalid, reserved or cyclic FAT chain.');seen.add(c);check(seen.size*clusterBytes<=maxBytes,'FAT_CHAIN_LIMIT','Target chain exceeds inspection bound.');const at=data+(c-2)*clusterBytes;need(at,clusterBytes);chunks.push(b.subarray(at,at+clusterBytes));const next=u32(fat+c*4)&0x0fff_ffff;if(next>=0x0fff_fff8)break;c=next;}return Buffer.concat(chunks);};
    const dir=chain(u32(boot+44)&0x0fff_ffff,4*MIB),entries=[];
    for(let at=0;at<dir.length;at+=32){if(dir[at]===0)break;if(dir[at]===0xe5||dir[at+11]&8)continue;if(testShortName(dir,at)){
      check(!(dir[at+11]&16),'FAT_TARGET_DIRECTORY','test.txt is a directory.');entries.push({cluster:(dir.readUInt16LE(at+20)*65536+dir.readUInt16LE(at+26))&0x0fff_ffff,bytes:dir.readUInt32LE(at+28)});}}
    const geometry={partitionStartSector:start,sectorBytes:512,clusterBytes};if(!entries.length)return {status:'not-found',path:'/test.txt',geometry};check(entries.length===1,'FAT_TARGET_AMBIGUOUS','Multiple test.txt entries.');
    const entry=entries[0];check(entry.bytes<=MIB,'FAT_TARGET_SIZE','test.txt exceeds inspection bound.');const content=entry.bytes?chain(entry.cluster,2*MIB).subarray(0,entry.bytes):Buffer.alloc(0);
    check(content.length===entry.bytes,'FAT_FILE_TRUNCATED','FAT chain is shorter than file size.');return {status:'observed',path:'/test.txt',bytes:content.length,sha256:hash(content),matchesExpectedPayload:content.equals(Buffer.from(SD_PAYLOAD)),geometry,guestMountInferred:false};
  }catch(e){return {status:'unavailable',code:e.code||'FAT_ERROR',message:e.message,guestMountInferred:false};}
}

export function referenceFactsObserved(kind,{executionStarted,abiVerified,uart,display,sd}){
  if(executionStarted!==true||abiVerified!==true||uart?.truncated||uart?.fatalRuntimeOutputObserved||uart?.appMain!==true)return false;
  const positive=v=>u64(v)&&BigInt(v)>0n;
  if(kind==='sd')return uart.guestReportedWriteReadComparePass===true&&sd?.after?.testFile?.matchesExpectedPayload===true&&sd.counterDelta?.status==='observed'&&positive(sd.counterDelta.storageBlocksLoaded)&&positive(sd.counterDelta.storageBlocksWritten);
  if(kind==='display')return display?.realNonzeroFrame===true&&display.panelVisible===true&&positive(display.panelPixelsWritten)&&positive(display.panelRamWriteCommands);
  return false;
}

export async function main(o){
  const r={schemaVersion:1,kind:o.kind,status:'error',referenceObserved:false,originalFactoryAccepted:false,execution:{started:false},sourceIdentity:{...SOURCE[o.kind],repository:SOURCE.repository,revision:SOURCE.revision,sdkRevision:SOURCE.sdkRevision,sdkVersion:SOURCE.sdkVersion,binaryBuildProvenanceVerified:false},
    inputs:{},workerSessions:[],hardwareDiagnostics:[],frames:[],sd:{},cleanup:{},limits:[
      'Source-built reference facts never upgrade original FactoryOnly acceptance; no app-name/descriptor identity inference.',
      'SD inner command calls/backing-store loads/writes are not all wire commands, completed MISO delivery or FAT mounting.',
      'Original SD is read only on host; the source example can overwrite /test.txt on the isolated clone.',
      'FAT32 inspection reads an exported host image; UART success and cached fread do not establish every payload byte was freshly received over MISO.',
      'Node memory storage provides no browser IndexedDB or cross-process durability evidence.',
      '02 runs its unchanged LVGL demo; its fixed config disables LV_USE_PERF_MONITOR, so a configuration-warning screen is expected, not a functioning benchmark or button-response proof.',
    ]};
  const local=p=>pathToFileURL(resolve(o['runtime-root'],p)),allThreads=[],live=new Set(),input={},uart={uart0:'',emulator:''};let runtime,session,loaded=false,phase='inputs',deadline=Infinity,publication=0,latestFrame=0,uartOverflow=false;
  const started=Date.now(),write=(name,data)=>writeFile(resolve(o.out,name),data,{flag:'wx'});
  const clean=s=>String(s??'').replace(/\/(?:Users|home)\/[^/\s]+/g,'/home/alice').replace(/https?:\/\/[^\s"']+/g,'<url>').slice(0,2048);
  const hardware=(stage,state)=>{const snapshot=state?.hardwareDiagnostics,exact=snapshot?.board===BOARD&&snapshot.schemaVersion===1&&snapshot.counterEncoding==='u64-decimal';
    const entry={sequence:r.hardwareDiagnostics.length+1,stage,epoch:{workerSequence:session?.summary.sequence??null,cardAttachSequence:session?.cardEpoch||null},publicationSequence:null,status:exact?'observed':'unavailable',snapshot:exact?structuredClone(snapshot):null};
    if(exact){const sha=hash(JSON.stringify(snapshot));entry.snapshotSha256=sha;if(session?.published?.sha===sha&&session.published.cardEpoch===session.cardEpoch)entry.publicationSequence=session.published.sequence;const prior=r.hardwareDiagnostics.findLast(v=>v.status==='observed');entry.counterDelta=prior?c5CounterDelta(prior,entry):{status:'unavailable',reason:'no-prior-observation'};}
    else entry.reason='no-exact-C5-snapshot-published';r.hardwareDiagnostics.push(entry);return entry.sequence;};
  const clock=async()=>{check(Date.now()<deadline,'WALL_TIMEOUT','Execution wall-time budget reached.');check(session&&!session.exited,'WORKER_EXITED','No live task worker.');return session.clock();};
  const capture=async state=>{if(!state.frame||state.frame.frameNumber===latestFrame||r.frames.length>=3)return;latestFrame=state.frame.frameNumber;const f=await runtime.capture({format:'rgba'});check(f.width===320&&f.height===320&&f.pixels.length===320*320*4,'FRAME_SHAPE','Expected real LCD273 320x320 RGBA.');let nonBlackPixels=0;const colors=new Set();for(let i=0;i<f.pixels.length;i+=4){if(f.pixels[i]||f.pixels[i+1]||f.pixels[i+2])nonBlackPixels++;colors.add((f.pixels[i]<<16)|(f.pixels[i+1]<<8)|f.pixels[i+2]);}
    if(r.frames.length&&!(nonBlackPixels>0&&colors.size>1))return;
    const filename=`frame-${r.frames.length}.rgba`;await write(filename,f.pixels);r.frames.push({filename,width:f.width,height:f.height,frameNumber:f.frameNumber,nonBlackPixels,uniqueRgbColors:colors.size,rgbaSha256:hash(f.pixels),hardwareDiagnosticsSequence:hardware('frame',await runtime.observe())});};
  const exportCard=async()=>{await runtime.flushSdCard();const b=Buffer.from((await runtime.exportSdCard()).bytes);r.sd.after={bytes:b.length,sha256:hash(b),changedFromInput:hash(b)!==r.inputs.sd.sha256,filename:'sd-after.img',testFile:inspectTestFile(b),hardwareDiagnosticsSequence:hardware('sd-export',await runtime.observe())};await write('sd-after.img',b);};
  await mkdir(dirname(o.out),{recursive:true});await mkdir(o.out);
  try{
    check(o['rom-sha256']===ROM_SHA256,'ROM_PROFILE','Production C5 requires the fixed MP rev100 ROM.');check(o['merged-sha256']!==FACTORY_SHA256,'REFERENCE_IMAGE_REQUIRED','Use the unchanged original-factory acceptance harness for FactoryOnly bytes.');
    for(const [k,max]of[['wasm',256*MIB],['rom',16*MIB],['merged',32*MIB],['sd',64*MIB],...(o['build-receipt']?[['build-receipt',4*MIB]]:[])]){input[k]=await bytesFrom(o[k],max);const digest=hash(input[k]);r.inputs[k]={bytes:input[k].length,sha256:digest,expectedSha256:o[k+'-sha256'],hashMatched:digest===o[k+'-sha256']};check(r.inputs[k].hashMatched,'INPUT_HASH',`${k} digest mismatch.`);}
    check(input.sd.length===64*MIB,'SD_SIZE','Reference input card must be exactly 64 MiB.');r.sd.before={bytes:input.sd.length,sha256:r.inputs.sd.sha256,testFile:inspectTestFile(input.sd)};
    r.sources={harnessSha256:hash(await readFile(fileURLToPath(import.meta.url))),counterHelperSha256:hash(await readFile(new URL('./c5-lcd273-acceptance.mjs',import.meta.url)))};
    for(const p of ['web/platform/runtime.js','web/platform/hardware-diagnostics.js','web/wasm/worker.js'])r.sources[p]=hash(await readFile(local(p)));
    if(!o.run){r.status='inputs-checked';return r;}
    phase='setup';deadline=Date.now()+o['wall-timeout-ms'];const catalog={};for(const k of ['products','accessories','firmwares'])catalog[k]=JSON.parse(await readFile(local(`web/platform/catalog/${k}.json`))).items;
    const p=catalog.products.find(v=>v.id===PRODUCT);check(p?.chip==='esp32c5'&&p.runtime?.board===BOARD&&p.runtime.flashMb===32&&p.runtime.psramMb===8,'CATALOG_PROFILE','Expected exact LCD273 C5 32F8P.');
    const [{createPlatformRuntime},{createDefaultScene},{createDeviceStorage},{createSdCardStorage}]=await Promise.all(['runtime.js','catalog.js','device-storage.js','sd-card-storage.js'].map(p=>import(local('web/platform/'+p).href)));
    const workerFactory=url=>{check(url.href===ORIGIN+'/wasm/worker.js','WORKER_SOURCE','Unexpected production worker path.');const summary={sequence:r.workerSessions.length+1,createCount:0,loads:[],attachments:[],created:null};r.workerSessions.push(summary);
      const script=`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(local('web/wasm/worker.js').href)});parentPort.on('message',data=>globalThis.onmessage({data}));`;
      const thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(script))),clocks=new Map(),attaching=new Map();allThreads.push(thread);live.add(thread);let termination,seq=0;
      const own={summary,exited:false,cardEpoch:0,published:null,firstEpochs:new Set(),clock:()=>new Promise((done,reject)=>{const id=`reference-clock-${++seq}`,timer=setTimeout(()=>{clocks.delete(id);reject(fail('CLOCK_TIMEOUT','No worker clock receipt.'));},10000);clocks.set(id,{done,reject,timer});thread.postMessage({op:'text',data:'{}',inputId:id});})};session=own;
      const adapter={postMessage(m,...t){if(m.op==='init')check(hash(new Uint8Array(m.wasm))===r.inputs.wasm.sha256,'WORKER_WASM','Unexpected WASM bytes.');
        if(m.op==='create'){check(m.board===BOARD&&m.flash_mb===32&&m.psram_mb===8&&m.appDirect===false&&m.jit===false&&!(m.experiments?.length)&&!(m.stubs?.length),'WORKER_PROFILE','Only exact interpreter cold-ROM profile is allowed.');summary.createCount++;summary.profile={board:m.board,flashMiB:32,psramMiB:8,appDirect:false,jit:false};}
        if(m.op==='load'){check(m.at===undefined&&[0,5].includes(m.kind),'WORKER_LOAD','Only ROM kind0 and caller merged kind5 are allowed.');const k=m.kind===0?'rom':'merged',digest=hash(new Uint8Array(m.data));check(digest===r.inputs[k].sha256,'WORKER_LOAD_HASH','Loaded bytes differ from caller pin.');summary.loads.push({kind:m.kind,sha256:digest,bytes:m.data.byteLength});}
        if(m.op==='start')check(m.appDirect===false,'WORKER_BOOT_MODE','App-direct is forbidden.');check(m.op!=='stub','WORKER_STUB','Stubs are forbidden.');
        if(m.op==='sd-attach'){const a={sequence:summary.attachments.length+1,bytes:m.data.byteLength,acknowledged:false};summary.attachments.push(a);attaching.set(m.requestId,a);}thread.postMessage(m,...t);},terminate(){return termination ||= thread.terminate();}};
      thread.on('message',m=>{if(m.created!==undefined)summary.created={ok:m.created,clockHz:m.clockHz};if(attaching.has(m.sdReply)){const a=attaching.get(m.sdReply);attaching.delete(m.sdReply);a.acknowledged=m.ok===true;if(a.acknowledged){own.cardEpoch=a.sequence;own.published=null;}}
        if(clocks.has(m.inputQueued)){const c=clocks.get(m.inputQueued);clocks.delete(m.inputQueued);clearTimeout(c.timer);if(Number.isFinite(m.time)&&m.time>=0)c.done(m.time);else c.reject(fail('CLOCK_INVALID','Invalid worker clock.'));}else adapter.onmessage?.({data:m});});
      thread.on('error',e=>adapter.onerror?.(e));thread.on('exit',code=>{own.exited=true;summary.exitCode=code;live.delete(thread);for(const c of clocks.values()){clearTimeout(c.timer);c.reject(fail('WORKER_EXITED','Task worker exited.'));}clocks.clear();});return adapter;};
    const paths=new Map([['/wasm/esp32sim.wasm','wasm'],['/wasm/fw/esp32c5_rev100_rom.elf','rom'],['/reference.bin','merged']]);
    const fetchImpl=async value=>{const u=new URL(value,ORIGIN);check(u.origin===ORIGIN&&paths.has(u.pathname)&&!u.search,'ASSET_REQUEST','Undeclared asset/fallback requested.');return new Response(input[paths.get(u.pathname)]);};
    const store={indexedDB:null,BroadcastChannel:null,dbName:'c5-reference-'+randomUUID()};
    runtime=createPlatformRuntime({catalog,workerFactory,fetchImpl,baseUrl:ORIGIN+'/platform/',assetBaseUrl:ORIGIN+'/',requestTimeout:10000,deviceStorage:createDeviceStorage(store),sdStorage:createSdCardStorage({...store,dbName:store.dbName+'-sd'}),onEvent:e=>{
      if(e.type==='serial'&&Object.hasOwn(uart,e.source)){if(Buffer.byteLength(uart[e.source])+Buffer.byteLength(e.data)>5*MIB)uartOverflow=true;else uart[e.source]+=e.data;}
      if(e.type==='hardware-diagnostics'&&session){if(e.hardwareDiagnostics){session.published={sequence:++publication,sha:hash(JSON.stringify(e.hardwareDiagnostics)),cardEpoch:session.cardEpoch};if(!session.firstEpochs.has(session.cardEpoch)){session.firstEpochs.add(session.cardEpoch);hardware('first-publication',{hardwareDiagnostics:e.hardwareDiagnostics});}}else session.published=null;}
    }});
    const scene=createDefaultScene(catalog,PRODUCT),source={url:ORIGIN+'/reference.bin',sha256:r.inputs.merged.sha256,bootMode:'rom'};scene.firmware=source;await runtime.configure(scene);r.capabilities=await runtime.capabilities();await runtime.attachSdCard({bytes:new Uint8Array(input.sd)});
    phase='load';const state=await runtime.loadFirmware(source);loaded=true;r.load={bootMode:state.firmware.bootMode,appDirect:state.firmware.appDirect,resetMode:state.firmware.resetMode,executionMode:state.firmware.executionMode,
      metadata:state.firmware.metadata,flashLayout:state.firmware.flashLayout,artifacts:state.firmware.artifacts.map(({kind,bytes,sha256})=>({kind,bytes,sha256}))};
    check(r.load.bootMode==='rom'&&r.load.appDirect===false&&r.load.resetMode==='rom-reset'&&r.load.executionMode==='wasm-interpreter','ABI_MODE','Unexpected execution mode.');
    const s=session.summary;check(s.createCount===1&&s.created?.ok===true&&s.created.clockHz===COUNTER_HZ&&s.loads.length===2&&s.loads[0].kind===0&&s.loads[1].kind===5,'ABI_PROFILE','Expected one nominal240MHz C5 handle, ROM0 before merged5.');r.load.nominalCounterHz=COUNTER_HZ;r.load.xtal={requiredHz:48_000_000,liveObserved:false};hardware('load-ready',state);
    check(hash((await runtime.exportSdCard()).bytes)===r.inputs.sd.sha256,'SD_IMPORT','Runtime clone differs from original SD bytes.');
    r.load.abiVerified=true;phase='execution';check(Date.now()<deadline,'WALL_TIMEOUT','Execution wall-time budget reached before guest startup.');await runtime.start();r.execution.started=true;hardware('started',await runtime.observe());
    while(true){check(Date.now()<deadline,'WALL_TIMEOUT','Execution wall-time budget reached.');const state=await runtime.observe();check(state.status!=='error','GUEST_BLOCKED',state.lastError?.message||'Runtime error.');check(!uartOverflow,'UART_LIMIT','UART capture exceeded its bound.');await capture(state);
      const facts=sdUartFacts(uart.uart0);check(!facts.fatalRuntimeOutputObserved,'GUEST_PANIC','Guest panic/assert/abort appeared in UART.');if(o.kind==='sd'&&(facts.guestReportedWriteReadComparePass||facts.errorMarkers.length)){r.stopReason=facts.errorMarkers.length?'guest-reported-failure':'source-routine-pass-marker';break;}
      if(o.kind==='display'&&facts.appMain&&r.frames.some(f=>f.nonBlackPixels>0&&f.uniqueRgbColors>1)){r.stopReason='real-frame-observed';break;}
      if(await clock()>=o['guest-seconds']){r.stopReason='guest-budget-reached';break;}await delay(50);
    }
    await runtime.pause();hardware('paused',await runtime.observe());await capture(await runtime.observe());await exportCard();
  }catch(e){r.failure={phase,code:e.code||'ERROR',message:clean(e.message)};}
  finally{
    if(runtime){try{const s=await runtime.observe();if(s.status==='running')await runtime.pause();hardware('finally',await runtime.observe());}catch(e){r.cleanup.pauseError=clean(e.message);}
      if(loaded&&!r.sd.after)try{await exportCard();}catch(e){r.sd.exportError={code:e.code||'ERROR',message:clean(e.message)};}
      try{const s=await runtime.observe();r.lastRuntime={status:s.status,guestTime:s.stats?.time??null,lastError:s.lastError?{code:s.lastError.code,message:clean(s.lastError.message)}:null};}catch{}
      try{await runtime.dispose();r.cleanup.runtimeDisposed=true;}catch(e){r.cleanup.disposeError=clean(e.message);}}
    await Promise.allSettled(allThreads.map(t=>t.terminate()));r.cleanup.activeWorkers=live.size;r.cleanup.workersExited=live.size===0&&r.workerSessions.every(s=>s.exitCode!==undefined);
    r.uart=sdUartFacts(uart.uart0);r.uart.truncated=uartOverflow;r.uart.streams={};for(const[k,v]of Object.entries(uart)){await write(k+'.log',v);r.uart.streams[k]={bytes:Buffer.byteLength(v),sha256:hash(v),filename:k+'.log'};}
    const observed=r.hardwareDiagnostics.filter(v=>v.status==='observed'),first=observed[0],last=observed.at(-1);r.sd.counterDelta=first&&last?c5CounterDelta(first,last):{status:'unavailable',reason:'two-published-card-snapshots-required'};
    const d=last?.snapshot?.state?.display;
    r.display={realNonzeroFrame:r.frames.some(f=>f.nonBlackPixels>0&&f.uniqueRgbColors>1),panelVisible:d?.visible===true&&d.resetHigh===true&&d.sleepOut===true&&d.displayOn===true&&d.brightness>0,
      panelPixelsWritten:u64(d?.pixelsWritten)?d.pixelsWritten:null,panelRamWriteCommands:u64(d?.ramWriteCommands)?d.ramWriteCommands:null,buttonResponseVerified:false,speedMeasured:false};
    r.referenceObserved=referenceFactsObserved(o.kind,{executionStarted:r.execution.started,abiVerified:r.load?.abiVerified,uart:r.uart,display:r.display,sd:r.sd});
    if(!r.cleanup.workersExited)r.failure={phase:'cleanup',code:'WORKERS_ALIVE',message:'Task-owned worker exit not observed.'};
    if(r.status!=='inputs-checked')r.status=r.failure&&['inputs','setup','load','cleanup'].includes(r.failure.phase)?'error':r.referenceObserved&&!r.failure?'reference-observed':r.uart.appMain||r.frames.length?'partial':'blocked';
    r.originalFactoryAccepted=false;r.wallSeconds=(Date.now()-started)/1000;await write('result.json',JSON.stringify(r,null,2)+'\n');
  }return r;
}

if(process.argv[1]&&pathToFileURL(resolve(process.argv[1])).href===import.meta.url){try{const o=parseOptions(process.argv.slice(2));if(o.help)process.stdout.write(HELP);else{const r=await main(o);process.stdout.write(JSON.stringify({status:r.status,referenceObserved:r.referenceObserved,originalFactoryAccepted:false,failure:r.failure??null,workersExited:r.cleanup.workersExited})+'\n');process.exitCode=({'reference-observed':0,'inputs-checked':0,error:1,blocked:2,partial:3})[r.status]??1;}}catch(e){process.stderr.write(JSON.stringify({status:'error',code:e.code||'ERROR',message:e.code==='ARGUMENT'?e.message:'Could not prepare/retain reference evidence at caller-selected paths.',originalFactoryAccepted:false})+'\n');process.exitCode=1;}}
