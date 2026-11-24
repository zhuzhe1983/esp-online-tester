import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {firmwareRunPolicy} from '../web/platform/firmware-policy.js';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDefaultScene} from '../web/platform/catalog.js';

test('C5 exact factory images remain first/default unverified trials and report the current ROM loader boundary',async()=>{
 const catalog={};for(const k of ['products','accessories','firmwares'])catalog[k]=JSON.parse(await readFile(new URL(`../web/platform/catalog/${k}.json`,import.meta.url))).items;
 const factoryIds=new Set(['waveshare-c5-lcd147-factory','waveshare-c5-touch169-factory','waveshare-c5-touch28-factory','waveshare-c5-touch35-factory','waveshare-c5-lcd273-factory']);
 const products=catalog.products.filter(p=>factoryIds.has(p.defaultFirmwareId));
 const runtime=createPlatformRuntime({catalog,fetchImpl:()=>{throw Error('Known unsupported ROM mode must fail before a download');}});
 try{for(const product of products){
  const factory=catalog.firmwares.find(f=>f.id===product.defaultFirmwareId);assert.equal(factory.role,'factory');assert.equal(factory.status,'trial');assert.equal(factory.simulationVerified,false);assert.equal(factory.hardwareValidated,false);assert.equal(factory.source.bootMode,'rom');assert.match(factory.source.sha256,/^[a-f0-9]{64}$/);
  assert.equal(firmwareRunPolicy(product,factory).canRun,true);
  const own=catalog.firmwares.filter(f=>f.productIds?.includes(product.id));assert.equal(own[0].id,factory.id);
  await runtime.configure(createDefaultScene(catalog,product.id));
  await assert.rejects(runtime.loadFirmware(factory.source),{code:'C5_SRAM_REQUIRED'});
 }}finally{await runtime.dispose();}
});
