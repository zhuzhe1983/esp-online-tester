import test from 'node:test';
import assert from 'node:assert/strict';
import {validateLoRaPacket} from '../web/platform/lora-input.js';
const packet={config:{frequency:0x36400000,sf:12,bandwidth:3,codingRate:4,syncWord:0x3444,implicitHeader:false,crc:true,invertIq:false},payloadHex:'50454552',rssiDbm:-70,snrQuarterDb:24,crcError:false};
test('LoRa peer validation preserves explicit channel and payload',()=>{
  const checked=validateLoRaPacket(packet);assert.deepEqual(checked,packet);assert.notEqual(checked.config,packet.config);
  const mismatched=structuredClone(packet);mismatched.config.frequency++;assert.equal(validateLoRaPacket(mismatched).config.frequency,0x36400001);
});
test('LoRa peer input rejects invalid sizes, non-finite and lossy values',()=>{
  for(const change of [{payloadHex:'a'},{payloadHex:'xx'},{payloadHex:'ff'.repeat(256)},{rssiDbm:1},{rssiDbm:-70.1},{snrQuarterDb:128},{crcError:0}])assert.throws(()=>validateLoRaPacket({...packet,...change}),{code:'INVALID_LORA_PACKET'});
  for(const change of [{frequency:0x100000000},{sf:4},{bandwidth:7},{codingRate:0},{syncWord:65536},{invertIq:null}])assert.throws(()=>validateLoRaPacket({...packet,config:{...packet.config,...change}}),{code:'INVALID_LORA_PACKET'});
});
