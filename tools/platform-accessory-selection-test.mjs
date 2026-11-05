import test from 'node:test';
import assert from 'node:assert/strict';
import {accessoryChoicesForProduct,productAccessoryInterfaces,accessorySearchPreference,ALL_ACCESSORY_INTERFACES} from '../web/platform/accessory-selection.js';

const product={id:'host',runtime:{board:null},interfaces:[
  {id:'usb',type:'USB-C',name:'USB-C',external:true},
  {id:'a',type:'HY2.0-4P',name:'Port A',external:true},
  {id:'b',type:'HY2.0-4P',name:'Port B',external:true},
  {id:'internal',type:'I2C',external:false},{id:'unverified',type:'GPIO'},
]};
const items=[{id:'gps',name:'GPS',manufacturer:'M5Stack',connector:{type:'HY2.0-4P'}},
  {id:'local:light',name:'DIY Light',manufacturer:'DIY',connector:{type:'HY2.0-4P'}},
  {id:'camera',name:'Camera',manufacturer:'Other',connector:{type:'FPC'}}];

test('uses documented external connector types and groups duplicate sockets without inferring routing',()=>{
  const ports=productAccessoryInterfaces(product);
  assert.deepEqual(ports.map(p=>p.type),['USB-C','HY2.0-4P']);
  assert.equal(ports[1].label,'HY2.0-4P · Port A / Port B');
  const selected=accessoryChoicesForProduct(product,items);
  assert.equal(selected.connector,'HY2.0-4P','prefer a type with catalog choices even when the board runtime is absent');
  assert.equal(selected.choices.every(item=>item.connector.type==='HY2.0-4P'),true);
});

test('switching to a USB-only product drops stale Grove/brand/accessory selections',()=>{
  const selected=accessoryChoicesForProduct({interfaces:[product.interfaces[0]]},items,{connector:'HY2.0-4P',brand:'M5Stack',id:'gps'});
  assert.equal(selected.connector,'USB-C');assert.deepEqual(selected.choices,[]);assert.equal(selected.id,'');assert.equal(selected.brands.length,0);
});

test('empty and internal-only records remain unknown instead of defaulting to Grove',()=>{
  for(const p of [{interfaces:[]},{interfaces:[{type:'HY2.0-4P',external:false}]},{}]){
    const selected=accessoryChoicesForProduct(p,items);
    assert.equal(selected.connector,'');assert.deepEqual(selected.choices,[]);assert.match(selected.notice,/待核对/);
  }
});

test('external GPIO signal maps and unspecified USB versions do not establish connector shape',()=>{
  const p={interfaces:[{type:'GPIO',external:true,name:'排针 I2C 信号（非接插件针序）'},{type:'USB',external:true,name:'接口批次待核'},{type:'SDMMC',external:true}]};
  const selected=accessoryChoicesForProduct(p,[...items,{id:'private-gpio',connector:{type:'GPIO'}}]);
  assert.deepEqual(selected.interfaces,[]);assert.deepEqual(selected.choices,[]);assert.equal(selected.id,'');
  assert.deepEqual(productAccessoryInterfaces({interfaces:[{type:'pin-header',external:true}]}).map(p=>p.type),['pin-header']);
});

test('matching DIY selections remain available and explicit all-interface browsing stays separate',()=>{
  const selected=accessoryChoicesForProduct(product,items,{connector:'HY2.0-4P',brand:'DIY',id:'local:light'});
  assert.equal(selected.id,'local:light');assert.equal(selected.brand,'DIY');
  const preference=accessorySearchPreference(product,items[2]);assert.equal(preference.browseAll,true);
  const browsed=accessoryChoicesForProduct(product,items,preference);
  assert.equal(browsed.connector,ALL_ACCESSORY_INTERFACES);assert.equal(browsed.id,'camera');assert.match(browsed.notice,/未按当前设备接口筛选/);
  const reset=accessoryChoicesForProduct(product,items);assert.notEqual(reset.connector,ALL_ACCESSORY_INTERFACES);
});

test('solder pads and superficially similar four-pin sockets are not interchangeable',()=>{
  const p={interfaces:[{type:'FPC-pad',external:true,name:'FPC 焊盘（未焊插座）'},{type:'JST-SH-4P',external:true,name:'Qwiic'}]};
  const selected=accessoryChoicesForProduct(p,items);
  assert.deepEqual(selected.choices,[]);assert.equal(accessorySearchPreference(p,items[0]).browseAll,true);
});
