#!/usr/bin/env node
// Real WASM ABI + I2S/GDMA loopback. No ROM, network download or fabricated audio output.
// ESP32SIM_WASM=/path/to/esp32sim.wasm selects an integrated build from another checkout.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createJitHost} from '../web/wasm/jit.mjs';

const bytes=await readFile(process.env.ESP32SIM_WASM||new URL('../web/wasm/esp32sim.wasm',import.meta.url));
const encoder=new TextEncoder(),decoder=new TextDecoder(),logs=[];
let wasm;
const jit=createJitHost(()=>wasm),memory=()=>new Uint8Array(wasm.memory.buffer);
const loaded=await WebAssembly.instantiate(bytes,{env:{...jit.imports,host_profile_now:()=>performance.now(),host_log:(p,n)=>logs.push(decoder.decode(memory().subarray(p,p+n)))}});
wasm=loaded.instance.exports;
for(const name of ['input_configure','input_push','input_clear','input_disable','input_state','state_ptr','output_streaming','output_flush'])assert.equal(typeof wasm[`esp32sim_audio_${name}`],'function',`Missing audio export ${name}; rebuild WASM.`);
const withBytes=(data,fn)=>{const p=wasm.esp32sim_alloc(data.length);try{memory().set(data,p);return fn(p,data.length);}finally{wasm.esp32sim_free(p,data.length);}};
const state=(e,port=0)=>{const n=wasm.esp32sim_audio_input_state(e,port),p=wasm.esp32sim_audio_state_ptr(e);return n?JSON.parse(decoder.decode(memory().subarray(p,p+n))):null;};
const pcm=samples=>{const bytes=new Uint8Array(samples.length*2),view=new DataView(bytes.buffer);samples.forEach((s,i)=>view.setInt16(i*2,s,true));return bytes;};
const push=(e,samples,channels=1,port=0)=>withBytes(pcm(samples),(p,n)=>wasm.esp32sim_audio_input_push(e,port,channels,p,n));
const create=(board,fn)=>{
  const e=withBytes(encoder.encode(board),(p,n)=>wasm.esp32sim_new(p,n,4,0));assert.notEqual(e,0,board);
  try{return fn(e);}finally{wasm.esp32sim_delete(e);}
};

create('none',e=>{
  assert.equal(state(e).configured,false);
  assert.equal(push(e,[11]),-1,'input requires explicit configuration');
  assert.equal(wasm.esp32sim_audio_input_configure(e,0,7999),1);
  assert.equal(wasm.esp32sim_audio_input_configure(e,0,96001),1);
  assert.equal(wasm.esp32sim_audio_input_configure(e,2,8000),1);
  assert.equal(wasm.esp32sim_audio_input_configure(e,0,8000),0);
  assert.equal(withBytes(new Uint8Array(3),(p,n)=>wasm.esp32sim_audio_input_push(e,0,2,p,n)),-1,'partial stereo frame rejects');
  assert.equal(withBytes(new Uint8Array(2),(p,n)=>wasm.esp32sim_audio_input_push(e,0,3,p,n)),-1,'invalid channel count rejects');
  assert.equal(withBytes(new Uint8Array(32770),(p,n)=>wasm.esp32sim_audio_input_push(e,0,1,p,n)),-1,'oversized block rejects');
  assert.equal(wasm.esp32sim_audio_input_push(e,0,1,0,2),-1,'null nonempty input rejects');
  assert.equal(state(e).queuedFrames,0,'malformed calls must not enqueue data');
  assert.equal(push(e,Array(16384).fill(1234)),16000);
  assert.equal(push(e,[-777]),0);
  assert.deepEqual({...state(e)},{configured:true,port:0,sampleRate:8000,queuedFrames:16000,capacityFrames:16000,pushedFrames:16000,consumedFrames:0,underrunFrames:0,rejectedFrames:385,clearedFrames:0});
  assert.equal(wasm.esp32sim_audio_input_clear(e,0),0);
  assert.equal(state(e).queuedFrames,0);assert.equal(state(e).clearedFrames,16000);
  assert.equal(wasm.esp32sim_audio_input_disable(e,0),0);
  assert.equal(state(e).configured,false);assert.equal(push(e,[11]),-1);
});

for(const [board,ports,output] of [['none',2,0],['esp32',2,1],['esp32c3',1,1],['esp32c6',1,1],['esp32p4',0,1]])create(board,e=>{
  for(let port=0;port<3;port++)assert.equal(wasm.esp32sim_audio_input_configure(e,port,48000),port<ports?0:1,`${board} port ${port}`);
  assert.equal(wasm.esp32sim_audio_output_streaming(e,1),output,`${board} output capability`);
  assert.equal(wasm.esp32sim_audio_output_streaming(e,2),1);
});

