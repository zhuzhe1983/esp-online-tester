import assert from 'node:assert/strict';
import {test} from 'node:test';
import {readFile} from 'node:fs/promises';
import {chipIdForBoard,validateBoardRevision} from '../web/platform/app-image.js';
import {createPlatformRuntime,validateMergedImage} from '../web/platform/runtime.js';
import {assembleLocalFirmware} from '../web/platform/local-firmware.js';
import {createDefaultScene,validateScene,encodeScene,decodeScene} from '../web/platform/catalog.js';
import {firmwareBoardMatches} from '../web/platform/board-compatibility.js';
import {setLocale,t} from '../web/platform/i18n.js';
const catalog={};for(const name of ['products','accessories','firmwares'])catalog[name]=JSON.parse(await readFile(new URL(`../web/platform/catalog/${name}.json`,import.meta.url),'utf8')).items;
const board='metalio-ink4-plus',product=catalog.products.find(p=>p.id===board),buttons=['power','volume-up','volume-down','home','previous','next'];
// Layout and transport stand-ins only. These tests do not execute firmware.
const image=()=>{const b=new Uint8Array(40),v=new DataView(b.buffer);b[0]=0xe9;b[1]=1;v.setUint16(12,32,true);v.setUint32(24,0x2f000000,true);v.setUint32(28,4,true);return b;};
const table=()=>{const b=new Uint8Array(4096).fill(255),v=new DataView(b.buffer);v.setUint16(0,0x50aa,true);b[2]=0;b[3]=16;v.setUint32(4,0x210000,true);v.setUint32(8,0xcf0000,true);return b;};
const merged=()=>{const b=new Uint8Array(0x210028).fill(255);b.set(image(),0x2000);b.set(table(),0x8000);b.set(image(),0x210000);return b;};
test('INK4 Plus preserves its S31 image identity and actual offsets',async()=>{
  assert.equal(chipIdForBoard(board),32);assert.equal(validateMergedImage(merged(),board,16<<20),true);
  for(const other of ['metalio-ink4','mosaico','esp-mosaico'])assert.equal(firmwareBoardMatches(other,board),false);
  assert.throws(()=>validateMergedImage(merged(),'metalio-ink4',16<<20),{code:'INCOMPLETE_IMAGE'});
  const wrongChip=merged();new DataView(wrongChip.buffer).setUint16(0x2000+12,9,true);assert.throws(()=>validateMergedImage(wrongChip,board,16<<20),{code:'CHIP_MISMATCH'});
  const later=image();new DataView(later.buffer).setUint16(15,100,true);assert.throws(()=>validateBoardRevision(later,board),{code:'CHIP_REVISION_UNSUPPORTED'});
  const source={format:'segments',files:[{offset:0x2000,file:new File([image()],'bootloader.bin')},{offset:0x8000,file:new File([table()],'partition-table.bin')},{offset:0x210000,file:new File([image()],'app.bin')}]};
  const result=await assembleLocalFirmware(source,{board,flashBytes:16<<20});assert.equal(validateMergedImage(result.data,board,16<<20),true);
  await assert.rejects(()=>assembleLocalFirmware({...source,files:[{...source.files[0],offset:0},...source.files.slice(1)]},{board,flashBytes:16<<20}),{code:'BOOTLOADER_REQUIRED'});
});
test('simulation memory and trial firmware remain distinct from unverified physical capacity',async()=>{
  const knowledge=JSON.parse(await readFile(new URL('../web/platform/knowledge/metalio-ink4-plus.json',import.meta.url),'utf8'));
  assert.equal(knowledge.facts.find(f=>f.id==='psram').value.capacityMB,null);
  assert.equal(product.runtime.psramMb,16);assert.equal(product.runtime.psramMbBasis,'simulation');assert.equal(product.runtime.status,'partial');
  const firmware=catalog.firmwares.find(f=>f.id===product.defaultFirmwareId);assert.equal(firmware.status,'trial');assert.equal(firmware.simulationVerified,false);
  const scene=createDefaultScene(catalog,board);assert.equal(validateScene(scene,catalog).valid,true);assert.deepEqual(decodeScene(encodeScene(scene)),scene);
  setLocale('en');try{
    assert.equal(t('16 MB Flash / 16 MB PSRAM（模拟配置）'),'16 MB Flash / 16 MB PSRAM (simulated configuration)');
    for(const text of [...product.limitations,...firmware.limitations])assert.equal(/[\u4e00-\u9fff]/.test(t(text)),false);
  }finally{setLocale('zh-CN');}
});
test('S31 loader, semantic inputs, SD and public capabilities use one product route',async()=>{
  const messages=[],urls=[];let sdBytes=0;
  const transport={postMessage(m){messages.push(m);let reply;
    if(m.op==='init')reply={ready:true};else if(m.op==='create')reply={created:true,flashStorage:true,networkAvailable:true};
    else if(m.op==='load')reply={loaded:m.at===undefined?m.kind:`at${m.at}`,ok:true};
    else if(m.op.startsWith('flash-'))reply={flashReply:m.requestId,ok:true,...(m.op==='flash-snapshot'?{data:new ArrayBuffer(16<<20)}:{})};
    else if(m.op.startsWith('sd-')){if(m.op==='sd-attach')sdBytes=m.data.byteLength;reply={sdReply:m.requestId,ok:true};}
    else if(m.op.startsWith('network-'))reply={networkReply:m.requestId,ok:true,softApSupported:false};
    else if(m.op==='start')reply={started:true};else if(m.op==='stop')reply={stopped:true};else if(m.op==='text')reply={inputQueued:m.inputId,time:0.25};
    if(reply)queueMicrotask(()=>this.onmessage?.({data:reply}));},terminate(){}};
  const runtime=createPlatformRuntime({catalog,baseUrl:'https://example.test/platform/index.html',assetBaseUrl:'https://example.test/',fetchImpl:async url=>{urls.push(String(url));return new Response(new Uint8Array([1]));},workerFactory:()=>transport});
  try {
    const configured=await runtime.configure(createDefaultScene(catalog,board));assert.equal(configured.network.browserSoftAp,false);
    const caps=await runtime.capabilities();assert.deepEqual(caps.input.buttons,buttons);assert.equal(caps.input.touch,true);assert.deepEqual(caps.input.serialPorts,['usb','uart0']);assert.equal(caps.sdCard.available,true);assert.equal(caps.network.softAp,false);assert.equal(caps.network.transport,'ethernet-websocket');assert.deepEqual(caps.network.automaticProvisioning,[]);
    for(const c of product.controls.filter(c=>c.type==='button')){assert.equal(c.transport,'board');assert.equal(c.pin,undefined);assert.equal(c.gpio,undefined);}
    await runtime.loadFirmware({format:'merged',file:new File([merged()],'layout-only.bin')});
    assert.ok(urls.some(u=>u.endsWith('/esp32s31_rev0_rom.elf')));assert.ok(!urls.some(u=>u.endsWith('/esp32s3_rev0_rom.elf')));
    const create=messages.find(m=>m.op==='create');assert.equal(create.board,board);assert.equal(create.jit,false);assert.equal(create.flash_mb,16);assert.equal(create.psram_mb,16);assert.equal(sdBytes,64<<20);
    assert.equal(messages.find(m=>m.op==='network-setup').softAp.enabled,false);
    await runtime.start();
    for(const id of buttons)for(const pressed of [true,false]){const receipt=await runtime.input({type:'button',id,pressed});assert.deepEqual(receipt.message,{t:'board-button',id,pressed:pressed?1:0});}
    const touch=await runtime.input({type:'touch',x:683,y:1215,down:true});assert.deepEqual(touch.message,{t:'touch',x:683,y:1215,down:1});
    for(const [x,y] of [[684,0],[0,1216],[100,1600]])await assert.rejects(runtime.input({type:'touch',x,y,down:true}),{code:'INVALID_INPUT'});
    await assert.rejects(runtime.input({type:'serial',port:'uart1',data:'unsupported'}),{code:'UNSUPPORTED_PORT'});
    for(const port of ['usb','uart0'])assert.equal((await runtime.input({type:'serial',port,data:'x'})).accepted,true);
    await runtime.pause();assert.equal((await runtime.input({type:'button',id:'power',pressed:false})).accepted,true);await assert.rejects(runtime.input({type:'button',id:'power',pressed:true}),{code:'NOT_RUNNING'});
  }finally{await runtime.dispose();}
});
