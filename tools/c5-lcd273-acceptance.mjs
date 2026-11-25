#!/usr/bin/env node
// Opt-in acceptance observations through the public runtime and production worker.
// Importing this module, or --help, never creates a worker or instantiates WASM.
import {createHash, randomUUID} from 'node:crypto';
import {readFile, writeFile, mkdir} from 'node:fs/promises';
import {dirname, resolve} from 'node:path';
import {fileURLToPath, pathToFileURL} from 'node:url';
import {Worker} from 'node:worker_threads';
import {execFile} from 'node:child_process';
import {promisify} from 'node:util';

const PRODUCT = 'waveshare-esp32-c5-lcd-2-73', BOARD = 'waveshare-c5-lcd273';
const ORIGIN = 'https://c5-lcd273-acceptance.invalid';
const PINS = {prev:2, next:3, boot:28}, MIB = 1024 * 1024;
// Worker clockHz is the nominal cycle/counter rate, not the XTAL source.
// C5Bus::CPU_HZ / Soc::CPU_HZ / esp32sim_cpu_hz currently publish 240 MHz.
const NOMINAL_COUNTER_HZ = 240_000_000, EXPECTED_XTAL_HZ = 48_000_000;
const SD_COUNTERS = ['nativeCommandCalls','storageBlocksLoaded','storageBlocksWritten'];
const u64 = value => typeof value==='string' && /^(0|[1-9][0-9]{0,19})$/.test(value) && (value.length<20 || value<='18446744073709551615');
const PINNED = {
  factory:'cf10a4358d03898458c16390f3f16ceb0306befe2260160de0919d755b920942',
  rom:'49693d745eb4b4e6e6c77be4f5213765d545d8c7a1d9742a8a619bf596c4e27a',
  sd:'a0e026d9aeafab624884455d25147647980bd6a83011618da04140659c28bfc6',
};
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const delay = ms => new Promise(done => setTimeout(done, ms));
const fault = (code, message) => Object.assign(new Error(message), {code});
const requireValue = (ok, code, message) => { if (!ok) throw fault(code, message); };

// A driver-assisted run is a separate result even if its guest produces pixels.
export function isStrictC5Execution(value) {
  return !!value && value.c5WifiDriverCompat===undefined && value.driverCompatibility==null;
}
const mainFile = fileURLToPath(import.meta.url), defaultRoot = resolve(dirname(mainFile), '..');
const execute = promisify(execFile);

export const HELP = `Usage:
  node tools/c5-lcd273-acceptance.mjs --run \\
    --runtime-root <production-checkout> --wasm <path-or-url> --wasm-sha256 <sha256> \\
    --rom <path-or-url> --factory <path-or-url> --sd <64-MiB-image-path-or-url> \\
    --out <new-private-output-directory> [--expectations <json>]

Modes (choose exactly one):
  --help                 No input reads, worker, WASM or guest startup.
  --check-inputs         Read/hash inputs and check source/catalog; do not execute.
  --run                  Explicitly start the real production worker and guest.

Options:
  --runtime-root <path>  Checkout supplying runtime, catalog and worker (default: script checkout).
  --wasm <path-or-url>   Caller-selected production esp32sim.wasm; never rebuilt here.
  --wasm-sha256 <hash>   Required independent pin for those exact WASM bytes.
  --rom <path-or-url>    Unmodified C5 MP rev100 ROM; fixed SHA-256 checked.
  --factory <path-or-url> Unmodified FactoryOnly_20260724 merged image; fixed SHA-256 checked.
  --sd <path-or-url>     Existing fixed 64 MiB card; fixed SHA-256 checked; read only on host.
  --out <path>           Must not exist. Default: output/c5-lcd273-acceptance-<UTC>.
  --expectations <json>  Optional firmware-specific button UART markers/quiet screen regions.
  --boot-guest-seconds <n> Maximum guest time per boot (default 15, range 1..120).
  --wall-timeout-ms <n>  Total execution budget (default 180000, range 1000..900000).
  --save-uart            Save private raw UART/emulator tails under the output directory.
  --save-sd              Save an exported card copy there; never overwrite the input image.

Result JSON distinguishes blocked, partial and error; exit 2, 3 and 1 respectively.
--check-inputs success exits 0 and means inputs-checked only, never product acceptance.
Records exact C5 cold LCD/SD snapshots and same-worker/card u64 counter differences.
Counters do not prove completed MISO reads or FAT mount; Node storage is memory-only.
Consequently this harness cannot report complete TF or product acceptance as passed.
See tools/c5-lcd273-acceptance.README.md for the evidence contract and limitations.
`;

export function parseOptions(args) {
  const values = new Set(['runtime-root','wasm','wasm-sha256','rom','factory','sd','out','expectations','boot-guest-seconds','wall-timeout-ms']);
  const flags = new Set(['help','check-inputs','run','save-uart','save-sd']), options = {};
  for (let i=0; i<args.length; i++) {
    const key = args[i].startsWith('--') ? args[i].slice(2) : '';
    requireValue(values.has(key) || flags.has(key), 'ARGUMENT', `Unknown argument: ${args[i]}`);
    requireValue(!Object.hasOwn(options,key), 'ARGUMENT', `Duplicate --${key}`);
    if (flags.has(key)) options[key] = true;
    else {
      requireValue(i+1<args.length && !args[i+1].startsWith('--'), 'ARGUMENT', `Missing value for --${key}`);
      options[key] = args[++i];
    }
  }
  if (options.help) return options;
  requireValue(Number(!!options.run)+Number(!!options['check-inputs'])===1, 'ARGUMENT', 'Choose --run or --check-inputs explicitly.');
  for (const key of ['wasm','wasm-sha256','rom','factory','sd']) requireValue(options[key], 'ARGUMENT', `Required --${key}`);
  requireValue(/^[a-f0-9]{64}$/i.test(options['wasm-sha256']), 'ARGUMENT', '--wasm-sha256 needs 64 hex digits.');
  options['wasm-sha256'] = options['wasm-sha256'].toLowerCase();
  options['runtime-root'] = resolve(options['runtime-root'] || defaultRoot);
  options.out = resolve(options.out || `output/c5-lcd273-acceptance-${new Date().toISOString().replace(/[:.]/g,'-')}`);
  for (const [key,value,min,max] of [['boot-guest-seconds',15,1,120],['wall-timeout-ms',180000,1000,900000]]) {
    options[key] = Number(options[key] ?? value);
    requireValue(Number.isFinite(options[key]) && options[key]>=min && options[key]<=max, 'ARGUMENT', `Invalid --${key} (${min}..${max}).`);
  }
  return options;
}

