import test from 'node:test';
import assert from 'node:assert/strict';
import {manufacturerNames,productsForManufacturer,searchProducts,firmwareGroups,firstFirmware,filterAccessories,searchAccessories} from '../web/platform/catalog-selection.js';
const products=[{id:'m10',manufacturer:'M5Stack',name:'M5Stack Device 10',sku:'C010',chip:'esp32s3'},{id:'m2',manufacturer:'M5Stack',name:'M5Stack Device 2',sku:'C002',chip:'esp32s3'},{id:'a',manufacturer:'Arduino',name:'Arduino Nesso N1',sku:'DK001',chip:'esp32c6'}];
test('manufacturer selection filters while device names use natural order',()=>{
  assert.deepEqual(manufacturerNames(products),['Arduino','M5Stack']);
  assert.deepEqual(productsForManufacturer(products,'M5Stack').map(p=>p.id),['m2','m10']);
  assert.deepEqual(productsForManufacturer(products,'Arduino').map(p=>p.id),['a']);
  assert.equal(products[0].id,'m10','sorting does not mutate provenance catalog order');
});
test('compact search can find a SKU outside the currently selected manufacturer',()=>{
  assert.deepEqual(searchProducts(products,'dk001').map(p=>p.id),['a']);
  assert.deepEqual(searchProducts(products,'M5Stack Ｃ００２').map(p=>p.id),['m2']);
  assert.deepEqual(searchProducts(products,'arduino c6').map(p=>p.id),['a']);
  assert.equal(searchProducts(products,'missing').length,0);
});
test('firmware groups preserve separate vendor distribution and upstream versions',()=>{
  const entries=[{id:'upstream',productIds:['m2'],publisherGroup:'CrossPoint',revision:'1.6.5'},{id:'vendor',productIds:['m2'],publisherGroup:'官方',revision:'1.5.0'},{id:'other',productIds:['a'],publisher:'Other'}];
  assert.deepEqual(firmwareGroups(entries,'m2').map(([name,items])=>[name,items[0].id]),[['官方','vendor'],['CrossPoint','upstream']]);
});
test('factory firmware leads the menu and is selected even before compatibility verification',()=>{
  const entries=[{id:'community',productIds:['m2'],publisher:'CrossPoint',source:{url:'https://example.org/community.bin'}},{id:'official-alternative',productIds:['m2'],publisher:'M5Stack',role:'alternative'},{id:'factory',productIds:['m2'],publisher:'M5Stack',role:'factory',status:'trial',simulationVerified:false,source:{url:'https://example.org/factory.bin'}}];
  assert.equal(firstFirmware(entries,{id:'m2',defaultFirmwareId:'community'}).id,'factory');
  assert.equal(firmwareGroups(entries,'m2')[0][1][0].id,'factory');
  assert.equal(entries[0].id,'community');
  assert.equal(firstFirmware(entries,{id:'absent'}),null);
});
test('accessory filters combine brand and connector; search crosses filters and includes local definitions',()=>{
  const entries=[{id:'gps',name:'GPS',sku:'U032',manufacturer:'M5Stack',connector:{type:'HY2.0-4P'}},{id:'other',name:'Other sensor',manufacturer:'Acme',connector:{type:'USB-C'}},{id:'local:knob',name:'Private knob',connector:{type:'custom'}}];
  assert.deepEqual(filterAccessories(entries,{brand:'Acme',connector:'USB-C'}).map(x=>x.id),['other']);
  assert.equal(filterAccessories(entries,{brand:'M5Stack',connector:'USB-C'}).length,0);
  assert.deepEqual(searchAccessories(entries,'Ｕ０３２').map(x=>x.id),['gps']);
  assert.deepEqual(searchAccessories(entries,'diy knob').map(x=>x.id),['local:knob']);
});
