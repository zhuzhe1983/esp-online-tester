import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createDefaultScene,validateScene,encodeScene,decodeScene} from '../web/platform/catalog.js';
import {routedAccessories} from '../web/platform/accessory-routing.js';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {validateRecording} from '../web/platform/recording.js';
const catalog={};for(const key of ['products','accessories','firmwares'])catalog[key]=JSON.parse(await readFile(new URL(`../web/platform/catalog/${key}.json`,import.meta.url))).items;
const host=catalog.products.find(p=>p.id==='m5stack-stopwatch');
const scene=()=>({...createDefaultScene(catalog,host.id),accessories:[{id:'hub',catalogId:'m5stack-unit-hub-u006',parameters:{},wiring:{mode:'physical',interfaceId:'grove'}},{id:'button',catalogId:'m5stack-unit-button-u027',parameters:{},wiring:{mode:'physical',viaAccessoryId:'hub',portId:'port-1'}},{id:'rgb',catalogId:'m5stack-unit-rgb-u003',parameters:{},wiring:{mode:'physical',viaAccessoryId:'hub',portId:'port-3'}}]});
test('U006 shares original host wires without inventing new GPIOs; portable scene preserves topology',async()=>{
 const s=scene();assert.equal(validateScene(s,catalog).valid,true);assert.deepEqual(decodeScene(encodeScene(s)),s);
 const routes=routedAccessories(s,host,catalog);assert.deepEqual(routes.map(r=>[r.accessoryId,r.pin,r.viaAccessoryId]),[['hub',undefined,undefined],['button',11,'hub'],['rgb',10,'hub']]);
 const runtime=createPlatformRuntime({catalog});try{await runtime.configure(s);const caps=await runtime.capabilities();assert.equal(caps.input.accessories[0].hub,true);assert.deepEqual(caps.input.accessories[0].controls,[]);assert.equal(caps.input.accessories[1].viaAccessoryId,'hub');await runtime.configure({...s,accessories:[]});assert.deepEqual((await runtime.observe()).accessoryInputs,[]);}finally{await runtime.dispose();}
});
test('U006 rejects occupied sockets, shared-signal contention, orphan paths, unknown ports, cycles and cascades',()=>{
 const invalid=[s=>s.accessories[2].wiring.portId='port-1',s=>s.accessories[2].catalogId='m5stack-unit-angle-u005',s=>s.accessories[1].wiring.viaAccessoryId='missing',s=>s.accessories[1].wiring.viaAccessoryId='button',s=>s.accessories[1].wiring.portId='port-4',s=>s.accessories[1].wiring.interfaceId='grove',s=>s.accessories[1].catalogId='m5stack-unit-hub-u006',s=>s.accessories.push({id:'other',catalogId:'m5stack-unit-rgb-u003',parameters:{},wiring:{mode:'physical',interfaceId:'grove'}}),s=>s.accessories[0].wiring={mode:'visual'},s=>{s.productId='m5stack-c123';s.keyMapping={};s.firmware=null;}];
 for(const mutate of invalid){const s=scene();mutate(s);assert.equal(validateScene(s,catalog).valid,false,JSON.stringify(s.accessories));}
 const s=scene();s.accessories[1].catalogId='m5stack-unit-angle-u005';assert.equal(validateScene(s,catalog).valid,true);
});
test('U006 recording treats parent disconnect as button release and requires effective connection for presses',()=>{
 const s=scene();s.firmware={url:'https://example.org/fixture.bin',sha256:'a'.repeat(64)};
 const r={schemaVersion:'1',scene:s,firmware:{source:'remote',manifestUrl:null,manifestHash:null,artifacts:[{kind:'flash',url:s.firmware.url,sha256:s.firmware.sha256,bytes:1024}],appDirect:false,stubs:[]},clock:{unit:'guest-seconds',origin:'reset',delivery:'worker-acknowledgement'},display:null,duration:3,events:[{at:1,type:'accessory-button',accessoryId:'button',id:'button',pressed:true},{at:2,type:'accessory-connection',accessoryId:'hub',connected:false}]};
 assert.equal(validateRecording(r).valid,true);r.events.push({at:3,type:'accessory-button',accessoryId:'button',id:'button',pressed:true});assert.equal(validateRecording(r).valid,false);
});
