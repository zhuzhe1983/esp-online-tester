import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createDefaultScene,validateScene,encodeScene,decodeScene} from '../web/platform/catalog.js';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {validateRecording} from '../web/platform/recording.js';
const catalog={};for(const name of ['products','accessories','firmwares'])catalog[name]=JSON.parse(await readFile(new URL(`../web/platform/catalog/${name}.json`,import.meta.url))).items;
const unit=()=>({id:'light',catalogId:'m5stack-unit-dlight-u136',parameters:{lux:1234.5},wiring:{mode:'physical',interfaceId:'grove'}});
const scene=()=>({...createDefaultScene(catalog,'m5stack-stopwatch'),accessories:[unit()]});
const via=(id,catalogId,port)=>({id,catalogId,parameters:{},wiring:{mode:'physical',viaAccessoryId:'hub',portId:port}});
const pair=()=>({...scene(),accessories:[{id:'hub',catalogId:'m5stack-unit-hub-u006',parameters:{},wiring:{mode:'physical',interfaceId:'grove'}},via('light','m5stack-unit-dlight-u136','port-1'),via('env','m5stack-unit-env4-u001d','port-2')]});
test('DLight illuminance bounds and exact physical I2C route survive share/configure',async()=>{
 const s=scene();assert.equal(validateScene(s,catalog).valid,true);assert.deepEqual(decodeScene(encodeScene(s)),s);
 for(const lux of [-1,100001,NaN,Infinity,null,'1000']){const bad=scene();bad.accessories[0].parameters.lux=lux;assert.equal(validateScene(bad,catalog).valid,false);}
 for(const lux of [0,100000]){const edge=scene();edge.accessories[0].parameters.lux=lux;assert.equal(validateScene(edge,catalog).valid,true);}
 const runtime=createPlatformRuntime({catalog});try{await runtime.configure(s);const [cap]=(await runtime.capabilities()).input.accessories;assert.deepEqual(cap.pins,[10,11]);assert.deepEqual(cap.addresses,[0x23]);assert.deepEqual(cap.controls,['illuminance']);assert.equal((await runtime.observe()).accessoryInputs[0].lux,1234.5);await runtime.configure({...s,accessories:[]});assert.deepEqual((await runtime.observe()).accessoryInputs,[]);}finally{await runtime.dispose();}
});
test('one Hub may share distinct I2C addresses but never duplicate address, socket or a GPIO driver',()=>{
 assert.equal(validateScene(pair(),catalog).valid,true);const reverse=pair();reverse.accessories.reverse();assert.equal(validateScene(reverse,catalog).valid,true);
 for(const catalogId of ['m5stack-unit-dlight-u136','m5stack-unit-env4-u001d','m5stack-unit-button-u027','m5stack-unit-rgb-u003','m5stack-unit-angle-u005']){
  const bad=pair();bad.accessories.push(via('conflict',catalogId,'port-3'));assert.equal(validateScene(bad,catalog).valid,false);assert.ok(validateScene(bad,catalog).errors.some(e=>e.code==='PIN_CONFLICT'));
 }
 for(const mutate of [s=>s.accessories[2].wiring.portId='port-1',s=>s.accessories[1].wiring={mode:'physical',interfaceId:'grove'},s=>{s.productId='m5stack-c123';s.keyMapping={};s.firmware=null;},s=>s.accessories[1].wiring.portId='port-4']){const bad=pair();mutate(bad);assert.equal(validateScene(bad,catalog).valid,false);}
});
test('DLight recording keeps ambient changes while disconnected and rejects missing, wrong-model or invalid lux',()=>{
 const s=scene();s.firmware={url:'https://example.org/fixture.bin',sha256:'a'.repeat(64)};const r={schemaVersion:'1',scene:s,firmware:{source:'remote',manifestUrl:null,manifestHash:null,artifacts:[{kind:'flash',url:s.firmware.url,sha256:s.firmware.sha256,bytes:1024}],appDirect:false,stubs:[]},clock:{unit:'guest-seconds',origin:'reset',delivery:'worker-acknowledgement'},display:null,duration:3,events:[{at:1,type:'accessory-connection',accessoryId:'light',connected:false},{at:2,type:'accessory-illuminance',accessoryId:'light',lux:600}]};assert.equal(validateRecording(r).valid,true);
 for(const mutate of [r=>delete r.events[1].lux,r=>r.events[1].lux=-1,r=>r.events[1].lux=100001,r=>r.scene.accessories[0].catalogId='m5stack-unit-env4-u001d']){const bad=structuredClone(r);mutate(bad);assert.equal(validateRecording(bad).valid,false);}
});
