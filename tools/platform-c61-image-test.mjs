import test from 'node:test';
import assert from 'node:assert/strict';
import {chipIdForBoard,validateBoardRevision} from '../web/platform/app-image.js';
import {validateMergedImage} from '../web/platform/runtime.js';
import {assembleLocalFirmware} from '../web/platform/local-firmware.js';
import {firmwareFlashPolicy} from '../web/platform/firmware-policy.js';

// Independent tiny ESP image/layout fixture; no downloaded guest firmware is committed.
const image=(chip=20,min=100,max=199)=>{
  const data=new Uint8Array(64),view=new DataView(data.buffer);
  data[0]=0xe9;data[1]=1;view.setUint16(12,chip,true);view.setUint16(15,min,true);view.setUint16(17,max,true);
  view.setUint32(24,0x40800000,true);view.setUint32(28,4,true);data[47]=0xef;return data;
};
const file=(name,data)=>new File([data],name),flashBytes=16*1024*1024;
function build(chip='esp32c61',id=20) {
  const table=new Uint8Array(0x1000).fill(0xff),view=new DataView(table.buffer);
  view.setUint16(0,0x50aa,true);table[2]=0;table[3]=0;view.setUint32(4,0x10000,true);view.setUint32(8,0x20000,true);
  return {format:'build',files:[file('flasher_args.json',JSON.stringify({flash_files:{'0x0':'bootloader.bin','0x8000':'partitions.bin','0x10000':'app.bin'},extra_esptool_args:{chip},flash_settings:{flash_size:'16MB'}})),file('bootloader.bin',image(id)),file('partitions.bin',table),file('app.bin',image(id))]};
}

test('C61 merged/build layouts stay distinct from C6 and S3 across board and chip names',async()=>{
  for(const board of ['corematrix','esp32c61']) {
    assert.equal(chipIdForBoard(board),20);
    const result=await assembleLocalFirmware(build(),{board,flashBytes});
    assert.equal(validateMergedImage(result.data,board,flashBytes),true);
    for(const other of ['esp32c6','nanoc6','stopwatch'])assert.throws(()=>validateMergedImage(result.data,other,flashBytes),{code:'CHIP_MISMATCH'});
    await assert.rejects(()=>assembleLocalFirmware(build('esp32c6',13),{board,flashBytes}),{code:'CHIP_MISMATCH'});
    // Correct build metadata never makes a different image chip acceptable.
    const wrong=await assembleLocalFirmware(build('esp32c61',13),{board,flashBytes});
    assert.throws(()=>validateMergedImage(wrong.data,board,flashBytes),{code:'CHIP_MISMATCH'});
  }
});

test('C61 rev100 validates both bootloader and application full revision fields',async()=>{
  for(const [min,max] of [[0,0],[100,199],[0,100]])assert.doesNotThrow(()=>validateBoardRevision(image(20,min,max),'corematrix'));
  for(const [min,max] of [[101,199],[0,99]])assert.throws(()=>validateBoardRevision(image(20,min,max),'corematrix'),{code:'CHIP_REVISION_UNSUPPORTED'});
  const {data}=await assembleLocalFirmware(build(),{board:'corematrix',flashBytes});
  for(const at of [0,0x10000]) {
    const changed=data.slice();new DataView(changed.buffer).setUint16(at+15,101,true);
    assert.throws(()=>validateMergedImage(changed,'corematrix',flashBytes),{code:'CHIP_REVISION_UNSUPPORTED'});
  }
});

test('C61 flashing accepts a complete source without conflating simulation readiness',()=>{
  const product={chip:'esp32c61',runtime:{board:null,flashMb:16}};
  assert.equal(firmwareFlashPolicy(product,{url:'https://example.com/corematrix.bin'}).canFlash,true);
  assert.equal(firmwareFlashPolicy(product,{url:'https://example.com/app.bin',bootMode:'app-direct'}).reason,'FLASH_LAYOUT');
  assert.equal(firmwareFlashPolicy({...product,chip:'esp32c6'},null).reason,'FIRMWARE_REQUIRED');
});
