import assert from 'node:assert/strict';
import {test} from 'node:test';
import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {chipIdForBoard} from '../web/platform/app-image.js';
import {createPlatformRuntime,validateMergedImage,fetchBytes} from '../web/platform/runtime.js';
import {createDefaultScene,validateScene,encodeScene,decodeScene} from '../web/platform/catalog.js';
import {imuModelForBoard} from '../web/platform/imu.js';
import * as THREE from '../web/platform/vendor/three.module.min.js';
import {createPlateGeometry} from '../web/platform/plate-geometry.js';
const catalog={};for(const name of ['products','accessories','firmwares'])catalog[name]=JSON.parse(await readFile(new URL(`../web/platform/catalog/${name}.json`,import.meta.url),'utf8')).items;
const product=catalog.products.find(p=>p.id==='m5stack-k150');
// Synthetic image/layout and worker transport only: no claim of firmware execution.
const merged=()=>{const b=new Uint8Array(0x10028),v=new DataView(b.buffer);for(const at of [0,0x10000]){b[at]=0xe9;b[at+1]=1;b[at+12]=9;v.setUint32(at+24,0x40370000,true);v.setUint32(at+28,4,true);}b[0x8000]=0xaa;b[0x8001]=0x50;v.setUint32(0x8004,0x10000,true);v.setUint32(0x8008,0x100000,true);return b;};
test('K150 keeps its own S3-PICO, memory, physical input and unverified firmware identity',()=>{
  assert.equal(product.module,'ESP32-S3-PICO-1-N8R8');assert.deepEqual(product.runtime,{board:'sticks3',flashMb:8,psramMb:8,status:'partial'});
  assert.equal(chipIdForBoard('sticks3'),9);assert.equal(validateMergedImage(merged(),'sticks3',8<<20),true);
  const bad=merged();bad[12]=0;assert.throws(()=>validateMergedImage(bad,'sticks3',8<<20),{code:'CHIP_MISMATCH'});
  assert.throws(()=>validateMergedImage(merged().subarray(0,0x10020),'sticks3',8<<20),{code:'INCOMPLETE_IMAGE'});
  assert.deepEqual(product.controls.map(c=>[c.id,c.pin,c.activeLow]),[['btnA',11,true],['btnB',12,true]]);
  assert.equal(product.display.touch,false);assert.equal(product.defaultFirmwareId,undefined);assert.equal(imuModelForBoard('sticks3'),'bmi270-register-subset');
  const options=catalog.firmwares.filter(f=>f.productIds.includes(product.id));assert.equal(options.length,2);
  for(const fw of options){assert.equal(fw.status,'trial');assert.equal(fw.role,'alternative');assert.equal(fw.simulationVerified,false);assert.equal(fw.hardwareValidated,false);assert.equal(fw.catalogUpdate.publisherIdentity.deviceId,'1897100000000000020');assert.match(fw.source.url,/^https:\/\/burner\.m5stack\.com\/api\/v1\/firmwares\/\d+\/versions\/\d+\/download$/);}
  const scene=createDefaultScene(catalog,product.id);assert.equal(validateScene(scene,catalog).valid,true);assert.deepEqual(decodeScene(encodeScene(scene)),scene);
});
test('K150 local merged loader and input API select the new board, not a visual alias',async()=>{
  const messages=[],urls=[];
  const transport={postMessage(m){messages.push(m);let reply;
    if(m.op==='imu')reply={imuApplied:m.inputId,ok:true,time:0};else if(m.op==='init')reply={ready:true};else if(m.op==='create')reply={created:true};
    else if(m.op==='load')reply={loaded:m.at===undefined?m.kind:`at${m.at}`,ok:true};
    else if(m.op==='start')reply={started:true};else if(m.op==='stop')reply={stopped:true};else if(m.op==='text')reply={inputQueued:m.inputId,time:0.25};
    if(reply)queueMicrotask(()=>this.onmessage?.({data:reply}));},terminate(){}};
  const runtime=createPlatformRuntime({catalog,baseUrl:'https://fixture.invalid/platform/',assetBaseUrl:'https://fixture.invalid/',fetchImpl:async url=>{urls.push(String(url));return new Response(new Uint8Array([1]));},workerFactory:()=>transport});
  try {
    await runtime.configure(createDefaultScene(catalog,product.id));const c=await runtime.capabilities();assert.equal(c.executable,true);assert.equal(c.imu.available,true);assert.deepEqual(c.input.buttons,['btnA','btnB']);assert.equal(c.input.touch,false);assert.equal(c.sdCard.available,false);assert.equal(c.audio.input.mode,'logical-i2s-port');
    await runtime.loadFirmware({format:'merged',file:new File([merged()],'synthetic-layout-only.bin')});assert.ok(urls.some(u=>u.endsWith('/esp32s3_rev0_rom.elf')));
    const cfg=messages.find(m=>m.op==='create');assert.equal(cfg.board,'sticks3');assert.equal(cfg.flash_mb,8);assert.equal(cfg.psram_mb,8);await runtime.start();
    for(const [id,pin] of [['btnA',11],['btnB',12]])for(const pressed of [true,false]){const r=await runtime.input({type:'button',id,pressed});assert.deepEqual(r.message,{t:'btn',pin,v:pressed?1:0});}
    await assert.rejects(runtime.input({type:'button',id:'power',pressed:true}),{code:'UNSUPPORTED_CONTROL'});await assert.rejects(runtime.input({type:'touch',x:1,y:1,down:true}),{code:'UNSUPPORTED_CONTROL'});
    await runtime.pause();assert.equal((await runtime.input({type:'button',id:'btnB',pressed:false})).accepted,true);await assert.rejects(runtime.input({type:'button',id:'btnA',pressed:true}),{code:'NOT_RUNNING'});
  }finally{await runtime.dispose();}
});
test('K150 input regions preserve manufacturer geometry and UNKNOWN internals',async()=>{
 const model=JSON.parse(await readFile(new URL('../web/platform/assets/m5stack-k150/model.json',import.meta.url),'utf8'));
 for(const name of ['geometry.bin','source.stl'])assert.equal(createHash('sha256').update(await readFile(new URL(`../web/platform/assets/m5stack-k150/${name}`,import.meta.url))).digest('hex'),model.artifacts[name]);assert.equal(model.artifacts['geometry.bin'],'e2de53c2cf1b6d3a2e6dd66bf60ee66c37050c12794bde5bb41dd9c294189e7d');assert.equal(model.parts.find(p=>p.name==='cad-component-3').controlId,'btnA');
 assert.equal(model.presentation.buttons[0].id,'btnB');assert.equal(model.presentation.buttons[0].basis,'image-measurement');assert.equal(model.derivedParts.find(p=>p.id==='pcb-main').label,'UNKNOWN');assert.equal(product.model.status,'partial');
});