async function inputBytes(source, maxBytes) {
  if (/^https?:\/\//.test(source)) {
    const response = await fetch(source, {signal:AbortSignal.timeout(30000)});
    requireValue(response.ok, 'INPUT_FETCH', `Input download returned HTTP ${response.status}.`);
    requireValue(!response.headers.has('content-length') || Number(response.headers.get('content-length'))<=maxBytes, 'INPUT_SIZE', 'Input exceeds its byte limit.');
    const reader = response.body.getReader(), chunks = []; let bytes = 0;
    try {
      while (true) { const next=await reader.read(); if(next.done)break; bytes+=next.value.length; requireValue(bytes<=maxBytes,'INPUT_SIZE','Input exceeds its byte limit.'); chunks.push(next.value); }
    } finally { await reader.cancel().catch(()=>{}); }
    return Buffer.concat(chunks,bytes);
  }
  const bytes = await readFile(source.startsWith('file:') ? new URL(source) : resolve(source));
  requireValue(bytes.length<=maxBytes, 'INPUT_SIZE', 'Input exceeds its byte limit.');
  return bytes;
}

function expectations(value = {}) {
  requireValue(value && typeof value==='object' && !Array.isArray(value), 'EXPECTATIONS', 'Expectations must be a JSON object.');
  requireValue(Object.keys(value).every(k=>['schemaVersion','factorySha256','appMainIncludes','controls'].includes(k)), 'EXPECTATIONS', 'Unknown expectations field.');
  requireValue(value.schemaVersion===undefined || value.schemaVersion===1, 'EXPECTATIONS', 'Unsupported expectations schema.');
  requireValue(value.factorySha256===undefined || value.factorySha256===PINNED.factory, 'EXPECTATIONS', 'Expectations must target the fixed original factory.');
  const marker=value.appMainIncludes ?? 'Calling app_main()';
  requireValue(typeof marker==='string' && marker.length>=8 && marker.length<=256, 'EXPECTATIONS', 'appMainIncludes must be a specific nonempty UART marker.');
  const controls=value.controls ?? {};
  requireValue(controls && typeof controls==='object' && !Array.isArray(controls) && Object.keys(controls).every(id=>Object.hasOwn(PINS,id)), 'EXPECTATIONS', 'Only prev, next and boot controls may be configured.');
  const result={appMainIncludes:marker,controls:{}};
  for (const id of Object.keys(PINS)) {
    const item=controls[id] ?? {}, region=item.region ?? [0,0,320,320];
    requireValue(item && typeof item==='object' && !Array.isArray(item) && Object.keys(item).every(k=>['serialIncludes','region','holdGuestSeconds','settleGuestSeconds'].includes(k)), 'EXPECTATIONS', `Invalid ${id} expectation fields.`);
    requireValue(item.serialIncludes==null || typeof item.serialIncludes==='string' && item.serialIncludes.length>=4 && item.serialIncludes.length<=256, 'EXPECTATIONS', `${id}.serialIncludes must be null or a specific nonempty marker.`);
    requireValue(Array.isArray(region) && region.length===4 && region.every(Number.isInteger) && region[0]>=0 && region[1]>=0 && region[2]>0 && region[3]>0 && region[0]+region[2]<=320 && region[1]+region[3]<=320, 'EXPECTATIONS', `Invalid ${id} framebuffer region.`);
    const hold=item.holdGuestSeconds ?? 0.15, settle=item.settleGuestSeconds ?? 0.6;
    requireValue(Number.isFinite(hold) && hold>=0.05 && hold<=0.4 && Number.isFinite(settle) && settle>=0.1 && settle<=5, 'EXPECTATIONS', `Invalid ${id} guest-time intervals.`);
    result.controls[id]={serialIncludes:item.serialIncludes ?? null,region,holdGuestSeconds:hold,settleGuestSeconds:settle};
  }
  return result;
}

function pixelChanges(a,b,[x,y,w,h]=[0,0,320,320]) {
  if (!a || !b || a.length!==b.length) return null;
  let changed=0;
  for(let row=y;row<y+h;row++)for(let col=x;col<x+w;col++) {
    const at=(row*320+col)*4;
    if(a[at]!==b[at] || a[at+1]!==b[at+1] || a[at+2]!==b[at+2] || a[at+3]!==b[at+3])changed++;
  }
  return changed;
}
function diskChanges(before,after) {
  requireValue(before.length===after.length, 'SD_SIZE_CHANGED', 'Exported card size differs from the input image.');
  let changedSectors=0,changedBytes=0;const firstSectorHashes=[];
  for(let offset=0;offset<before.length;offset+=512) {
    let changed=0;for(let i=offset;i<offset+512;i++)if(before[i]!==after[i])changed++;
    if(changed){changedSectors++;changedBytes+=changed;if(firstSectorHashes.length<16)firstSectorHashes.push({sector:offset/512,beforeSha256:hash(before.subarray(offset,offset+512)),afterSha256:hash(after.subarray(offset,offset+512))});}
  }
  return {changedSectors,changedBytes,firstSectorHashes,sectorHashListTruncated:changedSectors>firstSectorHashes.length};
}

