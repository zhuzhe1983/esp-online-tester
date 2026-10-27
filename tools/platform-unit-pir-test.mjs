import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createDefaultScene,validateScene,encodeScene,decodeScene} from '../web/platform/catalog.js';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {validateRecording} from '../web/platform/recording.js';
const catalog={};for(const name of ['products','accessories','firmwares'])catalog[name]=JSON.parse(await readFile(new URL(`../web/platform/catalog/${name}.json`,import.meta.url))).items;
const unit=()=>({id:'pir',catalogId:'m5stack-unit-pir-u004',parameters:{motion:false},wiring:{mode:'physical',interfaceId:'grove'}});
const scene=()=>({...createDefaultScene(catalog,'m5stack-stopwatch'),accessories:[unit()]});
const via=(id,catalogId,port)=>({id,catalogId,parameters:{},wiring:{mode:'physical',viaAccessoryId:'hub',portId:port}});
const pair=()=>({...scene(),accessories:[{id:'hub',catalogId:'m5stack-unit-hub-u006',parameters:{},wiring:{mode:'physical',interfaceId:'grove'}},via('pir','m5stack-unit-pir-u004','port-1'),via('rgb','m5stack-unit-rgb-u003','port-2')]});
test('PIR exact G11 route validates boolean stimulus and shares without losing its initial environment',async()=>{
 const s=scene();assert.equal(validateScene(s,catalog).valid,true);assert.deepEqual(decodeScene(encodeScene(s)),s);
 for(const motion of [0,1,null,'false',[],{}]){const bad=scene();bad.accessories[0].parameters.motion=motion;assert.equal(validateScene(bad,catalog).valid,false);}
 for(const motion of [false,true,undefined]){const edge=scene();if(motion===undefined)delete edge.accessories[0].parameters.motion;else edge.accessories[0].parameters.motion=motion;assert.equal(validateScene(edge,catalog).valid,true);}
 const runtime=createPlatformRuntime({catalog});try{await runtime.configure(s);const [cap]=(await runtime.capabilities()).input.accessories;assert.equal(cap.pin,11);assert.deepEqual(cap.controls,['motion']);assert.equal((await runtime.observe()).accessoryInputs[0].motion,false);await runtime.configure({...s,accessories:[]});assert.deepEqual((await runtime.observe()).accessoryInputs,[]);}finally{await runtime.dispose();}
});
test('PIR can share U006 with G10 RGB but rejects another G11 driver or occupied socket',()=>{
 assert.equal(validateScene(pair(),catalog).valid,true);const reverse=pair();reverse.accessories.reverse();assert.equal(validateScene(reverse,catalog).valid,true);
 for(const catalogId of ['m5stack-unit-pir-u004','m5stack-unit-dlight-u136','m5stack-unit-env4-u001d','m5stack-unit-button-u027','m5stack-unit-angle-u005']){const bad=pair();bad.accessories.push(via('conflict',catalogId,'port-3'));assert.ok(validateScene(bad,catalog).errors.some(e=>e.code==='PIN_CONFLICT'));}
 for(const mutate of [s=>s.accessories[2].wiring.portId='port-1',s=>s.accessories[1].wiring={mode:'physical',interfaceId:'grove'},s=>{s.productId='m5stack-c123';s.keyMapping={};s.firmware=null;},s=>s.accessories[1].wiring.portId='port-4']){const bad=pair();mutate(bad);assert.equal(validateScene(bad,catalog).valid,false);}
});
test('recording accepts ongoing environmental motion even while disconnected, not button hold rules',()=>{
 const s=scene();s.firmware={url:'https://example.org/fixture.bin',sha256:'a'.repeat(64)};const r={schemaVersion:'1',scene:s,firmware:{source:'remote',manifestUrl:null,manifestHash:null,artifacts:[{kind:'flash',url:s.firmware.url,sha256:s.firmware.sha256,bytes:1024}],appDirect:false,stubs:[]},clock:{unit:'guest-seconds',origin:'reset',delivery:'worker-acknowledgement'},display:null,duration:3,events:[{at:1,type:'accessory-connection',accessoryId:'pir',connected:false},{at:2,type:'accessory-motion',accessoryId:'pir',motion:true}]};assert.equal(validateRecording(r).valid,true);
 for(const mutate of [r=>delete r.events[1].motion,r=>r.events[1].motion=1,r=>r.events[1].motion=null,r=>r.scene.accessories[0].catalogId='m5stack-unit-button-u027',r=>r.events[1].accessoryId='unknown']){const bad=structuredClone(r);mutate(bad);assert.equal(validateRecording(bad).valid,false);}
});
