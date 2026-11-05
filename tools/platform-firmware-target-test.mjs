import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {firmwareTargetId,validateFirmwareTargets,firmwareTargetProduct,firmwareForProduct,firmwaresForProduct} from '../web/platform/firmware-target.js';
import {firmwareGroups,firstFirmware} from '../web/platform/catalog-selection.js';
import {firmwareRunPolicy} from '../web/platform/firmware-policy.js';

const base={id:'base',chip:'esp32s3',runtime:{board:'base-board',flashMb:16,psramMb:8}};
const sibling={id:'sibling',chip:'esp32s3',firmwareTargetId:'base',runtime:{board:null,flashMb:32,psramMb:8}};
const originalFirmware={id:'base-factory',name:'Base factory firmware',productIds:['base'],publisher:'M5Stack',role:'factory',status:'available',simulationVerified:true,hardwareValidated:true,source:{url:'https://example.org/base.bin',sha256:'a'.repeat(64)},validation:{passed:true},validationEvidence:{productId:'base',frames:4}};
const readJson=async path=>JSON.parse(await readFile(new URL(path,import.meta.url),'utf8'));

test('omitted and explicit self targets preserve direct product behavior',()=>{
  assert.equal(firmwareTargetId(base),'base');
  assert.equal(firmwareTargetId({...base,firmwareTargetId:'base'}),'base');
  assert.doesNotThrow(()=>validateFirmwareTargets([base,{id:'other',chip:'esp32c6',firmwareTargetId:'other'}]));
  assert.strictEqual(firmwareTargetProduct(base,[base,sibling]),base);
  assert.strictEqual(firmwareForProduct(originalFirmware,base),originalFirmware);
  assert.equal(firmwareForProduct(originalFirmware,{id:'other',chip:'esp32s3'}),null,'Sharing a chip alone does not create a firmware mapping');
  assert.deepEqual(firmwaresForProduct([originalFirmware],base),[originalFirmware]);
  assert.equal(firmwareRunPolicy(base,firmwareForProduct(originalFirmware,base)).trial,false);
});

test('shared targets preserve artifact identity but not factory or validation claims',()=>{
  const source=Object.freeze(structuredClone(originalFirmware)),before=structuredClone(source);
  const productBefore=structuredClone(sibling);
  assert.doesNotThrow(()=>validateFirmwareTargets([base,sibling]));
  assert.strictEqual(firmwareTargetProduct(sibling,[base,sibling]),base);
  const shared=firmwareForProduct(source,sibling);
  assert.notStrictEqual(shared,source);
  assert.equal(shared.id,source.id);assert.deepEqual(shared.source,source.source);
  assert.deepEqual(shared.productIds,['base'],'A browsing relation must not rewrite exact product mappings');
  assert.equal(shared.sharedTargetId,'base');assert.equal(shared.status,'trial');
  assert.equal(shared.simulationVerified,false);assert.equal(shared.hardwareValidated,false);
  for(const key of ['role','validation','validationEvidence'])assert.equal(Object.hasOwn(shared,key),false,`${key} belongs to the directly mapped product`);
  assert.deepEqual(source,before,'Projecting a shared item must not change the canonical catalog entry');
  assert.deepEqual(sibling,productBefore,'Looking up firmware must not inherit target hardware properties');
  const policy=firmwareRunPolicy(sibling,shared);
  assert.equal(policy.trial,true);assert.equal(policy.canRun,false,'A target pointer does not provide a board model');
  assert.equal(firmwareRunPolicy({...sibling,runtime:{...sibling.runtime,board:'candidate-board'}},shared).trial,true);
});

test('shared documentation without an image stays reference-only',()=>{
  const reference={...originalFirmware,source:null,status:'reference'};
  const shared=firmwareForProduct(reference,sibling);
  assert.equal(shared.status,'reference');assert.equal(shared.source,null);
  assert.equal(shared.simulationVerified,false);assert.equal(shared.hardwareValidated,false);
  assert.equal(firmwareRunPolicy({...sibling,runtime:{board:'candidate-board'}},shared).canRun,false);
});

test('an exact product mapping wins even when the product also declares a shared target',()=>{
  const direct={...originalFirmware,id:'direct',productIds:['base','sibling']};
  assert.strictEqual(firmwareForProduct(direct,sibling),direct);
  assert.equal(firmwareForProduct(direct,sibling).sharedTargetId,undefined);
  const unrelated={...originalFirmware,id:'unrelated',productIds:['other']};
  const choices=firmwaresForProduct([originalFirmware,direct,unrelated],sibling);
  assert.deepEqual(new Set(choices.map(item=>item.id)),new Set(['base-factory','direct']));
  assert.equal(choices.filter(item=>item.id==='direct').length,1,'An entry matching both direct and shared IDs is not duplicated');
  assert.strictEqual(choices.find(item=>item.id==='direct'),direct);
});

