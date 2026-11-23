import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {validateLoRaPacket} from '../web/platform/lora-input.js';
import {createPlatformRuntime} from '../web/platform/runtime.js';
import {createDefaultScene} from '../web/platform/catalog.js';
import {firmwareBoardMatches} from '../web/platform/board-compatibility.js';
import {chipIdForBoard} from '../web/platform/app-image.js';
const packet={config:{frequency:0x36400000,sf:12,bandwidth:3,codingRate:4,syncWord:0x3444,implicitHeader:false,crc:true,invertIq:false},payloadHex:'50454552',rssiDbm:-70,snrQuarterDb:24,crcError:false};
const indicatorRadioBoards=['seeed-sensecap-indicator-d1l','seeed-sensecap-indicator-d1pro','seeed-sensecap-indicator-meshtastic'];
const catalog=Object.fromEntries(await Promise.all(['products','accessories','firmwares'].map(async name=>[name,JSON.parse(await readFile(new URL(`../web/platform/catalog/${name}.json`,import.meta.url))).items])));
test('LoRa peer validation preserves explicit channel and payload',()=>{
  const checked=validateLoRaPacket(packet);assert.deepEqual(checked,packet);assert.notEqual(checked.config,packet.config);
  const mismatched=structuredClone(packet);mismatched.config.frequency++;assert.equal(validateLoRaPacket(mismatched).config.frequency,0x36400001);
});
test('LoRa peer input rejects invalid sizes, non-finite and lossy values',()=>{
  for(const change of [{payloadHex:'a'},{payloadHex:'xx'},{payloadHex:'ff'.repeat(256)},{rssiDbm:1},{rssiDbm:-70.1},{snrQuarterDb:128},{crcError:0}])assert.throws(()=>validateLoRaPacket({...packet,...change}),{code:'INVALID_LORA_PACKET'});
  for(const change of [{frequency:0x100000000},{sf:4},{bandwidth:7},{codingRate:0},{syncWord:65536},{invertIq:null}])assert.throws(()=>validateLoRaPacket({...packet,config:{...packet.config,...change}}),{code:'INVALID_LORA_PACKET'});
});

test('Indicator radio editions require exact board identity and the S3 chip',()=>{
  for(const id of indicatorRadioBoards){
    assert.equal(catalog.products.find(p=>p.id===id).runtime.board,id);
    assert.equal(chipIdForBoard(id),9);
    assert.equal(firmwareBoardMatches(id,id),true);
    for(const other of [...indicatorRadioBoards.filter(x=>x!==id),'seeed-sensecap-indicator-gx','seeed-sensecap-indicator','esp32s3','generic-esp32s3']){
      assert.equal(firmwareBoardMatches(other,id),false,'A same-chip or neighboring SKU alias cannot bypass the manifest identity');
    }
  }
});

// A structural image and worker transport double test API routing only. They
// execute no firmware and provide no evidence of pixels, RF or radio hardware.
function transportFixture(){
  const messages=[],state={accept:true};
  const workerFactory=()=>{
    const worker={terminate(){},postMessage(m){let response;
      if(m.op==='init')response={ready:true};if(m.op==='create')response={created:true,flashStorage:false};
      if(m.op==='load')response={loaded:m.at===undefined?m.kind:`at${m.at}`,ok:true};
      if(m.op==='start')response={started:true};if(m.op==='resume')response={resumed:true};if(m.op==='stop')response={stopped:0};
      if(m.op==='lora-read'||m.op==='lora-receive'){
        messages.push(structuredClone(m));response={loraResult:m.requestId,ok:state.accept,time:1.25,...(m.op==='lora-read'?{state:{mode:'rx'}}:{})};
      }
      if(response)queueMicrotask(()=>worker.onmessage?.({data:response}));
    }};return worker;
  };
  const runtime=createPlatformRuntime({catalog,workerFactory,fetchImpl:async()=>new Response(new Uint8Array([1,2,3])),baseUrl:'https://fixture.invalid/platform/',assetBaseUrl:'https://fixture.invalid/'});
  const bytes=new Uint8Array(0x10100),view=new DataView(bytes.buffer);
  for(const at of [0,0x10000]){bytes[at]=0xe9;bytes[at+1]=1;view.setUint16(at+12,9,true);view.setUint32(at+28,4,true);}
  view.setUint16(0x8000,0x50aa,true);view.setUint32(0x8004,0x10000,true);view.setUint32(0x8008,0x100000,true);
  return {runtime,messages,state,load:()=>runtime.loadFirmware({file:new Blob([bytes]),format:'merged'})};
}

test('Indicator radio API routes only the three populated editions and preserves packet rejection',async()=>{
  const f=transportFixture();try{
    for(const id of indicatorRadioBoards){
      await f.runtime.configure(createDefaultScene(catalog,id));
      const capability=(await f.runtime.capabilities()).lora;
      assert.equal(capability.available,true);assert.equal(capability.physicalRf,false);assert.equal(capability.model,'sx1262-lora-digital');
      await f.load();
      assert.deepEqual(await f.runtime.readLoRa(),{mode:'rx',guestTime:1.25});
      await assert.rejects(()=>f.runtime.receiveLoRa(packet),{code:'NOT_RUNNING'});
      await f.runtime.start();
      const result=await f.runtime.receiveLoRa(packet);
      assert.deepEqual(result,{accepted:true,delivery:'scheduled-digital-packet',guestTime:1.25,physicalRf:false});
      assert.deepEqual(f.messages.at(-1).packet,packet);assert.notEqual(f.messages.at(-1).packet,packet);
      const count=f.messages.length;
      await assert.rejects(()=>f.runtime.receiveLoRa({...packet,payloadHex:'x'}),{code:'INVALID_LORA_PACKET'});assert.equal(f.messages.length,count);
      f.state.accept=false;
      await assert.rejects(()=>f.runtime.receiveLoRa(packet),{code:'LORA_PACKET_REJECTED'});
      await assert.rejects(()=>f.runtime.readLoRa(),{code:'LORA_UNSUPPORTED'});f.state.accept=true;
    }
    for(const id of ['seeed-sensecap-indicator','seeed-sensecap-indicator-d1s']){
      await f.runtime.configure(createDefaultScene(catalog,id));assert.equal((await f.runtime.capabilities()).lora.available,false);
      await f.load();await f.runtime.start();const count=f.messages.length;
      await assert.rejects(()=>f.runtime.readLoRa(),{code:'LORA_UNSUPPORTED'});
      await assert.rejects(()=>f.runtime.receiveLoRa(packet),{code:'LORA_UNSUPPORTED'});assert.equal(f.messages.length,count);
    }
  }finally{await f.runtime.dispose();}
});
