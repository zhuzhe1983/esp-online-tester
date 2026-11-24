// Public input protocol regression. The transport stand-in is explicitly not
// guest execution evidence; it checks the real runtime's semantic conversion.
import assert from 'node:assert/strict';
import test from 'node:test';
import {readFile} from 'node:fs/promises';
import {join} from 'node:path';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDefaultScene} from '../web/platform/catalog.js';

test('physical active-high keys press HIGH and release LOW, including paused release; pull-up defaults remain unchanged',async()=>{
  const root=new URL('../web/',import.meta.url),origin='https://button-polarity.invalid';
  const catalog=Object.fromEntries(await Promise.all(['products','accessories','firmwares'].map(async n=>[n,JSON.parse(await readFile(new URL(`platform/catalog/${n}.json`,root))).items])));
  // Use the established S3 demo loader only as a transport fixture. These two
  // explicit fixture controls are not claims about the demo board's hardware.
  const product=structuredClone(catalog.products.find(p=>p.id==='m5stack-stopwatch'));
  product.id='polarity-transport-fixture';product.controls=[
    {id:'pull-up',type:'button',pin:0}, {id:'pull-down',type:'button',pin:10,activeLow:false},
  ];catalog.products.push(product);
  const messages=[],transport={postMessage(m){
    let reply;
    if(m.op==='init')reply={ready:true};else if(m.op==='create')reply={created:true};
    else if(m.op==='load')reply={loaded:m.kind,ok:true};else if(m.op==='start')reply={started:true};
    else if(m.op==='stop')reply={stopped:true};else if(m.op==='imu')reply={imuApplied:m.inputId,ok:true,time:0};
    else if(m.op==='text'){messages.push(JSON.parse(m.data));reply={inputQueued:m.inputId,time:.25};}
    if(reply)queueMicrotask(()=>this.onmessage?.({data:reply}));
  },terminate(){}};
  const runtime=createPlatformRuntime({catalog,workerFactory:()=>transport,baseUrl:origin+'/platform/',assetBaseUrl:origin+'/',fetchImpl:async value=>{
    const url=new URL(value,origin);assert.equal(url.origin,origin);
    if(process.env.ESP32SIM_ROM_DIR&&url.pathname==='/wasm/fw/esp32s3_rev0_rom.elf')return new Response(await readFile(join(process.env.ESP32SIM_ROM_DIR,'esp32s3_rev0_rom.elf')));
    return new Response(await readFile(new URL(url.pathname.slice(1),root)));
  }});
  try{
    await runtime.configure(createDefaultScene(catalog,product.id));
    await runtime.loadFirmware({manifestUrl:origin+'/wasm/fw/demo-stopwatch.json'});await runtime.start();
    for(const id of ['pull-up','pull-down'])for(const pressed of [true,false])await runtime.input({type:'button',id,pressed});
    assert.deepEqual(messages,[{t:'btn',pin:0,v:1},{t:'btn',pin:0,v:0},{t:'btn',pin:10,v:0},{t:'btn',pin:10,v:1}]);
    await runtime.input({type:'button',id:'pull-down',pressed:true});await runtime.pause();
    await runtime.input({type:'button',id:'pull-down',pressed:false});
    assert.deepEqual(messages.at(-1),{t:'btn',pin:10,v:1});
    await assert.rejects(()=>runtime.input({type:'button',id:'pull-down',pressed:true}),{code:'NOT_RUNNING'});
  }finally{await runtime.dispose();}
});
