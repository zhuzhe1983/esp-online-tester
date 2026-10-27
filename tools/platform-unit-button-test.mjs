import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createDefaultScene,validateScene,encodeScene,decodeScene} from '../web/platform/catalog.js';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {controlInput} from '../web/platform/control-input.js';
import {validateRecording,encodeRecording,decodeRecording} from '../web/platform/recording.js';
const catalog={};for(const name of ['products','accessories','firmwares'])catalog[name]=JSON.parse(await readFile(new URL(`../web/platform/catalog/${name}.json`,import.meta.url))).items;
const instance=()=>({id:'button-unit',catalogId:'m5stack-unit-button-u027',parameters:{},wiring:{mode:'physical',interfaceId:'grove'}});
const scene=()=>({...createDefaultScene(catalog,'m5stack-stopwatch'),accessories:[instance()]});
test('U027 permits only the audited host/port and rejects overlapping pin assignments',()=>{
  assert.equal(validateScene(scene(),catalog).valid,true);
  assert.deepEqual(decodeScene(encodeScene(scene())),scene());
  for(const change of [s=>{s.productId='m5stack-c123';s.keyMapping={};s.firmware=null;},s=>{s.accessories[0].wiring.interfaceId='internal-i2c';},s=>{s.accessories[0].wiring.pin=10;},s=>{s.accessories.push({...instance(),id:'second-button'});},s=>{s.accessories[0].catalogId='m5stack-gps-u032-v11';}]){
    const s=scene();change(s);assert.equal(validateScene(s,catalog).valid,false);
  }
  const overlap=scene();overlap.accessories.push({id:'gps',catalogId:'m5stack-gps-u032-v11',parameters:{},wiring:{mode:'physical',connectorId:'grove',port:'uart1',rx:11,tx:10}});assert.ok(validateScene(overlap,catalog).errors.some(e=>e.code==='PIN_CONFLICT'));
  overlap.accessories.reverse();assert.ok(validateScene(overlap,catalog).errors.some(e=>e.code==='PIN_CONFLICT'));
  const visual=scene();visual.productId='m5stack-c123';visual.firmware=null;visual.keyMapping={};visual.accessories[0].wiring={mode:'visual'};assert.equal(validateScene(visual,catalog).valid,true);
});
test('configure publishes semantic controls, reconfiguration removes the old physical route',async()=>{
  const runtime=createPlatformRuntime({catalog});try{
    await runtime.configure(scene());assert.deepEqual((await runtime.capabilities()).input.accessories,[{accessoryId:'button-unit',model:'gpio-button-u027',port:'grove',pin:11,controls:['button']}]);
    assert.deepEqual(controlInput({type:'button',id:'accessory:button-unit:button',pressed:true},{}),{type:'accessory-button',accessoryId:'button-unit',id:'button',pressed:true});
    await runtime.configure({...scene(),accessories:[]});assert.deepEqual((await runtime.observe()).accessoryInputs,[]);
    await runtime.configure(scene());assert.equal((await runtime.observe()).accessoryInputs[0].pressed,false);
  }finally{await runtime.dispose();}
});
test('recording tracks attachment, validates switch identity and treats detach as release',()=>{
  const s=scene();s.firmware={url:'https://example.org/fixture.bin',sha256:'a'.repeat(64)};
  const r={schemaVersion:'1',scene:s,firmware:{source:'remote',manifestUrl:null,manifestHash:null,artifacts:[{kind:'flash',url:s.firmware.url,sha256:s.firmware.sha256,bytes:1024}],appDirect:false,stubs:[]},clock:{unit:'guest-seconds',origin:'reset',delivery:'worker-acknowledgement'},display:null,duration:3,events:[{at:1,type:'accessory-button',accessoryId:'button-unit',id:'button',pressed:true},{at:2,type:'accessory-connection',accessoryId:'button-unit',connected:false}]};
  assert.equal(validateRecording(r).valid,true);assert.deepEqual(decodeRecording(encodeRecording(r)),r);
  const afterDetach=structuredClone(r);afterDetach.events.push({...r.events[0],at:3});assert.equal(validateRecording(afterDetach).valid,false);
  for(const [key,value] of [['accessoryId','missing'],['id','arbitrary'],['pressed',1],['pin',11]]){const bad=structuredClone(r);bad.events[0][key]=value;assert.equal(validateRecording(bad).valid,false);}
  const held=structuredClone(r);held.events.pop();assert.equal(validateRecording(held).valid,false);
});
