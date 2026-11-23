import assert from 'node:assert/strict';
import {test} from 'node:test';
import {readFile} from 'node:fs/promises';
import {chipIdForBoard} from '../web/platform/app-image.js';
import {createPlatformRuntime,validateMergedImage,sha256} from '../web/platform/runtime.js';
import {assembleLocalFirmware} from '../web/platform/local-firmware.js';
import {createDefaultScene,validateScene} from '../web/platform/catalog.js';
import {firstFirmware} from '../web/platform/catalog-selection.js';
import {imuModelForBoard} from '../web/platform/imu.js';
const catalog={};for(const n of ['products','accessories','firmwares'])catalog[n]=JSON.parse(await readFile(new URL(`../web/platform/catalog/${n}.json`,import.meta.url),'utf8')).items;
const product=catalog.products.find(p=>p.id==='m5stack-k016-c'),board='stickc';
// Synthetic layout is only a loader/protocol fixture, NOT firmware execution evidence.
const image=()=>{const b=new Uint8Array(40),v=new DataView(b.buffer);b[0]=0xe9;b[1]=1;v.setUint32(24,0x3ffb0000,true);v.setUint32(28,4,true);return b;};
const merged=()=>{const b=new Uint8Array(0x10028).fill(255),v=new DataView(b.buffer);b.set(image(),0x1000);b.set(image(),0x10000);v.setUint16(0x8000,0x50aa,true);b[0x8002]=0;b[0x8003]=0;v.setUint32(0x8004,0x10000,true);v.setUint32(0x8008,0x200000,true);return b;};
test('K016-C identity, factory-first and MPU6886 batch are explicit',()=>{
 assert.equal(product.runtime.board,board);assert.equal(product.runtime.flashMb,4);assert.equal(product.runtime.psramMb,0);assert.equal(product.runtime.status,'partial');assert.equal(product.display.width,80);assert.equal(product.display.height,160);
 assert.equal(imuModelForBoard(board),'mpu6886-register-subset');assert.equal(chipIdForBoard(board),0);assert.equal(validateScene(createDefaultScene(catalog,product.id),catalog).valid,true);
 const fw=firstFirmware(catalog.firmwares,product);assert.equal(fw.id,'m5stack-k016-c-official-factory');assert.equal(fw.role,'factory');assert.equal(fw.status,'trial');assert.equal(fw.simulationVerified,false);
 assert.deepEqual(product.controls.filter(c=>c.pin!==undefined).map(c=>c.pin),[37,39]);const power=product.controls.find(c=>c.id==='power');assert.equal(power.pin,undefined);assert.equal(power.transport,'board');
});
test('classic ROM/boot offsets are enforced for uploaded and remote StickC firmware',async()=>{
 assert.equal(validateMergedImage(merged(),board,4<<20),true);const wrong=merged();new DataView(wrong.buffer).setUint16(0x1000+12,9,true);assert.throws(()=>validateMergedImage(wrong,board,4<<20),{code:'CHIP_MISMATCH'});
 const local=await assembleLocalFirmware({format:'segments',files:[{offset:0x1000,file:new File([image()],'boot.bin')},{offset:0x8000,file:new File([merged().slice(0x8000,0x9000)],'partitions.bin')},{offset:0x10000,file:new File([image()],'app.bin')}]},{board,flashBytes:4<<20});assert.equal(validateMergedImage(local.data,board,4<<20),true);
 if(process.env.STICKC_FACTORY_BIN){const real=await readFile(process.env.STICKC_FACTORY_BIN);assert.equal(await sha256(real),'8bd01902abdc0f0f6bfbf36a563de027d01cc9640e59c30756cc7315b06ccb03');assert.equal(validateMergedImage(real,board,4<<20),true);}
});
test('API uses physical GPIO buttons, PMIC semantic PEK, classic ROM and 4MB memory',async()=>{
 const messages=[],urls=[],worker={postMessage(m){messages.push(m);let r;if(m.op==='init')r={ready:true};else if(m.op==='create')r={created:true,flashStorage:false};else if(m.op==='load')r={loaded:m.at===undefined?m.kind:`at${m.at}`,ok:true};else if(m.op==='start')r={started:true};else if(m.op==='stop')r={stopped:true};else if(m.op==='imu')r={imuApplied:m.inputId,ok:true,time:.25};else if(m.op==='text')r={inputQueued:m.inputId,time:.25};else if(m.op.startsWith('network-'))r={networkReply:m.requestId,ok:true,softApSupported:false};if(r)queueMicrotask(()=>this.onmessage?.({data:r}));},terminate(){}};
 const runtime=createPlatformRuntime({catalog,baseUrl:'https://test.invalid/platform/',assetBaseUrl:'https://test.invalid/',workerFactory:()=>worker,fetchImpl:async u=>{urls.push(String(u));return new Response(new Uint8Array([1,2,3]));}});
 try {await runtime.configure(createDefaultScene(catalog,product.id));await runtime.loadFirmware({format:'merged',file:new File([merged()],'layout.bin')});assert.ok(urls.some(u=>u.endsWith('/esp32_rev300_rom.elf')));assert.ok(!urls.some(u=>u.endsWith('/esp32s3_rev0_rom.elf')));const c=messages.find(m=>m.op==='create');assert.equal(c.board,board);assert.equal(c.flash_mb,4);assert.equal(c.psram_mb,0);await runtime.start();
 for(const [id,pin] of [['button-a',37],['button-b',39]])for(const pressed of [true,false])assert.deepEqual((await runtime.input({type:'button',id,pressed})).message,{t:'btn',pin,v:+pressed});
 for(const pressed of [true,false])assert.deepEqual((await runtime.input({type:'button',id:'power',pressed})).message,{t:'board-button',id:'power',pressed:+pressed});await assert.rejects(runtime.input({type:'touch',x:1,y:1,down:true}),{code:'UNSUPPORTED_CONTROL'});await assert.rejects(runtime.input({type:'button',id:'unwired',pressed:true}),{code:'UNSUPPORTED_CONTROL'});
 }finally{await runtime.dispose();}
});
