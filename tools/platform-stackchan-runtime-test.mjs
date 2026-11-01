import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDefaultScene} from '../web/platform/catalog.js';
import {isPressControl} from '../web/platform/control-input.js';
import {validServoSnapshot} from '../web/platform/servo-state.js';

const catalog=Object.fromEntries(await Promise.all(['products','accessories','firmwares'].map(async name=>[name,JSON.parse(await readFile(new URL(`../web/platform/catalog/${name}.json`,import.meta.url))).items])));
const pads=['touch-pad-front','touch-pad-middle','touch-pad-back'];
const motion=()=>({t:'motion',supply:true,version:1,axes:[{id:'pan',positionDegrees:-30,targetDegrees:20,enabled:true,moving:true},{id:'tilt',positionDegrees:15,targetDegrees:15,enabled:true,moving:false}]});

// Structural worker deliberately executes no firmware; real execution uses probe-stackchan.mjs.
function fixture(){
  const workers=[],messages=[],events=[];
  const workerFactory=()=>{
    const w={terminate(){},postMessage(m){messages.push(m);let response;
      if(m.op==='init')response={ready:true};if(m.op==='create')response={created:true,flashStorage:false};
      if(m.op==='load')response={loaded:m.at===undefined?m.kind:`at${m.at}`,ok:true};
      if(m.op==='start')response={started:true};if(m.op==='resume')response={resumed:true};if(m.op==='stop')response={stopped:0};
      if(m.op==='text')response={inputQueued:m.inputId,time:1};
      if(response)queueMicrotask(()=>w.onmessage?.({data:response}));
    }};workers.push(w);return w;
  };
  const runtime=createPlatformRuntime({catalog,workerFactory,onEvent:e=>events.push(e),fetchImpl:async()=>new Response(new Uint8Array([1,2,3])),baseUrl:'https://fixture.invalid/platform/',assetBaseUrl:'https://fixture.invalid/'});
  const bytes=new Uint8Array(0x10100),view=new DataView(bytes.buffer);
  for(const at of [0,0x10000]){bytes[at]=0xe9;bytes[at+1]=1;bytes[at+12]=9;view.setUint32(at+28,4,true);}
  bytes[0x8000]=0xaa;bytes[0x8001]=0x50;view.setUint32(0x8004,0x10000,true);view.setUint32(0x8008,0x100000,true);
  return {runtime,messages,events,workers,load:()=>runtime.loadFirmware({file:new Blob([bytes])}),send:(value,w=workers.at(-1))=>w.onmessage({data:{text:JSON.stringify(value)}})};
}

test('K151 keeps factory-first selection and separates three touch zones from the screen and unsupported power controls',async()=>{
  const f=fixture();try{
    const scene=createDefaultScene(catalog,'m5stack-k151');await f.runtime.configure(scene);
    const p=catalog.products.find(p=>p.id===scene.productId),firmwares=catalog.firmwares.filter(f=>f.productIds.includes(p.id));
    assert.equal(firmwares[0].id,p.defaultFirmwareId);assert.equal(firmwares[0].role,'factory');
    assert.deepEqual(scene.keyMapping,{Digit1:pads[0],Digit2:pads[1],Digit3:pads[2]});
    assert.equal(isPressControl(p.controls.find(c=>c.id==='power')),false);
    const caps=await f.runtime.capabilities();assert.deepEqual(caps.input.buttons,pads);assert.equal(caps.input.touch,true);assert.equal(caps.servos.available,true);
    await f.load();await f.runtime.start();
    for(const id of pads)for(const pressed of [true,false]){
      const receipt=await f.runtime.input({type:'button',id,pressed});assert.deepEqual(receipt.message,{t:'board-button',id,pressed:+pressed});
    }
    const screen=await f.runtime.input({type:'touch',x:319,y:239,down:true});assert.deepEqual(screen.message,{t:'touch',x:319,y:239,down:1});
    await assert.rejects(f.runtime.input({type:'touch',x:320,y:240,down:true}),{code:'INVALID_INPUT'});
    await assert.rejects(f.runtime.input({type:'button',id:pads[0],pressed:1}),{code:'INVALID_INPUT'});
    await assert.rejects(f.runtime.input({type:'button',id:'power',pressed:true}),{code:'UNSUPPORTED_CONTROL'});
    await assert.rejects(f.runtime.input({type:'servo',id:'pan',positionDegrees:40}),{code:'UNSUPPORTED_INPUT'});
    await f.runtime.pause();await f.runtime.input({type:'button',id:pads[0],pressed:false});
    await assert.rejects(f.runtime.input({type:'button',id:pads[0],pressed:true}),{code:'NOT_RUNNING'});
  }finally{await f.runtime.dispose();}
});

test('servo feedback is immutable to observers, rejects malformed snapshots and cannot leak across resets or products',async()=>{
  const f=fixture();try{
    await f.runtime.configure(createDefaultScene(catalog,'m5stack-k151'));await f.load();await f.runtime.start();
    f.send(motion());const state=await f.runtime.observe();assert.deepEqual(state.servos,motion().axes);assert.equal(state.servoSupply,true);
    state.servos[0].positionDegrees=100;assert.equal((await f.runtime.observe()).servos[0].positionDegrees,-30);
    for(const change of [v=>v.axes.pop(),v=>v.axes[0].positionDegrees=null,v=>v.axes[0].id='tilt',v=>v.supply='on',v=>v.axes[1].moving=1]){
      const invalid=motion();change(invalid);assert.equal(validServoSnapshot(invalid),false);f.send(invalid);
    }
    assert.equal((await f.runtime.observe()).servos[0].positionDegrees,-30);
    assert.equal(f.events.filter(e=>e.type==='servos'&&e.servos.length).length,1);
    const old=f.workers.at(-1);await f.runtime.reset();assert.deepEqual((await f.runtime.observe()).servos,[]);f.send(motion(),old);assert.deepEqual((await f.runtime.observe()).servos,[]);
    f.send(motion());await f.runtime.configure(createDefaultScene(catalog,'m5stack-k134'));assert.deepEqual((await f.runtime.observe()).servos,[]);assert.equal((await f.runtime.capabilities()).servos.available,false);
    await f.load();f.send(motion());assert.deepEqual((await f.runtime.observe()).servos,[]);
  }finally{await f.runtime.dispose();}
});