test('K150 Hat2 has two rows of eight open female pockets on 2.54 mm pitch',async()=>{
  const model=JSON.parse(await readFile(new URL('../web/platform/assets/m5stack-k150/model.json',import.meta.url),'utf8'));
  const header=model.derivedParts.find(part=>part.id==='hat2-header');
  const geometry=createPlateGeometry(header.geometry);
  const material=new THREE.MeshBasicMaterial({side:THREE.DoubleSide});
  try {
    const surface=new THREE.Mesh(geometry,material);
    const through=(x,y)=>new THREE.Raycaster(new THREE.Vector3(x,y,5),new THREE.Vector3(0,0,-1)).intersectObject(surface);
    assert.equal(header.geometry.holesMm.length,16);
    for(const y of [-1.27,1.27])for(let i=0;i<8;i++)assert.equal(through((i-3.5)*2.54,y).length,0);
    assert.ok(through(0,0).length>0,'Center web remains solid between the rows');
  }finally{geometry.dispose();material.dispose();}
});

test('K150 republished exact UIFlow pin retains real download gate and no boot claims',async()=>{
  const fw=catalog.firmwares.find(f=>f.id==='m5stack-k150-uiflow2');
  assert.equal(fw.catalogUpdate.publisherIdentity.versionId,'2122962646011428866');
  assert.equal(fw.catalogUpdate.assetName,'m5stack__build-M5STACK_StickS3__uiflow-2d611096-esp32s3-spiram-8mb-sticks3-v2.5.5-20261117.bin');
  assert.equal(fw.downloadAccess.sourceUrl,fw.source.url);assert.equal(fw.downloadAccess.providerCode,'BURN_CAPTCHA_INVALID');assert.equal(fw.downloadAccess.status,'verification-required');
  assert.equal(fw.catalogUpdate.layoutVerified,false);assert.equal(fw.source.sha256,undefined);assert.equal(fw.simulationVerified,false);
  const old=fw.artifactHistory.find(h=>h.catalogUpdate.publisherIdentity?.versionId==='2117450155839557633');
  assert.ok(old);
  assert.equal(old.simulationVerified,false);
  // Synthetic HTTP fixtures test existing fail-closed checks, NOT firmware boot or live network.
  for(const [url,status,providerCode,code] of [[old.source.url,404,'VERSION_NOT_FOUND','FIRMWARE_VERSION_NOT_FOUND'],[fw.source.url,400,'BURN_CAPTCHA_INVALID','FIRMWARE_AUTH_REQUIRED']]){
    let requests=0;
    await assert.rejects(fetchBytes(url,{fetchImpl:async(_,init)=>{requests++;assert.equal(init.credentials,'omit');return new Response(JSON.stringify({data:{errorCode:providerCode}}),{status,headers:{'content-type':'application/json'}});}}),e=>e.code===code&&e.details.providerCode===providerCode);
    assert.equal(requests,1);
  }
});