// Differences between published snapshots only, never rates or guest-completed reads.
// Pure/exported so precision and epoch boundaries can be checked without a worker.
export function c5CounterDelta(before,after) {
  if(before?.status!=='observed' || after?.status!=='observed')return {status:'unavailable',reason:'two-observed-snapshots-required'};
  const a=before.epoch,b=after.epoch,valid=e=>Number.isSafeInteger(e?.workerSequence) && e.workerSequence>0 && Number.isSafeInteger(e.cardAttachSequence) && e.cardAttachSequence>0;
  if(!valid(a) || !valid(b))return {status:'unavailable',reason:'worker-or-card-epoch-unknown'};
  if(a.workerSequence!==b.workerSequence)return {status:'not-comparable',reason:'worker-changed'};
  if(a.cardAttachSequence!==b.cardAttachSequence)return {status:'not-comparable',reason:'card-changed'};
  if(!Number.isSafeInteger(before.publicationSequence) || before.publicationSequence<1 || !Number.isSafeInteger(after.publicationSequence) || after.publicationSequence<1)return {status:'unavailable',reason:'publication-epoch-binding-unavailable'};
  if(after.publicationSequence<before.publicationSequence)return {status:'not-comparable',reason:'publication-order-reversed'};
  const snapshots=[before.snapshot,after.snapshot];
  if(snapshots.some(s=>s?.schemaVersion!==1 || s.board!==BOARD || s.counterEncoding!=='u64-decimal' || s.state?.sd?.present!==true || !s.state.sd.card))return {status:'unavailable',reason:'exact-C5-card-snapshots-required'};
  const cards=snapshots.map(s=>s.state.sd.card);
  if(cards.some(c=>SD_COUNTERS.some(key=>!u64(c[key]))))return {status:'invalid',reason:'noncanonical-u64-counter'};
  if(before.publicationSequence===after.publicationSequence && SD_COUNTERS.some(key=>cards[0][key]!==cards[1][key]))return {status:'invalid',reason:'same-publication-counter-content-changed'};
  const decreased=SD_COUNTERS.filter(key=>BigInt(cards[1][key])<BigInt(cards[0][key]));
  if(decreased.length)return {status:'not-comparable',reason:'counter-decreased-within-epoch',fields:decreased};
  return {status:'observed',counterEncoding:'u64-decimal',meaning:'difference-of-published-snapshots',
    fromObservation:before.sequence ?? null,fromPublication:before.publicationSequence ?? null,toPublication:after.publicationSequence ?? null,
    newPublication:before.publicationSequence!=null && after.publicationSequence!=null && before.publicationSequence!==after.publicationSequence,
    ...Object.fromEntries(SD_COUNTERS.map(key=>[key,(BigInt(cards[1][key])-BigInt(cards[0][key])).toString()]))};
}

