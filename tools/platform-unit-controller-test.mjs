import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createDefaultScene,validateScene} from '../web/platform/catalog.js';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {chipForBoard} from '../web/platform/app-image.js';
import {thermalDefaults,validThermal} from '../web/platform/thermal-input.js';
import {mountThermalEnvironment} from '../web/platform/thermal-ui.js';
import {validateRecording} from '../web/platform/recording.js';
const catalog={};for(const key of ['products','accessories','firmwares'])catalog[key]=JSON.parse(await readFile(new URL(`../web/platform/catalog/${key}.json`,import.meta.url))).items;
const product=id=>catalog.products.find(p=>p.id===id);
test('four exact classic controllers retain independent runtime, memory and firmware identities',async()=>{
 for(const[id,board,flash,ram]of[['m5stack-k117','atomu',4,0],['m5stack-u120','unit-lcd',4,0],['m5stack-u149','unit-thermal2',4,0],['m5stack-k115','atom-psram-display',8,2]]){
  const p=product(id);assert.equal(chipForBoard(board),'esp32');assert.equal(p.runtime.board,board);assert.equal(p.runtime.flashMb,flash);assert.equal(p.runtime.psramMb,ram);assert.equal(p.runtime.status,'partial');
  const r=createPlatformRuntime({catalog,baseUrl:'https://unit.invalid/platform/',workerFactory:()=>{throw Error('Host contract does not claim guest execution');}});try{await r.configure(createDefaultScene(catalog,id));assert.equal((await r.capabilities()).executable,true);}finally{await r.dispose();}
 }
 assert.equal(product('m5stack-k115').display,null,'FPGA external monitor must not become an invented internal LCD');
 assert.equal(product('m5stack-u120').display.controller,'ST7789V2');
 const factory=catalog.firmwares.find(f=>f.id===product('m5stack-u149').defaultFirmwareId);assert.equal(factory.status,'trial');assert.equal(factory.simulationVerified,false);assert.equal(factory.source.sha256,'ba1c95464cb8163f48aa12e9e807cfce5d3b32632dcde54b8859444448449c55');
});
test('Unit Thermal2 scene, capabilities and recording use its32x24 ADC profile independently of Lepton',async()=>{
 const id='m5stack-u149',sample={background:0,hotspot:32767,centerX:31,centerY:23,radius:32};
 assert(validThermal(sample,id));assert(!validThermal({...sample,centerX:32},id));assert(!validThermal(sample,'m5stack-k016-t2'));
 const scene=createDefaultScene(catalog,id);scene.thermalEnvironment=sample;assert.equal(validateScene(scene,catalog).valid,true);assert.equal(validateScene({...scene,thermalEnvironment:{...sample,centerY:24}},catalog).valid,false);
 const runtime=createPlatformRuntime({catalog,baseUrl:'https://unit.invalid/platform/',workerFactory:()=>{throw Error('No guest execution');}});
 try{await runtime.configure(scene);assert.deepEqual((await runtime.observe()).thermalEnvironment,sample);const c=await runtime.capabilities();assert.equal(c.thermal.model,'mlx90640-register-raw16-subset');assert(c.unsupported.includes('mlx90640-physical-calibration'));assert(!c.unsupported.includes('encoder-i2c-0x30'));}finally{await runtime.dispose();}
 const recording={schemaVersion:'1',scene:{...scene,firmware:{url:'https://unit.invalid/full.bin',sha256:'1'.repeat(64)}},firmware:{sha256:'1'.repeat(64)},clock:{unit:'seconds',basis:'guest'},display:null,duration:1,events:[{at:.5,type:'thermal-environment',...sample}]};
 const result=validateRecording(recording);assert(!result.errors.some(e=>e.code==='RECORDING_THERMAL'),JSON.stringify(result.errors));
});
test('shared thermal form switches native ADC ranges and preserves large integer stimuli',async()=>{
 class Node {constructor(){this.children=[];this.handlers={};this.dataset={};}append(...xs){for(const x of xs){x.parentElement=this;this.children.push(x);}}setAttribute(k,v){this[k]=v;}addEventListener(k,fn){this.handlers[k]=fn;}replaceChildren(){this.children=[];}reportValidity(){return true;}}
 const document={createElement:()=>new Node()},container=new Node();container.ownerDocument=document;let listener;const sent=[];const state={status:'running',scene:{productId:'m5stack-u149'},thermalEnvironment:thermalDefaults('m5stack-u149')};
 const runtime={subscribe:fn=>(listener=fn,()=>{}),observe:async()=>state,input:async event=>(sent.push(event),{guestTime:1})};const form=mountThermalEnvironment({container,runtime});await Promise.resolve();
 const all=n=>[n,...n.children.flatMap(all)],nodes=all(container),hotspot=nodes.find(n=>n.dataset.thermalField==='hotspot'),x=nodes.find(n=>n.dataset.thermalField==='centerX'),apply=nodes.find(n=>Object.hasOwn(n.dataset,'thermalApply'));
 assert.equal(hotspot.max,'32767');assert.equal(x.max,'31');hotspot.value='20000';apply.handlers.click();await Promise.resolve();await Promise.resolve();assert.equal(sent[0].hotspot,20000);assert.equal(container.hidden,false);
 listener({type:'state',state:{status:'running',scene:{productId:'m5stack-k016-t2'}}});assert.equal(hotspot.max,'16383');assert.equal(x.max,'159');form.dispose();
});
