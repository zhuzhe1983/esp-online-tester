import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile,readdir} from 'node:fs/promises';
import {basename,join} from 'node:path';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDefaultScene} from '../web/platform/catalog.js';
import {chipIdForBoard,chipForBoard,isC6Board} from '../web/platform/app-image.js';
const read=async path=>JSON.parse(await readFile(new URL('../'+path,import.meta.url)));
const catalog=Object.fromEntries(await Promise.all(['products','accessories','firmwares'].map(async name=>[name,(await read(`web/platform/catalog/${name}.json`)).items])));
test('reviewed LilyGO input and SD capabilities agree with explicit implementations before any firmware loads',async()=>{
  const runtime=createPlatformRuntime({catalog,workerFactory:()=>{throw Error('Metadata checks must not invent guest execution');}});
  try{
    const files=(await readdir(new URL('../docs/platform/research/',import.meta.url))).filter(name=>/^lilygo-.*-profiles-2026-11-22\.json$/.test(name));
    const profiles=(await Promise.all(files.map(name=>read(`docs/platform/research/${name}`)))).flatMap(report=>report.items);
    for(const profile of profiles){
      const scene=createDefaultScene(catalog,profile.productId);scene.firmware=null;
      const state=await runtime.configure(scene),caps=await runtime.capabilities();
      const encoders=profile.controls.filter(control=>control.type==='encoder');
      assert.deepEqual(caps.input.encoders.map(control=>control.id),encoders.map(control=>control.id),profile.productId+' rotary inputs');
      for(const control of profile.controls.filter(control=>control.type==='button'))
        assert(caps.input.buttons.includes(control.id),profile.productId+' missing physical button '+control.id);
      assert.equal(caps.input.touch,profile.display?.touchImplemented===true,profile.productId+' touch subset');
      assert.equal(chipForBoard(profile.boardId),profile.chip,profile.productId+' exact chip family');
      if(profile.chip==='esp32c6')assert.equal(chipIdForBoard(profile.boardId),13,profile.productId+' image/ROM family');
      assert.equal(caps.sdCard.available,profile.sdCard?.implemented===true,profile.productId+' transport is separate from a physical slot');
      assert.equal(caps.lora.available,profile.radio?.implemented===true,profile.productId+' digital LoRa requires an explicit implemented radio');
      if(profile.sdCard?.slot){assert.equal(state.sdCard.slot,true);assert.equal(state.sdCard.bytes,64*1024*1024);assert.equal(state.sdCard.format,'fat32');}
      assert.equal(state.frame,null,'No guest framebuffer exists before firmware runs');
      if(profile.productId==='lilygo-t-deck-pro'){
        assert.deepEqual(caps.input.keyboard,{rows:4,columns:10,model:'tca8418',layout:'lilygo-t-deck-pro'});
        assert.equal(state.product.controls.filter(c=>c.type==='matrix-key').length,35);
      }
    }
  }finally{await runtime.dispose();}
});
test('STM32 keys, KEY5 and powered trackball directions use board events rather than bypassing their controllers',async()=>{
  const sent=[];
  const workerFactory=()=>{const worker={terminate(){},postMessage(message){
    sent.push(message);let reply;
    if(message.op==='init')reply={ready:true};
    else if(message.op==='create')reply={created:true,flashStorage:false,networkAvailable:false,audioAvailable:false};
    else if(message.op==='load')reply={loaded:message.kind,ok:true};
    else if(message.op==='start')reply={started:true};
    else if(message.op==='stop')reply={stopped:0};
    else if(message.op==='text')reply={inputQueued:message.inputId,time:0};
    else throw Error('Unexpected routing-test operation '+message.op);
    queueMicrotask(()=>worker.onmessage?.({data:reply}));
  }};return worker;};
  const origin='https://lilygo-input.invalid';
  const runtime=createPlatformRuntime({catalog,workerFactory,baseUrl:origin+'/platform/',assetBaseUrl:origin+'/',fetchImpl:async value=>{
    const url=new URL(value);assert.equal(url.origin,origin);
    if(url.pathname.endsWith('esp32sim.wasm'))return new Response(new Uint8Array([0]));
    try{return new Response(await readFile(new URL('../web'+url.pathname,import.meta.url)));}
    catch(error){
      // CI keeps mask ROMs outside the served web tree.
      if(error.code!=='ENOENT'||!process.env.ESP32SIM_ROM_DIR||!url.pathname.endsWith('_rom.elf'))throw error;
      return new Response(await readFile(join(process.env.ESP32SIM_ROM_DIR,basename(url.pathname))));
    }
  }});
  try{
    for(const [id,keys] of [
      ['lilygo-t-keyboard-s3-pro',['key1','key2','key3','key4','knob']],
      ['lilygo-t-track',['track-right','track-up','track-left','track-down']],
    ]){
      const scene=createDefaultScene(catalog,id);scene.firmware=null;await runtime.configure(scene);
      await runtime.loadFirmware({url:origin+'/wasm/fw/public/hello_world.bin',bootMode:'app-direct'});await runtime.start();
      const offset=sent.length;
      for(const key of keys)for(const pressed of [true,false])await runtime.input({type:'button',id:key,pressed});
      assert.deepEqual(sent.slice(offset).filter(message=>message.op==='text').map(message=>JSON.parse(message.data)),keys.flatMap(id=>[{t:'board-button',id,pressed:1},{t:'board-button',id,pressed:0}]));
      await assert.rejects(()=>runtime.input({type:'button',id:'invented-key',pressed:true}),{code:'UNSUPPORTED_CONTROL'});
      await runtime.pause();
    }
  }finally{await runtime.dispose();}
});
test('C6 boards with product names select C6 without reclassifying C61',()=>{
  for(const board of ['nesso-n1','lilygo-t-knob','lilygo-t7-c6','lilygo-t-qt-c6']){assert.equal(isC6Board(board),true);assert.equal(chipIdForBoard(board),13);}
  for(const board of ['esp32c61','corematrix']){assert.equal(isC6Board(board),false);assert.equal(chipIdForBoard(board),20);}
});