function busyLoopImage() {
  // Xtensa `j .`, identical to tools/wasm-test.mjs's app-direct fixture, executes real instructions.
  const app=new Uint8Array(35),view=new DataView(app.buffer),entry=0x40370000;
  app[0]=0xe9;app[1]=1;view.setUint32(4,entry,true);view.setUint32(24,entry,true);view.setUint32(28,3,true);app.set([0x06,0xff,0xff],32);return app;
}

function loopback(channels,sourceRate,samples,expected) {
  create('none',e=>{
    assert.equal(wasm.esp32sim_audio_output_streaming(e,1),0);
    assert.equal(wasm.esp32sim_audio_input_configure(e,0,sourceRate),0);
    assert.equal(push(e,samples,channels),samples.length/channels);
    const script=[],poke=(t,a,v)=>script.push(`${t} poke ${a.toString(16)} ${(v>>>0).toString(16)}`);
    // ESP-IDF v5.5.5 components/soc/esp32s3/register/soc/{i2s_reg.h,gdma_reg.h}:
    // I2S RX/TX clock and TDM fields, GDMA channel 0.
    // Existing core fixtures: cli/tests/i2s_rx.rs. Scripted register writes configure real
    // peripherals; host PCM must pass RX -> GDMA -> guest SRAM -> GDMA -> TX -> binary outbox.
    const clock=(1<<26)|(2<<27)|25,format=(24<<7)|(15<<13)|(15<<18),size=expected.length*4;
    for(const [a,v] of [[0x6000f030,clock],[0x6000f028,format],[0x6000f050,(1<<16)|3],[0x6000f064,size],
      [0x3fc90000,size|(1<<31)],[0x3fc90004,0x3fc90100],[0x3fc90008,0],
      [0x6003f048,3],[0x6003f020,0x90000|(1<<22)],[0x6000f020,4]])poke(0,a,v);
    poke(0.002,0x6000f020,0);
    for(const [a,v] of [[0x6000f034,clock],[0x6000f02c,format],[0x6000f054,(1<<16)|3],
      [0x3fc90020,size|(size<<12)|(1<<30)|(1<<31)],[0x3fc90024,0x3fc90100],[0x3fc90028,0],
      [0x6003f0a8,3],[0x6003f080,0x90020|(1<<21)],[0x6000f024,4]])poke(0.003,a,v);
    assert.equal(withBytes(busyLoopImage(),(p,n)=>wasm.esp32sim_load(e,3,p,n)),0);
    assert.equal(withBytes(encoder.encode(script.join('\n')),(p,n)=>wasm.esp32sim_load(e,6,p,n)),0);
    wasm.esp32sim_set_jit(e,0);assert.equal(wasm.esp32sim_boot(e,1),0);
    const output=[],rates=[];
    const drain=()=>{
      const count=wasm.esp32sim_out_take(e);
      for(let i=0;i<count;i++) {
        const kind=wasm.esp32sim_out_kind(e,i),p=wasm.esp32sim_out_ptr(e,i),n=wasm.esp32sim_out_len(e,i);
        const b=memory().slice(p,p+n);if(kind!==2||b[0]!==2)continue;
        const view=new DataView(b.buffer);assert.equal((b.length-5)%2,0);rates.push(view.getUint32(1,true));
        for(let at=5;at<b.length;at+=2)output.push(view.getInt16(at,true));
      }
    };
    const hz=wasm.esp32sim_cpu_hz(e);
    for(let i=0;i<8;i++){assert.equal(wasm.esp32sim_run(e,hz/1000,0),0,logs.join('\n'));drain();}
    assert.equal(wasm.esp32sim_audio_output_flush(e),0);drain();
    assert.deepEqual(output,expected,'captured output must contain the actual RX DMA samples in order');
    assert.ok(rates.length>0);assert.ok(rates.every(rate=>rate===8000));
    assert.equal(state(e).consumedFrames,samples.length/channels);
    assert.equal(state(e).queuedFrames,0);
    const count=output.length;
    assert.equal(wasm.esp32sim_audio_output_flush(e),0);drain();assert.equal(output.length,count,'flush must not duplicate samples');
    for(let i=0;i<25;i++){assert.equal(wasm.esp32sim_run(e,hz/1000,0),0);drain();}
    assert.equal(output.length,count,'later periodic publication must not replay drained samples');
    assert.ok(wasm.esp32sim_insns(e)>0,'fixture must execute guest instructions');
  });
}
loopback(1,8000,[-32768,-1234,0,1234,32767],[-32768,-1234,0,1234,32767]);
loopback(2,16000,[111,-111,1234,-2345,333,-333,-32768,32767],[1234,-32768]);
assert.equal(logs.some(line=>line.includes('panic')),false,logs.join('\n'));
console.log('PASS: real WASM audio ABI bounds, lifecycle, chip capabilities and exact RX→GDMA→TX PCM loopback (mono + stereo rate conversion).');
