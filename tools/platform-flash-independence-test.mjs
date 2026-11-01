import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createDefaultScene} from '../web/platform/catalog.js';
import {createPlatformRuntime,sha256} from '../web/platform/runtime.js';
import {createFlasher,prepareFlashPlan} from '../web/platform/flasher.js';
import {firmwareFlashPolicy,firmwareRunPolicy} from '../web/platform/firmware-policy.js';
import {firmwareCache} from '../web/platform/firmware-cache.js';

const baseUrl='https://platform.example/platform/',publicCatalog={};
for(const name of ['products','accessories','firmwares'])publicCatalog[name]=JSON.parse(await readFile(new URL(`../web/platform/catalog/${name}.json`,import.meta.url))).items;
const publicWatchlist=JSON.parse(await readFile(new URL('../web/platform/catalog/firmware-watchlist.json',import.meta.url)));
// Structural protocol fixture, not a runnable or published manufacturer image.
function mergedImage(chip=13) {
  const bytes=new Uint8Array(0x10040).fill(255),view=new DataView(bytes.buffer);
  for(const at of [0,0x10000]) {
    bytes.fill(0,at,at+64);bytes[at]=0xe9;bytes[at+1]=1;view.setUint16(at+12,chip,true);view.setUint32(at+28,4,true);
  }
  view.setUint16(0x8000,0x50aa,true);bytes[0x8002]=0;bytes[0x8003]=0;
  view.setUint32(0x8004,0x10000,true);view.setUint32(0x8008,0x100000,true);
  return bytes;
}
async function fixture(id='m5stack-u202-meshtastic-stable') {
  const catalog=structuredClone(publicCatalog),watchlist=structuredClone(publicWatchlist),bytes=mergedImage(),hash=await sha256(bytes);
  const firmware=catalog.firmwares.find(item=>item.id===id),product=catalog.products.find(item=>item.id==='m5stack-u202');
  // Intercept the exact catalog URLs with test bytes; never fetch real firmware.
  if(firmware.source.sha256)firmware.source.sha256=hash;
  for(const entry of watchlist.entries)for(const asset of entry.assets)if(asset.url===firmware.source.url&&asset.sha256)asset.sha256=hash;
  const scene=createDefaultScene(catalog,product.id);scene.firmware=structuredClone(firmware.source);
  const requests=[];
  const fetchImpl=async(url,options)=>{requests.push(url);assert.equal(url,firmware.source.url);assert.equal(options.cache,'no-store');return new Response(bytes);};
  return {catalog,watchlist,firmware,product,scene,bytes,hash,requests,fetchImpl,baseUrl};
}

test('C6L trial sources download and prepare without a board or a prior simulator download',async()=>{
  for(const id of ['m5stack-u202-meshtastic-stable','m5stack-u202-meshtastic-alpha','m5stack-u202-uiflow2']) {
    const f=await fixture(id);await firmwareCache.clear();
    assert.equal(f.product.runtime.board,null);assert.equal(f.firmware.simulationVerified,false);
    assert.equal(firmwareRunPolicy(f.product,f.firmware).canRun,false);
    assert.equal(firmwareFlashPolicy(f.product,f.scene.firmware).canFlash,true);
    let simulatorDownloads=0,workers=0;
    const runtime=createPlatformRuntime({catalog:f.catalog,baseUrl,fetchImpl:async()=>{simulatorDownloads++;throw Error('Unexpected fetch');},workerFactory:()=>{workers++;throw Error('Unexpected worker');}});
    try {
      await runtime.configure(f.scene);await assert.rejects(()=>runtime.loadFirmware(),{code:'UNSUPPORTED_PRODUCT'});
      const watchEntry=f.watchlist.entries.find(entry=>entry.assets.some(asset=>asset.url===f.firmware.source.url));
      assert.equal(watchEntry.compatibility.status,'reference-only');
      const plan=await prepareFlashPlan({...f,watchEntry});
      assert.deepEqual(plan.files.map(item=>item.address),[0]);assert.deepEqual(plan.files[0].data,f.bytes);
      assert.equal(plan.receipt.artifacts[0].sha256,f.hash);assert.equal(plan.receipt.chip,'esp32c6');
      assert.deepEqual(f.requests,[f.firmware.source.url]);assert.equal(simulatorDownloads,0);assert.equal(workers,0);
    } finally {await runtime.dispose();await firmwareCache.clear();}
  }
});

