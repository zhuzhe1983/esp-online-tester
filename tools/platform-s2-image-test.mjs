// Image and alias-boundary checks. Real Xtensa execution has a separate Worker/WASM probe.
import test from 'node:test';
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {validateAppImage,chipForBoard,chipIdForBoard} from '../web/platform/app-image.js';

const boards=['waveshare-s2-pico','waveshare-s2-lcd096','lilygo-esp32-s2-woor','lilygo-esp32-s2','lilygo-t-dongle-esp32-s2-1-14-inch-lcd-board'];
function image({segments=[{address:0x40020000,length:4}],entry=0x40020000,chip=2,min=0}={}) {
  const end=24+segments.reduce((n,s)=>n+8+s.length,0),checksumAt=Math.floor(end/16)*16+15;
  const bytes=new Uint8Array(checksumAt+33),view=new DataView(bytes.buffer);
  bytes[0]=0xe9;bytes[1]=segments.length;bytes[3]=0x20;bytes[23]=1;
  view.setUint32(4,entry,true);view.setUint16(12,chip,true);view.setUint16(15,min,true);
  let at=24,checksum=0xef;
  for(const segment of segments){
    view.setUint32(at,segment.address,true);view.setUint32(at+4,segment.length,true);at+=8;
    for(let i=0;i<segment.length;i++){bytes[at+i]=segment.data?.[i]??(i%2?0xf0:0x3d);checksum^=bytes[at+i];}at+=segment.length;
  }
  bytes[checksumAt]=checksum;bytes.set(createHash('sha256').update(bytes.subarray(0,checksumAt+1)).digest(),checksumAt+1);
  return bytes;
}
const parse=bytes=>validateAppImage(bytes,boards[0],4*1024*1024);
const reject=(options,code)=>assert.rejects(()=>parse(image(options)),{code});

test('S2 selects chip 2 and accepts disjoint physical IRAM/DRAM allocations at exact SRAM bounds',async()=>{
  for(const board of boards){assert.equal(chipForBoard(board),'esp32s2');assert.equal(chipIdForBoard(board),2);assert.equal((await validateAppImage(image(),board,4*1024*1024)).chipId,2);}
  await parse(image({segments:[{address:0x3ffb0000,length:4},{address:0x40020004,length:4}],entry:0x40020004}));
  await parse(image({segments:[{address:0x4006fffc,length:4}],entry:0x4006fffc}));
  await parse(image({segments:[{address:0x40020000,length:4},{address:0x3ffffffc,length:4}]}));
  const bare=await parse(image({segments:[{address:0x3ffb0000,length:4,data:[0x32,0x54,0xcd,0xab]},{address:0x40020004,length:4}],entry:0x40020004}));
  assert.equal(bare.applicationDescriptor,false,'A short ordinary data constant is not a truncated descriptor');
});
test('S2 rejects IRAM/DRAM alias overlaps and cannot execute from its data alias',async()=>{
  for(const segments of [
    [{address:0x40020000,length:4},{address:0x3ffb0000,length:4}],
    [{address:0x3ffb0010,length:8},{address:0x40020014,length:4}],
    [{address:0x40020000,length:8},{address:0x40020004,length:4}],
  ])await reject({segments},'S2_SRAM_OVERLAP');
  await reject({segments:[{address:0x3ffb0000,length:4}],entry:0x3ffb0000},'S2_SRAM_ENTRY_INVALID');
  for(const entry of [0x40020002,0x40020004,0x40000000])await reject({entry},'S2_SRAM_ENTRY_INVALID');
});
test('S2 refuses Flash, ROM, RTC, MMIO, overflow, unsupported revision and S3 images',async()=>{
  for(const address of [0x40000000,0x40070000,0x3ffafffc,0x3f000000,0x50000000,0x3f404000,0xfffffffc])await reject({segments:[{address,length:4}]},'S2_SRAM_REQUIRED');
  await reject({segments:[{address:0x4006fffc,length:8}],entry:0x4006fffc},'S2_SRAM_REQUIRED');
  await reject({segments:[{address:0x40020001,length:4}]},'S2_SRAM_SEGMENT_INVALID');
  await reject({chip:9},'CHIP_MISMATCH');await reject({min:1},'CHIP_REVISION_UNSUPPORTED');
  const bad=image();bad[32]^=1;await assert.rejects(()=>parse(bad),{code:'APP_IMAGE_CHECKSUM'});
});
