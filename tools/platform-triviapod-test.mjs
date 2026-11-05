import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createDefaultScene,validateScene,encodeScene,decodeScene} from '../web/platform/catalog.js';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {accessoryChoicesForProduct} from '../web/platform/accessory-selection.js';
import {firstFirmware} from '../web/platform/catalog-selection.js';
import {firmwareRunPolicy,firmwareFlashPolicy} from '../web/platform/firmware-policy.js';
import {deviceStorageScope} from '../web/platform/device-storage.js';
const catalog={};for(const name of ['products','accessories','firmwares'])catalog[name]=JSON.parse(await readFile(new URL(`../web/platform/catalog/${name}.json`,import.meta.url))).items;
const trivia=catalog.products.find(item=>item.id==='m5stack-triviapod'),watch=catalog.products.find(item=>item.id==='m5stack-stopwatch');

test('TriviaPOD offers shared firmware for simulation with its own capacity and persistence identity',()=>{
  const firmware=firstFirmware(catalog.firmwares,trivia),scene=createDefaultScene(catalog,trivia.id);
  assert.equal(firmware.id,'m5stack-stopwatch-official');assert.equal(firmwareRunPolicy(trivia,firmware).canRun,true);
  assert.equal(firmwareRunPolicy(trivia,firmware).trial,true);assert.equal(firmwareFlashPolicy(trivia,firmware.source,firmware).canFlash,false);
  scene.firmware=firmware.source;assert.equal(decodeScene(encodeScene(scene,catalog),catalog).productId,trivia.id);
  const scope=p=>deviceStorageScope({productId:p.id,board:p.runtime.board,chip:p.chip,flashBytes:p.runtime.flashMb*1024*1024,firmwareSha256:firmware.source.sha256});
  assert.equal(scope(trivia).flashBytes,32*1024*1024);assert.equal(scope(watch).flashBytes,16*1024*1024);assert.notEqual(scope(trivia).key,scope(watch).key);
});

test('the compatibility board exposes touch and buttons without inheriting physical accessories or the GPS route',async()=>{
  const selected=accessoryChoicesForProduct(trivia,catalog.accessories);
  assert.equal(selected.connector,'USB-C');assert.deepEqual(selected.interfaces.map(item=>item.type),['USB-C']);
  for(const accessory of catalog.accessories.filter(item=>item.connector?.type==='HY2.0-4P')) {
    const scene=createDefaultScene(catalog,trivia.id);scene.accessories=[{id:'unit',catalogId:accessory.id,parameters:accessory.parameters||{},wiring:{mode:'physical',interfaceId:'grove'}}];
    assert.equal(validateScene(scene,catalog).valid,false,accessory.id);
  }
  const runtime=createPlatformRuntime({catalog});
  try {
    await runtime.configure(createDefaultScene(catalog,trivia.id));const caps=await runtime.capabilities();
    assert.equal(caps.executable,true);assert.equal(caps.input.touch,true);assert.deepEqual(caps.input.buttons,['button-a','button-b']);
    assert.deepEqual(caps.input.accessories,[]);assert.equal(caps.gps.available,false);assert.equal(caps.gps.mode,null);
    const scene=createDefaultScene(catalog,trivia.id);scene.accessories=[{id:'gps',catalogId:'m5stack-gps-u032',parameters:{},wiring:{mode:'logical',port:'uart1'}}];
    await runtime.configure(scene);await assert.rejects(()=>runtime.injectGps({accessoryId:'gps'}),{code:'UNSUPPORTED_WIRING'});
    await runtime.configure(createDefaultScene(catalog,watch.id));assert.equal((await runtime.capabilities()).gps.available,true);
  } finally {await runtime.dispose();}
});