test('a missing simulator ROM does not prevent a separate hardware write',async()=>{
  const f=await fixture();await firmwareCache.clear();f.product.runtime={...f.product.runtime,board:'nanoc6',status:'partial'};
  const runtime=createPlatformRuntime({catalog:f.catalog,baseUrl,assetBaseUrl:'https://platform.example/',fetchImpl:async(url,options)=>url===f.firmware.source.url?f.fetchImpl(url,options):new Response('ROM unavailable',{status:404}),workerFactory:()=>{throw Error('No worker should start');}});
  try {
    await runtime.configure(f.scene);await assert.rejects(()=>runtime.loadFirmware(),{code:'HTTP_ERROR'});
    assert.equal((await runtime.observe()).status,'error');
    // Force a fresh hardware download to prove the paths do not share a prerequisite.
    await firmwareCache.clear();f.requests.length=0;
    const calls={chooser:0,writes:0,disconnect:0,reset:0};
    class Transport {async disconnect(){calls.disconnect++;}}
    class ESPLoader {
      chip={CHIP_NAME:'ESP32-C6'};
      async main(){} async getSecurityInfo(){return {flashCryptCnt:0,parsedFlags:{}};}
      async detectFlashSize(){return '16MB';} flashSizeBytes(){return 16*1024*1024;}
      async writeFlash(options){calls.writes++;assert.deepEqual(options.fileArray[0].data,f.bytes);assert.equal(options.eraseAll,false);options.reportProgress(0,f.bytes.length,f.bytes.length);}
      async after(){calls.reset++;}
    }
    const flasher=createFlasher({navigator:{userActivation:{isActive:true},serial:{requestPort(){calls.chooser++;return Promise.resolve({});}}},secureContext:true,fetchImpl:f.fetchImpl,baseUrl,loadWatchlistData:async()=>f.watchlist,loadTools:async()=>({Transport,ESPLoader,md5:()=>''})});
    const operation=flasher.flash(f);assert.equal(calls.chooser,1,'Keep the port chooser in the user click stack');
    await operation;assert.equal(flasher.observe().status,'done');assert.deepEqual(calls,{chooser:1,writes:1,disconnect:1,reset:1});
    assert.deepEqual(f.requests,[f.firmware.source.url]);assert.equal((await runtime.observe()).status,'error');
  } finally {await runtime.dispose();await firmwareCache.clear();}
});

test('missing sources, app-only mode and archives cannot become physical flash plans',async()=>{
  const f=await fixture();
  const reference=f.catalog.firmwares.find(item=>item.id===f.product.defaultFirmwareId);
  assert.equal(firmwareFlashPolicy(f.product,reference.source).reason,'FIRMWARE_REQUIRED');
  for(const source of [{...f.scene.firmware,bootMode:'app-direct'},{...f.scene.firmware,archive:{format:'zip',entry:'app.bin'}}]) {
    assert.equal(firmwareFlashPolicy(f.product,source).reason,'FLASH_LAYOUT');
    await assert.rejects(()=>prepareFlashPlan({...f,source}),{code:'FLASH_LAYOUT'});
  }
  await assert.rejects(()=>prepareFlashPlan({...f,source:null}),{code:'FIRMWARE_REQUIRED'});
  assert.equal(f.requests.length,0);
});

test('exact product mapping is retained for raw URLs, aliases and nested manifest files',async()=>{
  const f=await fixture();await firmwareCache.clear();
  f.scene={...createDefaultScene(f.catalog,'m5stack-dk001'),firmware:f.scene.firmware}; // Same chip, wrong product.
  await assert.rejects(()=>prepareFlashPlan(f),{code:'BOARD_MISMATCH'});
  const watched=f.watchlist.entries.find(entry=>entry.id==='meshtastic'),asset=watched.assets.find(item=>item.url===f.firmware.source.url);
  asset.upstreamUrl='https://publisher.example/c6l.bin';
  await assert.rejects(()=>prepareFlashPlan({...f,source:{url:asset.upstreamUrl}}),{code:'BOARD_MISMATCH'});
  const manifestUrl='https://publisher.example/wrong.json';
  const manifest={productId:f.scene.productId,flash_mb:16,files:{flash:f.firmware.source}};
  await assert.rejects(()=>prepareFlashPlan({...f,source:{manifestUrl},fetchImpl:async url=>{assert.equal(url,manifestUrl);return Response.json(manifest);}}),{code:'BOARD_MISMATCH'});
  assert.equal(f.requests.length,0);
});