export async function main(options) {
  const receipt={schemaVersion:1,productId:PRODUCT,board:BOARD,checkedAtUTC:new Date().toISOString(),mode:options.run?'run':'check-inputs',status:'error',acceptancePassed:false,
    execution:'public-runtime-production-worker-wasm-in-node',browserUiVerified:false,physicalHardwareVerified:false,
    checks:{boot:{status:'not-run',appMainObserved:false},framebuffer:{status:'not-run'},tfAcceptance:{status:'unavailable',guestReadProven:false,guestFilesystemMountProven:false,durablePersistenceProven:false}},
    inputs:{},sources:{},frames:[],controls:[],workerSessions:[],hardwareDiagnostics:{counterEncoding:'u64-decimal',observations:[],
      freshness:'Most recent runtime publication; may lag by the existing worker throttle and is not atomic with a frame or guest clock.'},
    sd:{guestWritebackObserved:null,transportReads:{status:'unavailable',reason:'Backing-store load counters do not establish valid complete MISO delivery or guest readback.'},guestFilesystemMount:{status:'unavailable',hostMountedFlagIsInsufficient:true},durablePersistence:{status:'not-tested',reason:'Existing Node memory backend; browser IndexedDB and process restart were not exercised.'}},cleanup:{},limitations:[
      'Worker queue acknowledgements establish delivery only, not firmware consumption.',
      'Public sdCard.mounted means host image attachment, not a guest filesystem mount.',
      'C5 cold counters expose inner command calls and backing-store loads/writes, not complete MISO delivery, guest-consumed reads or FAT mount.',
      'Counter differences require the same worker and acknowledged card-attachment epoch; a zero difference of reused publications is not proof of no intervening activity.',
      'The existing Node storage backend is memory-only: reset/reload checks cannot establish browser IndexedDB or cross-process durability.',
      'Frame changes without a quiet region and a fresh firmware-specific button marker remain observations, not verified button semantics.',
      'No host PC/register override, ROM stub, substitute firmware, forced peripheral ACK, host drawing, SD formatting or card clearing is used.',
    ]};
  let runtime, phase='inputs', loaded=false, input, lastState, currentSession, runtimeDeadline=Infinity,publicationSequence=0;
  const threads=new Set(), allThreads=[], serialStreams=new Map(), started=Date.now();
  const root=options['runtime-root'], local=p=>pathToFileURL(resolve(root,p));
  const redactions=Object.entries(options).filter(([k,v])=>typeof v==='string' && !['wasm-sha256'].includes(k)).map(([k,v])=>[v,`<${k}>`]).sort((a,b)=>b[0].length-a[0].length);
  const clean=value=>{let s=String(value ?? '');for(const [text,replacement] of redactions)if(text.length>3)s=s.split(text).join(replacement);return s.replace(/https?:\/\/[^\s"']+/g,'<url>').replace(/\/(?:Users|home)\/[^/\s]+/g,'/home/alice').slice(0,2048);};
  const errorReceipt=error=>({code:error?.code || 'ERROR',message:clean(error?.message || error)});
  const stream=name=>{if(!serialStreams.has(name))serialStreams.set(name,{units:0,bytes:0,tail:'',digest:createHash('sha256')});return serialStreams.get(name);};
  const fresh=(name,mark)=>{const s=stream(name);return s.tail.slice(Math.max(0,mark-(s.units-s.tail.length)));};
  const serialMark=()=>stream('uart0').units;
  const diagnosticTokens=()=>[...new Set([...stream('emulator').tail.matchAll(/(?:0x[0-9a-f]{8}|\b(?:READ|WRITE|PC|pc)[ :=]*(?:0x)?[0-9a-f]{8}\b|\bBOD0\b)/gi)].map(m=>m[0]))].slice(-24);
  const checkWall=()=>requireValue(Date.now()<runtimeDeadline,'WALL_TIMEOUT','Execution exhausted its total wall-time budget.');
  const observe=async()=>{checkWall();lastState=await runtime.observe();requireValue(lastState.status!=='error','GUEST_BLOCKED',lastState.lastError?.message || 'Runtime entered error.');return lastState;};
  const recordHardware=(stage,state,source='observe')=>{
    const snapshot=state?.hardwareDiagnostics,exact=snapshot?.schemaVersion===1 && snapshot.board===BOARD && snapshot.counterEncoding==='u64-decimal';
    const epoch={workerSequence:currentSession?.summary.sequence ?? null,cardAttachSequence:currentSession?.cardEpoch || null};
    const entry={sequence:receipt.hardwareDiagnostics.observations.length+1,stage,phase,source,epoch,runtimeStatus:state?.status ?? null,
      lastStatGuestTime:Number.isFinite(state?.stats?.time)?state.stats.time:null,frameNumber:state?.frame?.frameNumber ?? null,
      publicationSequence:null,status:exact?'observed':'unavailable',snapshot:exact?structuredClone(snapshot):null};
    if(exact){
      entry.snapshotSha256=hash(JSON.stringify(entry.snapshot));const published=currentSession?.lastHardwarePublication;
      if(published?.sha256===entry.snapshotSha256 && published.cardEpoch===currentSession.cardEpoch)entry.publicationSequence=published.sequence;
      const prior=receipt.hardwareDiagnostics.observations.findLast(item=>item.status==='observed');
      entry.counterDelta=prior?c5CounterDelta(prior,entry):{status:'unavailable',reason:'no-prior-observed-snapshot'};
    }else{entry.reason=snapshot?'unexpected-board-schema-or-counter-encoding':'no-runtime-snapshot-published';entry.counterDelta={status:'unavailable',reason:'no-observed-snapshot'};}
    receipt.hardwareDiagnostics.observations.push(entry);return entry.sequence;
  };
  const queryClock=async()=>{
    checkWall();requireValue(currentSession && !currentSession.exited,'WORKER_EXITED','No current live production worker.');
    // Reuse the existing probes' ignored-JSON queue receipt. This does not write
    // UART, alter registers, advance time, or claim guest input consumption.
    return currentSession.clock();
  };
  const waitGuest=async target=>{while(true){await observe();const t=await queryClock();if(t>=target)return t;await delay(50);}};
  const waitBoot=async(mark,startTime)=>{
    let appMain=false,visibleFrame=false,lastFrameNumber=-1;
    while(true) {
      const state=await observe(),time=await queryClock();
      appMain ||= fresh('uart0',mark).includes(input.expectations.appMainIncludes);
      if(state.frame && state.frame.frameNumber!==lastFrameNumber){
        const f=await runtime.capture({format:'rgba'}),p=f.pixels;lastFrameNumber=f.frameNumber;
        requireValue(f.width===320 && f.height===320 && p.length===320*320*4,'FRAME_SHAPE','The real framebuffer does not match LCD2.73 320x320 RGBA.');
        let nonBlack=false,varied=false;for(let i=0;i<p.length;i+=4){nonBlack ||= !!(p[i] || p[i+1] || p[i+2]);varied ||= p[i]!==p[0] || p[i+1]!==p[1] || p[i+2]!==p[2];}
        visibleFrame=nonBlack && varied;
      }
      if(appMain && visibleFrame)return {appMainObserved:true,appMainMarker:input.expectations.appMainIncludes,guestTime:time,frameNumber:lastFrameNumber,visibleFrameObserved:true};
      requireValue(time-startTime<options['boot-guest-seconds'],'BOOT_OBSERVATION_TIMEOUT','No fresh app_main marker and visible real framebuffer within the boot guest-time budget.');
      await delay(50);
    }
  };
  const capture=async label=>{
    const frame=await runtime.capture({format:'rgba'}),pixels=Buffer.from(frame.pixels);
    requireValue(frame.width===320 && frame.height===320 && pixels.length===320*320*4,'FRAME_SHAPE','The real framebuffer does not match LCD2.73 320x320 RGBA.');
    let nonBlackPixels=0;const colors=new Set();for(let i=0;i<pixels.length;i+=4){if(pixels[i] || pixels[i+1] || pixels[i+2])nonBlackPixels++;colors.add((pixels[i]<<16)|(pixels[i+1]<<8)|pixels[i+2]);}
    const metadata={label,width:frame.width,height:frame.height,frameNumber:frame.frameNumber,observedGuestTime:await queryClock(),nonBlackPixels,uniqueRgbColors:colors.size,rgbaSha256:hash(pixels),file:`${label}.rgba`,
      hardwareDiagnosticsSequence:recordHardware(`capture:${label}`,await runtime.observe())};
    await writeFile(resolve(options.out,metadata.file),pixels,{flag:'wx'});receipt.frames.push(metadata);
    return {metadata,pixels};
  };
  const recordDisk=async label=>{
    await runtime.flushSdCard();const bytes=Buffer.from((await runtime.exportSdCard()).bytes),state=await runtime.observe();
    const result={bytes:bytes.length,sha256:hash(bytes),hostAttachment:state.sdCard,againstInput:diskChanges(input.sd,bytes),hardwareDiagnosticsSequence:recordHardware(`disk:${label}`,state)};
    receipt.sd[label]=result;
    if(['afterGuest','afterStop'].includes(label)){receipt.sd.guestWritebackObserved=result.againstInput.changedSectors>0;receipt.checks.tfAcceptance.guestWritebackObserved=receipt.sd.guestWritebackObserved;}
    if(options['save-sd']){result.file=`sd-${label}.img`;await writeFile(resolve(options.out,result.file),bytes,{flag:'wx'});}
    return bytes;
  };
  await mkdir(dirname(options.out),{recursive:true});
  await mkdir(options.out); // A prior receipt/card is never silently overwritten.
  try {
    input={};
    for(const [kind,maxBytes] of [['wasm',256*MIB],['rom',16*MIB],['factory',32*MIB],['sd',64*MIB]]) {
      input[kind]=await inputBytes(options[kind],maxBytes);
      const actual=hash(input[kind]),expected=kind==='wasm'?options['wasm-sha256']:PINNED[kind];
      receipt.inputs[kind]={bytes:input[kind].length,sha256:actual,expectedSha256:expected,hashMatched:actual===expected,sourceKind:/^https?:/.test(options[kind])?'caller-url':'caller-file'};
      requireValue(actual===expected,'INPUT_HASH_MISMATCH',`${kind} SHA-256 does not match the required input.`);
    }
    requireValue(input.sd.length===64*MIB,'SD_SIZE','The fixed acceptance card must be exactly 64 MiB.');
    input.expectations=expectations(options.expectations?JSON.parse(await readFile(resolve(options.expectations),'utf8')):{});
    if(options.expectations)receipt.expectationsSha256=hash(await readFile(resolve(options.expectations)));
    receipt.expectations=input.expectations;receipt.checks.inputHashes={status:'passed'};
    for(const path of ['web/platform/runtime.js','web/wasm/worker.js','web/platform/hardware-diagnostics.js','web/platform/catalog/products.json','web/platform/catalog/firmwares.json','web/platform/app-image.js','web/platform/board-compatibility.js','web/platform/sd-card-storage.js','web/platform/device-storage.js'])receipt.sources[path]=hash(await readFile(local(path)));
    receipt.sources.harnessSha256=hash(await readFile(mainFile));
    try{receipt.sourceRevision=(await execute('git',['-C',root,'rev-parse','HEAD'])).stdout.trim();receipt.sources.haveUncommittedChanges=!!(await execute('git',['-C',root,'status','--porcelain','--untracked-files=no'])).stdout.trim();}catch{receipt.sourceRevision=null;}
    receipt.sources.wasmBuildSourceAttested=false; // An input digest is not a build provenance claim.
    const catalog={};for(const name of ['products','accessories','firmwares'])catalog[name]=JSON.parse(await readFile(local(`web/platform/catalog/${name}.json`))).items;
    const product=catalog.products.find(p=>p.id===PRODUCT),factory=catalog.firmwares.find(f=>f.id==='waveshare-c5-lcd273-factory');
    requireValue(product?.chip==='esp32c5' && product.runtime?.board===BOARD && product.runtime.flashMb===32 && product.runtime.psramMb===8,'CATALOG_PROFILE','Expected independent LCD2.73 C5 32 MiB Flash / 8 MiB PSRAM profile.');
    for(const [id,pin] of Object.entries(PINS))requireValue(product.controls.some(c=>c.id===id && c.pin===pin && c.activeLow===true),'CATALOG_CONTROL',`Expected ${id} on active-low GPIO${pin}.`);
    requireValue(factory?.role==='factory' && factory.source?.sha256===PINNED.factory && factory.source.bootMode==='rom','CATALOG_FACTORY','Catalog factory does not identify the fixed original ROM-boot image.');
    receipt.checks.catalogProfile={status:'passed',flashMiB:32,psramMiB:8,controls:PINS,factoryStatus:factory.status};
    if(!options.run){receipt.status='inputs-checked';receipt.checks.execution={status:'not-run'};return receipt;}

    phase='runtime-setup';runtimeDeadline=Date.now()+options['wall-timeout-ms'];
    const [{createPlatformRuntime},{createDefaultScene},{createDeviceStorage},{createSdCardStorage}]=await Promise.all(['runtime.js','catalog.js','device-storage.js','sd-card-storage.js'].map(p=>import(local(`web/platform/${p}`).href)));
    const workerFactory=url=>{
      requireValue(url.href===ORIGIN+'/wasm/worker.js','WORKER_SOURCE','Unexpected worker URL.');
      const summary={sequence:receipt.workerSessions.length+1,creates:[],loads:[],starts:[],cardAttachments:[],clockQueries:0,sdDeltaMessages:0,sdDeltaPages:0};receipt.workerSessions.push(summary);
      const source=`import {parentPort} from 'node:worker_threads';globalThis.onmessage=null;globalThis.postMessage=(data,transfer)=>parentPort.postMessage(data,transfer);await import(${JSON.stringify(local('web/wasm/worker.js').href)});parentPort.on('message',data=>globalThis.onmessage({data}));`;
      const thread=new Worker(new URL('data:text/javascript,'+encodeURIComponent(source))),clocks=new Map();threads.add(thread);allThreads.push(thread);
      const attaching=new Map();let termination,clockSequence=0;const session={exited:false,summary,cardEpoch:0,lastHardwarePublication:null,firstHardwareEpochs:new Set(),clock:()=>new Promise((done,reject)=>{
        const inputId=`acceptance-clock-${summary.sequence}-${++clockSequence}`,timer=setTimeout(()=>{clocks.delete(inputId);reject(fault('CLOCK_TIMEOUT','Production worker did not acknowledge its clock receipt.'));},10000);
        clocks.set(inputId,{done,reject,timer});summary.clockQueries++;thread.postMessage({op:'text',data:'{}',inputId});
      })};currentSession=session;
      const adapter={postMessage(message,...transfer){
        if(message.op==='create'){
          requireValue(isStrictC5Execution(message),'DRIVER_ASSISTED_MODE','Original-factory strict acceptance forbids Wi-Fi driver compatibility.');
          requireValue(message.board===BOARD && message.flash_mb===32 && message.psram_mb===8 && message.appDirect===false && message.jit===false && !(message.experiments?.length) && !(message.stubs?.length),'WORKER_PROFILE','Create must retain the production C5 32F8P interpreter cold-ROM profile.');
          summary.creates.push({board:message.board,flashMiB:message.flash_mb,psramMiB:message.psram_mb,appDirect:message.appDirect,jit:message.jit});
        }
        if(message.op==='init')requireValue(hash(new Uint8Array(message.wasm))===receipt.inputs.wasm.sha256,'WASM_CHANGED','Worker init received unexpected WASM bytes.');
        if(message.op==='load'){
          requireValue(message.at===undefined && [0,5].includes(message.kind),'WORKER_LOAD_KIND','Only exact ROM kind 0 and original merged Flash kind 5 may load.');
          const kind=message.kind===0?'rom':'factory',sha256=hash(new Uint8Array(message.data));
          requireValue(sha256===PINNED[kind],'WORKER_LOAD_HASH','Worker load bytes differ from the original input.');summary.loads.push({kind:message.kind,bytes:message.data.byteLength,sha256});
        }
        if(message.op==='start'){requireValue(message.appDirect===false,'WORKER_BOOT_MODE','App-direct must never share this cold-ROM handle.');summary.starts.push({appDirect:false});}
        if(message.op==='sd-attach'){
          const attachment={sequence:summary.cardAttachments.length+1,requestId:message.requestId,bytes:message.data.byteLength,acknowledged:false};summary.cardAttachments.push(attachment);attaching.set(message.requestId,attachment);
        }
        requireValue(message.op!=='c5-driver-compat','DRIVER_ASSISTED_MODE','Driver compatibility is excluded from original-factory strict acceptance.');
        requireValue(message.op!=='stub','WORKER_STUB','ROM/peripheral stubs are forbidden in original-factory acceptance.');
        thread.postMessage(message,...transfer);
      },terminate(){return termination ||= thread.terminate();}};
      thread.on('message',data=>{
        if(data.driverCompatibility!=null){adapter.onerror?.({message:'DRIVER_ASSISTED_MODE: an assisted worker cannot become strict factory evidence.'});return;}
        if(data.created!==undefined)summary.created={ok:data.created,clockHz:data.clockHz,flashStorage:data.flashStorage};
        if(attaching.has(data.sdReply)){const attachment=attaching.get(data.sdReply);attaching.delete(data.sdReply);attachment.acknowledged=data.ok===true;if(attachment.acknowledged){session.cardEpoch=attachment.sequence;session.lastHardwarePublication=null;}}
        if(data.sdDelta){summary.sdDeltaMessages++;summary.sdDeltaPages+=data.sdDelta.pages.length;}
        if(clocks.has(data.inputQueued)){const item=clocks.get(data.inputQueued);clearTimeout(item.timer);clocks.delete(data.inputQueued);if(Number.isFinite(data.time) && data.time>=0)item.done(data.time);else item.reject(fault('CLOCK_RECEIPT','Production worker returned an invalid guest clock.'));}
        else adapter.onmessage?.({data});
      });
      thread.on('error',error=>adapter.onerror?.(error));
      thread.on('exit',code=>{session.exited=true;summary.exitCode=code;threads.delete(thread);for(const item of clocks.values()){clearTimeout(item.timer);item.reject(fault('WORKER_EXITED','Production worker exited before a clock receipt.'));}clocks.clear();});
      return adapter;
    };
    const mapped=new Map([['/wasm/esp32sim.wasm','wasm'],['/wasm/fw/esp32c5_rev100_rom.elf','rom'],['/inputs/factory.bin','factory']]);
    const fetchImpl=async value=>{const url=new URL(value,ORIGIN);requireValue(url.origin===ORIGIN && mapped.has(url.pathname) && !url.search,'ASSET_REQUEST','Runtime requested an undeclared asset or remote fallback.');return new Response(input[mapped.get(url.pathname)]);};
    const storage={indexedDB:null,BroadcastChannel:null,dbName:'c5-lcd273-acceptance-'+randomUUID()};
    runtime=createPlatformRuntime({catalog,fetchImpl,workerFactory,baseUrl:ORIGIN+'/platform/',assetBaseUrl:ORIGIN+'/',requestTimeout:10000,deviceStorage:createDeviceStorage(storage),sdStorage:createSdCardStorage({...storage,dbName:storage.dbName+'-sd'}),onEvent:event=>{
      if(event.type==='serial'){const s=stream(event.source),data=String(event.data);s.units+=data.length;s.bytes+=Buffer.byteLength(data);s.digest.update(data);s.tail=(s.tail+data).slice(-65536);}
      if(event.type==='hardware-diagnostics' && currentSession){
        if(event.hardwareDiagnostics){
          currentSession.lastHardwarePublication={sequence:++publicationSequence,sha256:hash(JSON.stringify(event.hardwareDiagnostics)),cardEpoch:currentSession.cardEpoch};
          if(!currentSession.firstHardwareEpochs.has(currentSession.cardEpoch)){currentSession.firstHardwareEpochs.add(currentSession.cardEpoch);recordHardware('first-publication',{hardwareDiagnostics:event.hardwareDiagnostics},'event');}
        }else currentSession.lastHardwarePublication=null;
      }
    }});
    const scene=createDefaultScene(catalog,PRODUCT),source={url:ORIGIN+'/inputs/factory.bin',sha256:PINNED.factory,bootMode:'rom'};scene.firmware=source;
    await runtime.configure(scene);receipt.capabilities=await runtime.capabilities();
    requireValue(receipt.capabilities.execution?.romBoot===true && receipt.capabilities.sdCard?.available===true,'CAPABILITY','Production runtime does not expose C5 ROM boot / SD transport.');
    // Isolated task-owned memory storage; no browser card, original file or
    // existing user scope is replaced. No empty-card/clear/format call exists.
    await runtime.attachSdCard({bytes:new Uint8Array(input.sd)});
    receipt.sd.prepared={sha256:hash((await runtime.exportSdCard()).bytes),hostPreparedOnly:true,guestFilesystemMounted:null};
    requireValue(receipt.sd.prepared.sha256===PINNED.sd,'SD_IMPORT_CHANGED','Prepared card differs from the original input.');
    phase='load';lastState=await runtime.loadFirmware(source);loaded=true;
    recordHardware('load:ready',lastState);
    requireValue(isStrictC5Execution(lastState)&&isStrictC5Execution(lastState.firmware),'DRIVER_ASSISTED_MODE','Strict factory evidence requires an unassisted runtime and firmware receipt.');
    receipt.firmware={bootMode:lastState.firmware.bootMode,appDirect:lastState.firmware.appDirect,resetMode:lastState.firmware.resetMode,executionMode:lastState.firmware.executionMode,artifacts:lastState.firmware.artifacts.map(({kind,bytes,sha256})=>({kind,bytes,sha256}))};
    requireValue(receipt.firmware.bootMode==='rom' && receipt.firmware.appDirect===false && receipt.firmware.resetMode==='rom-reset' && receipt.firmware.executionMode==='wasm-interpreter','FIRMWARE_MODE','Loaded receipt must be the original cold-ROM interpreter profile.');
    const validateSession=()=>{const s=currentSession.summary;requireValue(s.created?.ok===true && s.created.clockHz===NOMINAL_COUNTER_HZ && s.creates.length===1 && s.loads.length===2 && s.loads[0].kind===0 && s.loads[1].kind===5,'WORKER_ABI','Expected one C5 32F8P handle with a 240 MHz nominal counter loading ROM kind 0 before merged image kind 5.');};
    validateSession();receipt.checks.productionLoad={status:'passed',nominalCounterHz:NOMINAL_COUNTER_HZ,romRevision:100,loadKinds:[0,5],xtal:{requiredHz:EXPECTED_XTAL_HZ,source:'independent-C5-runtime-constraint',liveObserved:false,note:'created.clockHz does not expose the XTAL; no live XTAL validation is claimed.'}};
    phase='boot';const bootMark=serialMark();await runtime.start();recordHardware('boot:started',await runtime.observe());receipt.checks.boot=await waitBoot(bootMark,0);receipt.checks.boot.status='observed';
    receipt.checks.boot.hardwareDiagnosticsSequence=recordHardware('boot:ready',await runtime.observe());
    const bootFrame=await capture('boot');
    requireValue(bootFrame.metadata.nonBlackPixels>0 && bootFrame.metadata.uniqueRgbColors>1,'NO_VISIBLE_FRAME','Actual framebuffer is black or uniform; no visible firmware image established.');
    receipt.checks.framebuffer={status:'observed',nonBlackPixels:bootFrame.metadata.nonBlackPixels,rgbaSha256:bootFrame.metadata.rgbaSha256};
    phase='controls';
    for(const id of Object.keys(PINS)){
      const expected=input.expectations.controls[id],item={id,pin:PINS[id],status:'partial',expected};receipt.controls.push(item);
      const beforeQuiet=await capture(`${id}-quiet-start`);await waitGuest((await queryClock())+0.3);const baseline=await capture(`${id}-before`);
      item.quietChangedPixels=pixelChanges(beforeQuiet.pixels,baseline.pixels,expected.region);const mark=serialMark();let down,up;
      try{down=await runtime.input({type:'button',id,pressed:true});item.down=down;await waitGuest(down.guestTime+expected.holdGuestSeconds);}
      finally{try{up=await runtime.input({type:'button',id,pressed:false});item.up=up;}catch(e){item.releaseError=errorReceipt(e);}}
      requireValue(up,'BUTTON_RELEASE','Button release was not acknowledged.');
      item.actualHoldGuestSeconds=up.guestTime-down.guestTime;await waitGuest(up.guestTime+expected.settleGuestSeconds);
      const after=await capture(`${id}-after`),freshUart=fresh('uart0',mark);
      item.changedPixels=pixelChanges(baseline.pixels,after.pixels,expected.region);item.freshUart={bytes:Buffer.byteLength(freshUart),sha256:hash(freshUart),expectedMarkerObserved:expected.serialIncludes===null?null:freshUart.includes(expected.serialIncludes)};
      item.outputChanged=item.changedPixels>0 || freshUart.length>0;
      item.firmwareResponseVerified=expected.serialIncludes!==null && item.freshUart.expectedMarkerObserved && item.quietChangedPixels===0 && item.changedPixels>0 && item.actualHoldGuestSeconds<=0.5;
      item.status=item.firmwareResponseVerified?'observed':item.outputChanged?'partial':'not-observed';
    }
    phase='sd-writeback';await runtime.pause();recordHardware('pause:afterGuest',await runtime.observe());const after=await recordDisk('afterGuest');
    receipt.sd.guestWritebackObserved=receipt.sd.afterGuest.againstInput.changedSectors>0;
    phase='reset';recordHardware('reset:before',await runtime.observe());const resetMark=serialMark();await runtime.reset();validateSession();recordHardware('reset:started',await runtime.observe());receipt.checks.resetBoot=await waitBoot(resetMark,0);
    receipt.checks.resetBoot.hardwareDiagnosticsSequence=recordHardware('reset:ready',await runtime.observe());await runtime.pause();recordHardware('pause:afterReset',await runtime.observe());const afterReset=await recordDisk('afterReset');
    receipt.sd.reset={exactImageRetained:hash(afterReset)===hash(after),guestRanDuringReset:true,difference:diskChanges(after,afterReset),note:'A difference may be a new guest write; equality alone does not prove a guest read.'};
    phase='reload';recordHardware('reload:before',await runtime.observe());await runtime.loadFirmware(source);validateSession();recordHardware('reload:ready',await runtime.observe());const afterReload=await recordDisk('afterReload');
    receipt.sd.reload={exactImageRetainedBeforeGuestStart:hash(afterReload)===hash(afterReset),newProductionWorker:true,storageBackend:(await runtime.observe()).sdCard.backend};
    receipt.checks.tfAcceptance={status:'partial',guestWritebackObserved:receipt.sd.guestWritebackObserved,guestReadProven:false,guestFilesystemMountProven:false,durablePersistenceProven:false};
    receipt.status='partial';
  } catch(error) {
    receipt.failure={phase,...errorReceipt(error),observedDiagnosticTokens:diagnosticTokens()};
    const appMainObserved=!!input?.expectations && fresh('uart0',0).includes(input.expectations.appMainIncludes);
    receipt.status=phase==='inputs' || ['WORKER_PROFILE','WORKER_LOAD_KIND','WORKER_LOAD_HASH','WASM_CHANGED','WORKER_BOOT_MODE','WORKER_STUB','WORKER_ABI','FIRMWARE_MODE','CATALOG_PROFILE','CATALOG_CONTROL','CATALOG_FACTORY'].includes(error.code)?'error':receipt.checks.boot?.appMainObserved || appMainObserved?'partial':'blocked';
    if(phase==='boot')receipt.checks.boot={status:appMainObserved?'partial':'blocked',appMainObserved,framebufferObserved:!!lastState?.frame};
  } finally {
    if(runtime){
      try{lastState=await runtime.observe();if(lastState.status==='running')await runtime.pause();recordHardware('cleanup:paused-or-stopped',await runtime.observe());}catch(e){receipt.cleanup.pauseError=errorReceipt(e);}
      if(loaded && !receipt.sd.afterGuest)try{await recordDisk('afterStop');}catch(e){receipt.sd.exportError=errorReceipt(e);}
      try{const state=await runtime.observe();receipt.lastObservation={status:state.status,stats:state.stats,frame:state.frame,sdCard:state.sdCard,hardwareDiagnosticsSequence:recordHardware('cleanup:last',state),lastError:state.lastError?errorReceipt(state.lastError):null};}catch(e){receipt.cleanup.observeError=errorReceipt(e);}
      try{await runtime.dispose();receipt.cleanup.runtimeDisposed=true;}catch(e){receipt.cleanup.disposeError=errorReceipt(e);}
    }
    await Promise.allSettled(allThreads.map(t=>t.terminate()));
    receipt.cleanup.activeTaskWorkers=threads.size;receipt.cleanup.workersExited=allThreads.length===receipt.workerSessions.filter(s=>s.exitCode!==undefined).length && threads.size===0;
    if(!receipt.cleanup.workersExited){receipt.status='error';receipt.cleanup.error='Not all task-owned worker exits were observed.';}
    receipt.diagnostics={observedTokens:diagnosticTokens(),uartPanicInRetainedTail:/Guru Meditation Error|assert failed:|abort\(\) was called/i.test(stream('uart0').tail)};
    receipt.serial={};for(const [name,s] of serialStreams){receipt.serial[name]={bytes:s.bytes,sha256:s.digest.digest('hex'),retainedTailCodeUnits:s.tail.length};if(options['save-uart']){const filename=`${name.replace(/[^a-z0-9_-]/gi,'_')}.log`;await writeFile(resolve(options.out,filename),s.tail,{flag:'wx'});receipt.serial[name].privateTailFile=filename;}}
    receipt.wallSeconds=(Date.now()-started)/1000;
    await writeFile(resolve(options.out,'result.json'),JSON.stringify(receipt,null,2)+'\n',{flag:'wx'});
  }
  return receipt;
}

if(process.argv[1] && pathToFileURL(resolve(process.argv[1])).href===import.meta.url){
  try{
    const options=parseOptions(process.argv.slice(2));
    if(options.help)process.stdout.write(HELP);
    else{const receipt=await main(options);process.stdout.write(JSON.stringify({status:receipt.status,acceptancePassed:false,failure:receipt.failure ?? null,workersExited:receipt.cleanup.workersExited})+'\n');process.exitCode=({'inputs-checked':0,error:1,blocked:2,partial:3})[receipt.status] ?? 1;}
  }catch(error){process.stderr.write(JSON.stringify({status:'error',code:error.code || 'ERROR',message:error.code==='ARGUMENT'?error.message:'Could not prepare or retain the acceptance receipt; inspect the caller-selected paths.'})+'\n');process.exitCode=1;}
}
