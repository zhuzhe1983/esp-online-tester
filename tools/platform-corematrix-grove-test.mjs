import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createDefaultScene,validateScene,encodeScene,decodeScene} from '../web/platform/catalog.js';
import {routedAccessories} from '../web/platform/accessory-routing.js';
import {createPlatformRuntime} from '../web/platform/runtime.js';
const catalog={};for(const name of ['products','accessories','firmwares'])catalog[name]=JSON.parse(await readFile(new URL(`../web/platform/catalog/${name}.json`,import.meta.url))).items;
const product=catalog.products.find(p=>p.id==='m5stack-c158');
const leaf=(id,catalogId,portId)=>({id,catalogId,parameters:{},wiring:{mode:'physical',viaAccessoryId:'hub',portId}});
const scene=()=>({...createDefaultScene(catalog,product.id),accessories:[{id:'hub',catalogId:'m5stack-unit-hub-u006',parameters:{},wiring:{mode:'physical',interfaceId:'grove'}},leaf('env','m5stack-unit-env4-u001d','port-1'),leaf('light','m5stack-unit-dlight-u136','port-2')]});
test('CoreMatrix shares GPIO0/1 across distinct I2C addresses and portable Hub topology',async()=>{
 const s=scene();assert.equal(validateScene(s,catalog).valid,true);assert.deepEqual(decodeScene(encodeScene(s)),s);
 assert.deepEqual(routedAccessories(s,product,catalog).map(r=>[r.accessoryId,r.pins,r.addresses]),[['hub',undefined,undefined],['env',[0,1],[0x44,0x76]],['light',[0,1],[0x23]]]);
 const runtime=createPlatformRuntime({catalog});try{await runtime.configure(s);const caps=(await runtime.capabilities()).input.accessories;assert.deepEqual(caps[1].pins,[0,1]);assert.deepEqual(caps[2].controls,['illuminance']);}finally{await runtime.dispose();}
});
test('CoreMatrix does not inherit StopWatch ADC/RMT/GPIO routes or conflicting Hub addresses',()=>{
 for(const id of ['m5stack-unit-button-u027','m5stack-unit-rgb-u003','m5stack-unit-angle-u005','m5stack-unit-pir-u004','m5stack-unit-env4-u001d']) {const s=scene();s.accessories[2].catalogId=id;assert.equal(validateScene(s,catalog).valid,false,id);}
 const s=scene();s.accessories[1].wiring.portId='port-2';assert.equal(validateScene(s,catalog).valid,false);
 const wrong=scene();wrong.accessories[0].wiring.interfaceId='usb';assert.equal(validateScene(wrong,catalog).valid,false);
 const direct=scene();direct.accessories=[{...direct.accessories[1],wiring:{mode:'physical',interfaceId:'grove'}}];assert.equal(validateScene(direct,catalog).valid,true);
});
