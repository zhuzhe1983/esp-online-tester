// StickT2-specific host boundaries, not firmware execution or GUI acceptance.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {chipIdForBoard} from '../web/platform/app-image.js';
import {validateMergedImage,createPlatformRuntime} from '../web/platform/runtime.js';
import {assembleLocalFirmware} from '../web/platform/local-firmware.js';
import {createDefaultScene} from '../web/platform/catalog.js';
const root=new URL('../',import.meta.url),catalog={};
for(const name of ['products','accessories','firmwares'])catalog[name]=JSON.parse(await readFile(new URL(`web/platform/catalog/${name}.json`,root),'utf8')).items;
const product=catalog.products.find(p=>p.id==='m5stack-k016-t2');
assert.equal(product.runtime.board,'stickt2');assert.equal(product.runtime.flashMb,4);assert.equal(product.display.touch,false);
assert.deepEqual(product.controls.map(c=>[c.id,c.pin]),[['btnA',37],['btnB',39],['power',undefined]]);
assert.equal(product.controls.find(control=>control.id==='power').transport,'board');
const model=JSON.parse(await readFile(new URL('web/platform/assets/m5stack-k016-t2/model.json',root),'utf8'));
const derived=new Map(model.derivedParts.map(part=>[part.id,part]));
const usb=derived.get('usb-c'),grove=derived.get('grove');
assert.equal(usb.housingVariant,'recessed');
assert.equal(usb.geometry.centerMm[0],grove.geometry.centerMm[0]);
assert.ok(usb.geometry.centerMm[1]>grove.geometry.centerMm[1]);
assert.ok(usb.geometry.centerMm[2]>grove.geometry.centerMm[2]);
for(const socket of [usb,grove]){
  assert.deepEqual(socket.geometry.rotation,[Math.PI/2,0,0]);
  assert.equal(socket.allowConnections,false);
}
assert.equal(derived.get('rotary-encoder-insert').kind,'plate');
assert.ok(derived.has('thermal-lens-window') && derived.has('rear-screw-left') && derived.has('rear-screw-right'));
const rear=model.parts.filter(part=>part.sourceComponent===1);
const sourceFaces=rear.flatMap(part=>part.sourceFaceRanges.flatMap(([first,last])=>Array.from({length:last-first},(_,offset)=>first+offset)));
assert.equal(sourceFaces.length,3130);
assert.equal(new Set(sourceFaces).size,3130);
assert.ok(sourceFaces.every(index=>index>=0 && index<3130));
assert.equal(rear.find(part=>part.name==='rear-m5-engraving').role,'shell');
assert.deepEqual(model.presentation.buttons.map(button=>button.id),['btnA','btnB','power']);
assert.deepEqual(model.presentation.buttons[0].sourceIds,['product-photo-5094fb4b8d']);
for(const id of ['btnB','power']){
  const region=model.presentation.buttons.find(button=>button.id===id);
  assert.deepEqual(region.sourceIds,['product-photo-d829ecdbd1','product-photo-552313a3da','manufacturer-cad-cfa964c8a1']);
  assert.equal(region.basis,'source-photo-and-existing-surface');
  assert.match(region.notes,/Input-only/);
  assert.equal(region.supportingPart,id==='btnB'?'shell-back':'shell-front');
}
for(const name of ['side-key-a','side-key-b'])assert.equal(model.parts.find(part=>part.name===name).controlId,undefined,'Hidden internal pieces must not become exterior button hits');
const [frontCover,board,rearCover]=model.presentation.inspectionGroups;
assert.deepEqual([frontCover.offsetMm,board.offsetMm,rearCover.offsetMm],[[0,0,32],[0,0,3],[0,0,-26]]);
assert.ok(frontCover.parts.includes('live-display') && frontCover.parts.includes('cover-key:btnA'));
assert.ok(frontCover.parts.includes('cover-key:power')&&!frontCover.parts.includes('cover-key:btnB'));
for (const part of ['pcb-main','usb-c','grove','grove-left-retainer','grove-right-retainer',
  'rotary-encoder-insert','rotary-recess-back','rotary-grip-1','rotary-grip-2','rotary-grip-3']) assert.ok(board.parts.includes(part));
assert.deepEqual(rearCover.parts,[
  'shell-back','rear-m5-engraving',
  'rear-screw-left','rear-screw-left-socket-floor','rear-screw-right','rear-screw-right-socket-floor',
  'thermal-lens-bezel','thermal-lens-window','cover-key:btnB'
]);
assert.ok(!rearCover.parts.includes('cover-key:power'));
assert.equal(chipIdForBoard('stickt2'),0);
const candidates=catalog.firmwares.filter(f=>f.productIds.includes(product.id));
assert.equal(candidates[0].id,product.defaultFirmwareId);assert.equal(candidates[0].role,'factory');assert.equal(candidates[0].simulationVerified,false);
assert.equal(candidates[0].source.sha256,'bb33ffd41f9c6f9235b2fb44eac893b7ce6f70879f48eed9cb1ff75ef9419cd7');
const runtime=createPlatformRuntime({catalog,baseUrl:'https://platform.example/platform/',workerFactory:()=>{throw Error('Host-only test must not start stale WASM');}});
try {
  await runtime.configure(createDefaultScene(catalog,product.id));const caps=await runtime.capabilities();
  assert.equal(caps.executable,true);assert.equal(caps.imu.model,'mpu6886-register-subset');assert.equal(caps.input.touch,false);
  assert.equal(caps.sdCard.slot,false);
  assert.deepEqual(caps.input.buttons,['btnA','btnB','power']);assert.deepEqual(caps.input.encoders,[]);
  assert.equal(caps.thermal.model,'lepton3-cci-raw14-subset');assert.equal(caps.thermal.calibrated,false);assert.equal(caps.thermal.environmentInput,'input:thermal-environment');
  assert.ok(caps.unsupported.includes('lepton3-calibrated-thermal-imaging'));assert.ok(caps.unsupported.includes('encoder-i2c-0x30'));
} finally {await runtime.dispose();}
// Explicitly SYNTHETIC layout-only fixtures. They do not represent official code.
const image=chip=>{const b=new Uint8Array(64);b[0]=0xe9;b[1]=1;b[12]=chip;new DataView(b.buffer).setUint32(28,4,true);return b;};
const table=new Uint8Array(0x1000).fill(255),view=new DataView(table.buffer);view.setUint16(0,0x50aa,true);table[2]=0;table[3]=0;view.setUint32(4,0x10000,true);view.setUint32(8,0x100000,true);
const file=(name,data)=>new File([data],name);
const build=chip=>({format:'build',files:[file('flasher_args.json',JSON.stringify({flash_files:{'0x1000':'boot.bin','0x8000':'part.bin','0x10000':'app.bin'},extra_esptool_args:{chip}})),file('boot.bin',image(0)),file('part.bin',table),file('app.bin',image(0))]});
const merged=await assembleLocalFirmware(build('esp32'),{board:'stickt2',flashBytes:4<<20});
assert.equal(merged.data[0x1000],0xe9);assert.equal(merged.data[0],255);assert.equal(validateMergedImage(merged.data,'stickt2',4<<20),true);
assert.throws(()=>validateMergedImage(merged.data,'stopwatch',4<<20),{code:'INCOMPLETE_IMAGE'});
await assert.rejects(()=>assembleLocalFirmware(build('esp32s3'),{board:'stickt2',flashBytes:4<<20}),{code:'CHIP_MISMATCH'});
console.log('ok StickT2: exact SKU, factory-first/hash retained, classic ESP32 local/merged layout, truthful API limits; no WASM/GUI compatibility claim');
