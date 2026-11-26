import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {chipForBoard,isH2Board,isC5Board,isC61Board,validateAppImage} from '../web/platform/app-image.js';
import {waveshareH2CoreProbe} from './fixtures/waveshare-h2-core-probe.mjs';
const expected={'m5-acssr':'esp32c3','m5-dcssr':'esp32c3','m5-kmeter':'esp32c3','m5-switchc6':'esp32c6','m5-stamp-addon-c6':'esp32c6','m5-unit-gateway-h2':'esp32h2','m5-module-gateway-h2':'esp32h2','m5-cardkb2':'esp32c61','m5-tough-c5':'esp32c5'};
test('M5 edge exact profiles route to the real chip and retain SRAM-only H2 memory boundaries',async()=>{
 for(const [board,chip]of Object.entries(expected))assert.equal(chipForBoard(board),chip);
 assert(isH2Board('m5stack-u195'));assert(isH2Board('m5stack-m141'));assert(isC5Board('m5stack-k162'));assert(isC61Board('m5stack-u215'));
 const image=Buffer.from(waveshareH2CoreProbe().subarray(0,-32));image[23]=0;image[3]=0x10;
 for(const board of ['m5-unit-gateway-h2','m5-module-gateway-h2','m5stack-u195','m5stack-m141']){
  await validateAppImage(image,board,2<<20);await assert.rejects(validateAppImage(image,board,4<<20),e=>e.code==='FLASH_CAPACITY_UNSUPPORTED');
  const tooLarge=Buffer.from(image);tooLarge[3]=0x20;await assert.rejects(validateAppImage(tooLarge,board,2<<20),e=>e.code==='FLASH_CAPACITY_UNSUPPORTED');
 }
 await assert.rejects(validateAppImage(image,'nanoh2',2<<20),e=>e.code==='FLASH_CAPACITY_UNSUPPORTED');
 const wrong=Buffer.from(image);wrong.writeUInt16LE(13,12);await assert.rejects(validateAppImage(wrong,'m5-unit-gateway-h2',2<<20),e=>e.code==='CHIP_MISMATCH');
});
test('CardKB2 catalog retains the complete physical matrix and excludes only two absent switches',async()=>{
 const products=JSON.parse(await readFile(new URL('../web/platform/catalog/products.json',import.meta.url))).items;
 const kb=products.find(p=>p.id==='m5stack-u215'),keys=kb.controls.filter(c=>c.type==='matrix-key');
 assert.equal(keys.length,42);assert.equal(new Set(keys.map(k=>k.row+','+k.column)).size,42);
 for(let row=0;row<4;row++)for(let column=0;column<11;column++)assert.equal(keys.some(k=>k.row===row&&k.column===column),!(column===10&&[0,3].includes(row)));
 for(const key of keys)assert.equal(key.scanIndex,key.row*11+key.column);
});

test('thermocouple recording keeps physical stimulus separate from temperature output',async()=>{
 const {validateRecording}=await import('../web/platform/recording.js');
 const url='https://example.org/kmeter.bin',hash='a'.repeat(64);
 const recording={schemaVersion:'1',scene:{schemaVersion:'1',productId:'m5stack-u133',firmware:{url,sha256:hash},accessories:[],keyMapping:{},view:{}},firmware:{source:'remote',manifestUrl:null,manifestHash:null,appDirect:true,stubs:[],artifacts:[{kind:'app',url,sha256:hash,bytes:1024}]},clock:{unit:'guest-seconds',origin:'reset',delivery:'worker-acknowledgement'},display:null,duration:1,events:[{at:0.1,type:'thermocouple-environment',connected:true,temperatureC:-30.5},{at:0.5,type:'thermocouple-environment',connected:false,temperatureC:-30.5}]};
 assert.equal(validateRecording(recording).valid,true);
 const bad=structuredClone(recording);bad.events[0].temperatureC=1350.25;assert.equal(validateRecording(bad).valid,false);
 bad.events[0].temperatureC=25;bad.scene.productId='m5stack-u133-b';assert.equal(validateRecording(bad).valid,false);
 assert(!Object.hasOwn(recording.events[0],'guestTemperatureC'),'stimulus does not fabricate a firmware result');
});
