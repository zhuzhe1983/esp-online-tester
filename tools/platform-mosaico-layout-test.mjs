import assert from 'node:assert/strict';
import {test} from 'node:test';
import {chipIdForBoard,validateBoardRevision} from '../web/platform/app-image.js';
import {validateMergedImage} from '../web/platform/runtime.js';
import {assembleLocalFirmware} from '../web/platform/local-firmware.js';

// Layout fixture only; not firmware execution evidence. S31 ID and boot offset
// are from IDF v6.1 esp_app_format.h and esptool targets/esp32s31.py.
const image=()=>{const b=new Uint8Array(40),v=new DataView(b.buffer);b[0]=0xe9;b[1]=1;v.setUint16(12,32,true);v.setUint32(24,0x2f000000,true);v.setUint32(28,4,true);return b;};
const table=()=>{const b=new Uint8Array(4096).fill(255),v=new DataView(b.buffer);v.setUint16(0,0x50aa,true);b[2]=0;b[3]=16;v.setUint32(4,0x210000,true);v.setUint32(8,0xcf0000,true);return b;};
test('S31 uses its own chip ID, rev0 and bootloader at 0x2000',()=>{
  assert.equal(chipIdForBoard('mosaico'),32);assert.equal(chipIdForBoard('esp-mosaico'),32);
  assert.equal(chipIdForBoard('esp32s3'),9);
  const merged=new Uint8Array(0x210028).fill(255);merged.set(image(),0x2000);merged.set(table(),0x8000);merged.set(image(),0x210000);
  assert.equal(validateMergedImage(merged,'mosaico',16*1024*1024),true);
  assert.throws(()=>validateMergedImage(merged,'stamp-p4',16*1024*1024),{code:'CHIP_MISMATCH'});
  assert.throws(()=>validateMergedImage(merged,'mosaico',8*1024*1024),{code:'FLASH_OVERFLOW'});
  const later=image();new DataView(later.buffer).setUint16(15,100,true);
  assert.throws(()=>validateBoardRevision(later,'mosaico'),{code:'CHIP_REVISION_UNSUPPORTED'});
});
test('local S31 segments preserve the actual boot and application offsets',async()=>{
  const source={format:'segments',files:[{offset:0x2000,file:new File([image()],'bootloader.bin')},{offset:0x8000,file:new File([table()],'partition-table.bin')},{offset:0x210000,file:new File([image()],'main.bin')}]};
  const result=await assembleLocalFirmware(source,{board:'mosaico',flashBytes:16*1024*1024});
  assert.equal(validateMergedImage(result.data,'mosaico',16*1024*1024),true);
  await assert.rejects(()=>assembleLocalFirmware({...source,files:[{...source.files[0],offset:0},...source.files.slice(1)]},{board:'mosaico',flashBytes:16*1024*1024}),{code:'BOOTLOADER_REQUIRED'});
});