test('trial status does not bypass pinned hashes, chip identity, capacity or complete image checks',async()=>{
  const f=await fixture();await firmwareCache.clear();
  for(const [bytes,code] of [[mergedImage(9),'CHIP_MISMATCH'],[new Uint8Array([0xe9]),'INCOMPLETE_IMAGE'],[new Uint8Array(16*1024*1024+1),'FLASH_OVERFLOW']]) {
    const changed=structuredClone(f.catalog),fw=changed.firmwares.find(item=>item.id===f.firmware.id);
    delete fw.source.sha256;const scene={...f.scene,firmware:fw.source};
    await assert.rejects(()=>prepareFlashPlan({scene,catalog:changed,baseUrl,fetchImpl:async()=>new Response(bytes)}),{code});
  }
  const altered=mergedImage();altered[32]^=1;const alteredHash=await sha256(altered);
  await assert.rejects(()=>prepareFlashPlan({...f,source:{...f.scene.firmware,sha256:alteredHash},fetchImpl:async()=>new Response(altered)}),{code:'HASH_MISMATCH'});
  f.firmware.status='reference';
  await assert.rejects(()=>prepareFlashPlan(f),{code:'REFERENCE_ONLY'});
  await firmwareCache.clear();
});

test('merged BINs and build manifests write the image once at zero',async()=>{
  const f=await fixture(),image=new File([f.bytes],'merged.bin');
  const config=new File([JSON.stringify({flash_files:{'0x0':'merged.bin'},extra_esptool_args:{chip:'esp32c6'},flash_settings:{flash_size:'16MB'}})],'flasher_args.json');
  for(const source of [{file:image},{format:'build',files:[config,image]},{format:'segments',files:[{file:image,offset:0}]}]) {
    const plan=await prepareFlashPlan({...f,source});
    assert.equal(plan.receipt.format,'merged');assert.equal(plan.files.length,1);
    assert.equal(plan.files[0].address,0);assert.deepEqual(plan.files[0].data,f.bytes);
  }
  assert.equal(f.requests.length,0);
});

test('partition builds preserve non-default table/app offsets and never write filled gaps',async()=>{
  const f=await fixture(),boot=new File([f.bytes.slice(0,64)],'bootloader.bin'),app=new File([f.bytes.slice(0x10000)],'app.bin');
  const tableBytes=f.bytes.slice(0x8000,0x9000);new DataView(tableBytes.buffer).setUint32(4,0x20000,true);
  const table=new File([tableBytes],'partitions.bin');
  const config=new File([JSON.stringify({flash_files:{'0x0':'bootloader.bin','0x9000':'partitions.bin','0x20000':'app.bin'},extra_esptool_args:{chip:'esp32c6'}})],'flasher_args.json');
  for(const source of [{format:'build',files:[config,boot,table,app]},{format:'segments',files:[{file:boot,offset:0},{file:table,offset:0x9000},{file:app,offset:0x20000}]}]) {
    const plan=await prepareFlashPlan({...f,source});
    assert.equal(plan.receipt.format,'segments');assert.equal(plan.receipt.partitionTableOffset,0x9000);
    assert.deepEqual(plan.files.map(item=>item.address),[0,0x9000,0x20000]);
    assert.equal(plan.files.reduce((sum,item)=>sum+item.data.length,0),boot.size+table.size+app.size);
  }
  await assert.rejects(()=>prepareFlashPlan({...f,source:{format:'build',files:[config,app]}}),{code:'LOCAL_FILES_MISSING'});
  assert.equal(f.requests.length,0);
});

test('a standalone app BIN is recognized but cannot invent a hardware partition address',async()=>{
  const scene=createDefaultScene(publicCatalog,'m5stack-stopwatch');
  const app=await readFile(new URL('../web/wasm/fw/public/hello_world.bin',import.meta.url));
  await assert.rejects(()=>prepareFlashPlan({scene,catalog:publicCatalog,source:{file:new File([app],'app.bin')},baseUrl}),error=>error.code==='FLASH_LAYOUT'&&error.message.includes('flasher_args.json'));
});
