import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createDefaultScene,validateScene,encodeScene,decodeScene} from '../web/platform/catalog.js';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {validateRecording} from '../web/platform/recording.js';
const catalog={};for(const name of ['products','accessories','firmwares'])catalog[name]=JSON.parse(await readFile(new URL(`../web/platform/catalog/${name}.json`,import.meta.url))).items;
const instance=()=>({id:'rgb-unit',catalogId:'m5stack-unit-rgb-u003',parameters:{},wiring:{mode:'physical',interfaceId:'grove'}});
const scene=()=>({...createDefaultScene(catalog,'m5stack-stopwatch'),accessories:[instance()]});
test('U003 physical route is G10 on one StopWatch Grove input, with whole-port exclusivity',()=>{
  assert.equal(validateScene(scene(),catalog).valid,true);assert.deepEqual(decodeScene(encodeScene(scene())),scene());
  for(const change of [s=>{s.productId='m5stack-c123';s.firmware=null;s.keyMapping={};},s=>{s.accessories[0].wiring.interfaceId='out';},s=>{s.accessories[0].wiring.pin=11;},s=>{s.accessories[0].wiring={mode:'physical',connectorId:'grove',port:'uart1',rx:10,tx:11};},s=>{s.accessories.push({...instance(),id:'other'});},s=>{s.accessories[0].catalogId='m5stack-unit-angle-u005';}]){const s=scene();change(s);assert.equal(validateScene(s,catalog).valid,false);}
  const s=scene();s.accessories.push({...instance(),id:'button',catalogId:'m5stack-unit-button-u027'});assert.ok(validateScene(s,catalog).errors.some(e=>e.code==='PIN_CONFLICT'));s.accessories.reverse();assert.ok(validateScene(s,catalog).errors.some(e=>e.code==='PIN_CONFLICT'));
});
test('U003 exposes output and connection, never a fake button or frontend colour setter',async()=>{
 const runtime=createPlatformRuntime({catalog});try{await runtime.configure(scene());assert.deepEqual((await runtime.capabilities()).input.accessories,[{accessoryId:'rgb-unit',model:'sk6812-u003',port:'grove',pin:10,controls:[]}]);assert.deepEqual((await runtime.observe()).accessoryOutputs,[{accessoryId:'rgb-unit',model:'sk6812-u003',port:'grove',leds:[[0,0,0],[0,0,0],[0,0,0]]}]);await runtime.configure({...scene(),accessories:[]});assert.deepEqual((await runtime.observe()).accessoryOutputs,[]);}finally{await runtime.dispose();}
});
test('U003 recording allows plug events and rejects button events for output-only units',()=>{
 const s=scene();s.firmware={url:'https://example.org/fixture.bin',sha256:'a'.repeat(64)};const r={schemaVersion:'1',scene:s,firmware:{source:'remote',manifestUrl:null,manifestHash:null,artifacts:[{kind:'flash',url:s.firmware.url,sha256:s.firmware.sha256,bytes:1024}],appDirect:false,stubs:[]},clock:{unit:'guest-seconds',origin:'reset',delivery:'worker-acknowledgement'},display:null,duration:2,events:[{at:1,type:'accessory-connection',accessoryId:'rgb-unit',connected:false}]};assert.equal(validateRecording(r).valid,true);r.events=[{at:1,type:'accessory-button',accessoryId:'rgb-unit',id:'button',pressed:false}];assert.equal(validateRecording(r).valid,false);
});