test('old string selection remains scoped while product selection includes shared trials and prefers direct firmware',()=>{
  const direct={id:'own-application',name:'Sibling app',productIds:['sibling'],publisher:'Community',status:'trial',source:{url:'https://example.org/sibling.bin'}};
  const entries=[originalFirmware,direct];
  assert.deepEqual(firmwareGroups(entries,'base').flatMap(([,items])=>items.map(item=>item.id)),['base-factory']);
  assert.deepEqual(firmwareGroups(entries,'sibling').flatMap(([,items])=>items.map(item=>item.id)),['own-application']);
  const shared=firmwareGroups(entries,sibling).flatMap(([,items])=>items).find(item=>item.id==='base-factory');
  assert.ok(shared);assert.equal(shared.simulationVerified,false);assert.equal(shared.role,undefined);
  assert.equal(firmwaresForProduct([originalFirmware,{...direct,source:originalFirmware.source}],sibling)[0].id,'own-application','Source restoration must prefer a direct entry even when the URL/hash is shared');
  assert.equal(firstFirmware(entries,sibling).id,'own-application','Direct firmware takes priority over a canonical target factory entry');
  assert.equal(firstFirmware([originalFirmware],sibling).sharedTargetId,'base');
  assert.equal(firstFirmware(entries,{id:'absent',chip:'esp32s3'}),null);
});

test('invalid, missing, cross-chip and chained mappings fail as catalog errors',()=>{
  for(const target of ['', ' ', null, 4, {}, [], 'https://example.org/base', '../base', '__proto__']){
    assert.throws(()=>validateFirmwareTargets([base,{...sibling,firmwareTargetId:target}]),{code:'CATALOG_SCHEMA'},`Invalid target ${JSON.stringify(target)}`);
  }
  assert.throws(()=>validateFirmwareTargets([base,{...sibling,firmwareTargetId:'missing'}]),{code:'CATALOG_SCHEMA'});
  assert.throws(()=>validateFirmwareTargets([base,{...sibling,chip:'esp32c6'}]),{code:'CATALOG_SCHEMA'});
  assert.throws(()=>validateFirmwareTargets([base,sibling,{id:'third',chip:'esp32s3',firmwareTargetId:'sibling'}]),{code:'CATALOG_SCHEMA'},'A target must be canonical, not another alias');
  assert.throws(()=>validateFirmwareTargets([{...base,firmwareTargetId:'sibling'},sibling]),{code:'CATALOG_SCHEMA'},'Mutual aliases must not form a cycle');
});

test('TriviaPOD shares StopWatch firmware discovery while keeping its own hardware and support state',async()=>{
  const products=(await readJson('../web/platform/catalog/products.json')).items;
  const firmwares=(await readJson('../web/platform/catalog/firmwares.json')).items;
  assert.doesNotThrow(()=>validateFirmwareTargets(products));
  const trivia=products.find(product=>product.id==='m5stack-triviapod');
  const watch=products.find(product=>product.id==='m5stack-stopwatch');
  const before=structuredClone(trivia);
  assert.equal(firmwareTargetId(trivia),watch.id);
  assert.strictEqual(firmwareTargetProduct(trivia,products),watch);
  const expected=firmwares.filter(firmware=>firmware.productIds.includes(watch.id));
  const shared=firmwaresForProduct(firmwares,trivia).filter(firmware=>firmware.sharedTargetId===watch.id);
  assert.ok(expected.length>0);assert.deepEqual(new Set(shared.map(firmware=>firmware.id)),new Set(expected.map(firmware=>firmware.id)));
  for(const firmware of shared){assert.equal(firmware.simulationVerified,false);assert.equal(firmware.role,undefined);assert.equal(firmwareRunPolicy(trivia,firmware).canRun,true);assert.equal(firmwareRunPolicy(trivia,firmware).trial,true);}
  assert.deepEqual(trivia,before);
  assert.equal(trivia.id,'m5stack-triviapod');assert.equal(trivia.runtime.flashMb,32);assert.equal(watch.runtime.flashMb,16);
  assert.equal(trivia.runtime.board,'stopwatch');assert.equal(trivia.runtime.status,'partial');
  assert.equal(trivia.visualVariantOf,undefined);
  assert.deepEqual(trivia.interfaces.filter(port=>port.external).map(port=>port.type),['USB-C']);
  assert.ok(watch.interfaces.some(port=>port.external&&port.type==='HY2.0-4P'));
});
