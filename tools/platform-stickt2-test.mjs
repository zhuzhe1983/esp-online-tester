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
assert.equal(chipIdForBoard('stickt2'),0);
const candidates=catalog.firmwares.filter(f=>f.productIds.includes(product.id));
assert.equal(candidates[0].id,product.defaultFirmwareId);assert.equal(candidates[0].role,'factory');assert.equal(candidates[0].simulationVerified,false);
assert.equal(candidates[0].source.sha256,'bb33ffd41f9c6f9235b2fb44eac893b7ce6f70879f48eed9cb1ff75ef9419cd7');
const runtime=createPlatformRuntime({catalog,baseUrl:'https://platform.example/platform/',workerFactory:()=>{throw Error('Host-only test must not start stale WASM');}});
try {
  await runtime.configure(createDefaultScene(catalog,product.id));const caps=await runtime.capabilities();
  assert.equal(caps.executable,true);assert.equal(caps.imu.model,'mpu6886-register-subset');assert.equal(caps.input.touch,false);
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
