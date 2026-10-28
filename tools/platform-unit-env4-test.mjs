import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createDefaultScene,validateScene,encodeScene,decodeScene} from '../web/platform/catalog.js';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {validateRecording} from '../web/platform/recording.js';
const catalog={};for(const name of ['products','accessories','firmwares'])catalog[name]=JSON.parse(await readFile(new URL(`../web/platform/catalog/${name}.json`,import.meta.url))).items;
const unit=()=>({id:'environment',catalogId:'m5stack-unit-env4-u001d',parameters:{temperatureC:22,humidityPercent:60,pressureHpa:990},wiring:{mode:'physical',interfaceId:'grove'}});
const scene=()=>({...createDefaultScene(catalog,'m5stack-stopwatch'),accessories:[unit()]});
test('ENV-IV bounded ambient, I2C addresses and both physical GPIOs survive portable scene',async()=>{
 const s=scene();assert.equal(validateScene(s,catalog).valid,true);assert.deepEqual(decodeScene(encodeScene(s)),s);
 for(const [key,values] of [['temperatureC',[-41,86,null,'25',NaN]],['humidityPercent',[-1,101,null]],['pressureHpa',[299,1101,null]]])for(const value of values){const bad=scene();bad.accessories[0].parameters[key]=value;assert.equal(validateScene(bad,catalog).valid,false,`${key}=${value}`);}
 for(const values of [[-40,0,300],[85,100,1100]]){const edge=scene();edge.accessories[0].parameters=Object.fromEntries(['temperatureC','humidityPercent','pressureHpa'].map((key,i)=>[key,values[i]]));assert.equal(validateScene(edge,catalog).valid,true);}
 const runtime=createPlatformRuntime({catalog});try{await runtime.configure(s);const [cap]=(await runtime.capabilities()).input.accessories;assert.deepEqual(cap.pins,[10,11]);assert.deepEqual(cap.addresses,[0x44,0x76]);assert.deepEqual(cap.controls,['environment']);assert.equal((await runtime.observe()).accessoryInputs[0].temperatureC,22);await runtime.configure({...s,accessories:[]});assert.deepEqual((await runtime.observe()).accessoryInputs,[]);}finally{await runtime.dispose();}
});
test('ENV-IV cannot claim another host, swapped connector, or GPIO shared with Hub leaves',()=>{
 for(const mutate of [s=>s.accessories[0].wiring.interfaceId='wrong',s=>{s.productId='m5stack-c123';s.keyMapping={};s.firmware=null;}]){const s=scene();mutate(s);assert.equal(validateScene(s,catalog).valid,false);}
 const s=scene();s.accessories[0].wiring={mode:'physical',viaAccessoryId:'hub',portId:'port-1'};s.accessories.unshift({id:'hub',catalogId:'m5stack-unit-hub-u006',parameters:{},wiring:{mode:'physical',interfaceId:'grove'}});assert.equal(validateScene(s,catalog).valid,true);
 for(const catalogId of ['m5stack-unit-button-u027','m5stack-unit-rgb-u003','m5stack-unit-angle-u005','m5stack-unit-env4-u001d']){const bad=structuredClone(s);bad.accessories.push({id:'competitor',catalogId,parameters:{},wiring:{mode:'physical',viaAccessoryId:'hub',portId:'port-2'}});const result=validateScene(bad,catalog);assert.equal(result.valid,false);assert.ok(result.errors.some(e=>e.code==='PIN_CONFLICT'));}
});
test('ENV-IV recording retains an ambient sample while open but rejects incomplete or wrong-model input',()=>{
 const s=scene();s.firmware={url:'https://example.org/fixture.bin',sha256:'a'.repeat(64)};const r={schemaVersion:'1',scene:s,firmware:{source:'remote',manifestUrl:null,manifestHash:null,artifacts:[{kind:'flash',url:s.firmware.url,sha256:s.firmware.sha256,bytes:1024}],appDirect:false,stubs:[]},clock:{unit:'guest-seconds',origin:'reset',delivery:'worker-acknowledgement'},display:null,duration:3,events:[{at:1,type:'accessory-connection',accessoryId:'environment',connected:false},{at:2,type:'accessory-environment',accessoryId:'environment',temperatureC:10,humidityPercent:20,pressureHpa:800}]};assert.equal(validateRecording(r).valid,true);
 for(const mutate of [r=>delete r.events[1].pressureHpa,r=>r.events[1].humidityPercent=101,r=>r.scene.accessories[0].catalogId='m5stack-unit-angle-u005']){const bad=structuredClone(r);mutate(bad);assert.equal(validateRecording(bad).valid,false);}
});
