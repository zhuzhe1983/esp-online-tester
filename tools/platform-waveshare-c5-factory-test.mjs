import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {firmwareRunPolicy} from '../web/platform/firmware-policy.js';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDefaultScene} from '../web/platform/catalog.js';

test('C5 factory images stay unverified trials and ROM loading reaches the actual source download boundary',async()=>{
 const catalog={};for(const k of ['products','accessories','firmwares'])catalog[k]=JSON.parse(await readFile(new URL(`../web/platform/catalog/${k}.json`,import.meta.url))).items;
 const factoryIds=new Set(['waveshare-c5-lcd147-factory','waveshare-c5-touch169-factory','waveshare-c5-touch28-factory','waveshare-c5-touch35-factory','waveshare-c5-lcd273-factory']);
 const products=catalog.products.filter(p=>factoryIds.has(p.defaultFirmwareId));
 const requests=[];
 const runtime=createPlatformRuntime({catalog,fetchImpl:url=>{requests.push(String(url));throw Error('Deliberate fixture download failure; no firmware bytes supplied');}});
 try{for(const product of products){
  const factory=catalog.firmwares.find(f=>f.id===product.defaultFirmwareId);assert.equal(factory.role,'factory');assert.equal(factory.status,'trial');assert.equal(factory.simulationVerified,false);assert.equal(factory.hardwareValidated,false);assert.equal(factory.source.bootMode,'rom');assert.match(factory.source.sha256,/^[a-f0-9]{64}$/);
  assert.equal(firmwareRunPolicy(product,factory).canRun,true);
  const own=catalog.firmwares.filter(f=>f.productIds?.includes(product.id));assert.equal(own[0].id,factory.id);
  await runtime.configure(createDefaultScene(catalog,product.id));
  const caps=await runtime.capabilities();assert.deepEqual(caps.execution.bootModes,['rom','app-direct']);assert.equal(caps.execution.romBoot,true);
  const before=requests.length;
  await assert.rejects(runtime.loadFirmware(factory.source),{code:'FETCH_FAILED'});
  assert.equal(requests.length,before+1);assert.equal(requests[before],factory.source.url); // A download attempt is not factory execution or acceptance.
 }}finally{await runtime.dispose();}
});
