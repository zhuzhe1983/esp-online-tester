import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createDefaultScene,validateScene,encodeScene,decodeScene} from '../web/platform/catalog.js';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createScenePointerRouter} from '../web/platform/scene-interaction.js';
import {validateRecording} from '../web/platform/recording.js';
const catalog={};for(const name of ['products','accessories','firmwares'])catalog[name]=JSON.parse(await readFile(new URL(`../web/platform/catalog/${name}.json`,import.meta.url))).items;
const instance=()=>({id:'angle-unit',catalogId:'m5stack-unit-angle-u005',parameters:{position:.25},wiring:{mode:'physical',interfaceId:'grove'}});
const scene=()=>({...createDefaultScene(catalog,'m5stack-stopwatch'),accessories:[instance()]});
test('U005 routes to G11 with bounded initial position and a single physical Grove owner',async()=>{
 assert.equal(validateScene(scene(),catalog).valid,true);assert.deepEqual(decodeScene(encodeScene(scene())),scene());
 for(const value of [-.1,1.1,null,'0.5']){const s=scene();s.accessories[0].parameters.position=value;assert.equal(validateScene(s,catalog).valid,false);}
 for(const change of [s=>s.accessories[0].wiring.interfaceId='wrong',s=>{s.productId='m5stack-c123';s.keyMapping={};s.firmware=null;},s=>s.accessories.push({...instance(),id:'rgb',catalogId:'m5stack-unit-rgb-u003'})]){const s=scene();change(s);assert.equal(validateScene(s,catalog).valid,false);}
 const runtime=createPlatformRuntime({catalog});try{await runtime.configure(scene());assert.deepEqual((await runtime.capabilities()).input.accessories,[{accessoryId:'angle-unit',model:'potentiometer-u005-10k-curve',port:'grove',pin:11,controls:['knob']}]);assert.equal((await runtime.observe()).accessoryInputs[0].value,.25);await runtime.configure({...scene(),accessories:[]});assert.deepEqual((await runtime.observe()).accessoryInputs,[]);}finally{await runtime.dispose();}
});
test('U005 continuous angular drag wraps safely, clamps at end stops and never emits encoder/button events',()=>{
 const events=[];const router=createScenePointerRouter({onInput:event=>events.push(event),onOrbit:()=>assert.fail('knob must not orbit')});
 router.down({pointerId:1,x:0,y:0,surface:{kind:'potentiometer',id:'knob',accessoryId:'angle-unit',value:.5,sweep:Math.PI*1.5},point:{angle:3.1}});
 router.move({pointerId:1,x:1,y:0,point:{angle:-3.1}});assert.equal(events.length,1);assert.ok(events[0].value<.5&&events[0].value>.45);
 for(let i=0;i<15;i++)router.move({pointerId:1,x:i,y:0,point:{angle:-3.1-i*.8}});
 assert.equal(events.at(-1).value,1);router.up(1);assert.equal(router.activeCount,0);assert.ok(events.every(e=>e.type==='accessory-angle'&&e.id==='knob'&&e.accessoryId==='angle-unit'&&e.value>=0&&e.value<=1));
});
test('U005 recording preserves normalized input and rejects fake button or out-of-range values',()=>{
 const s=scene();s.firmware={url:'https://example.org/fixture.bin',sha256:'a'.repeat(64)};const r={schemaVersion:'1',scene:s,firmware:{source:'remote',manifestUrl:null,manifestHash:null,artifacts:[{kind:'flash',url:s.firmware.url,sha256:s.firmware.sha256,bytes:1024}],appDirect:false,stubs:[]},clock:{unit:'guest-seconds',origin:'reset',delivery:'worker-acknowledgement'},display:null,duration:2,events:[{at:1,type:'accessory-angle',accessoryId:'angle-unit',id:'knob',value:.75}]};assert.equal(validateRecording(r).valid,true);r.events[0].value=2;assert.equal(validateRecording(r).valid,false);r.events=[{at:1,type:'accessory-button',accessoryId:'angle-unit',id:'button',pressed:false}];assert.equal(validateRecording(r).valid,false);
});
