// ESP-IDF v5.5.4 esp_app_format.h / esp_app_desc.h; esptool image checksum
// is XOR of segment bytes seeded with 0xef, at the end of a 16-byte block.
// An application descriptor distinguishes app-only files from bootloaders.
// https://docs.espressif.com/projects/esptool/en/latest/esp32s3/advanced-topics/firmware-image-format.html
// https://github.com/espressif/esp-idf/blob/v5.5.4/components/esp_app_format/include/esp_app_desc.h
import {isCore2Board,isClassicStickBoard} from './board-compatibility.js';
import {LILYGO_BOARD_CHIPS} from './lilygo-board-families.js';
import {WAVESHARE_BOARD_CAPABILITIES} from './waveshare-board-families.js';
export const isP4Rev3Board=board=>WAVESHARE_BOARD_CAPABILITIES[board]?.virtualRevision===301||['waveshare-p4-lcd35','p4-lcd35','lcd35','esp32p4-rev3','p4-rev3'].includes(board);
const fail=(code,message)=>{throw Object.assign(new Error(message),{code});};
const check=(value,code,message)=>{if(!value)fail(code,message);};
export const isC5Board=board=>WAVESHARE_BOARD_CAPABILITIES[board]?.chip==='esp32c5'||['esp32c5','stamp-c5','stamp-c5-dip','lilygo-t7-c5','lilygo-t-dongle-c5'].includes(board);
export const isS2Board=board=>WAVESHARE_BOARD_CAPABILITIES[board]?.chip==='esp32s2'||['waveshare-s2-pico','waveshare-s2-lcd096','esp32s2','lilygo-esp32-s2-woor','lilygo-esp32-s2','lilygo-t-dongle-esp32-s2-1-14-inch-lcd-board'].includes(board);
export const isH2Board=board=>WAVESHARE_BOARD_CAPABILITIES[board]?.chip==='esp32h2'||['esp32h2','waveshare-h2-zero','waveshare-h2-dev-kit-n4'].includes(board);
export const isSramOnlyBoard=board=>isS2Board(board)||isH2Board(board);
export const isS31Board=board=>WAVESHARE_BOARD_CAPABILITIES[board]?.chip==='esp32s31'||['esp32s31','mosaico','esp-mosaico','metalio-ink4-plus'].includes(board);
export const isC61Board=board=>['esp32c61','corematrix'].includes(board);
export const isC3Board=board=>WAVESHARE_BOARD_CAPABILITIES[board]?.chip==='esp32c3'||['esp32c3','stampc3','stampc3u','seeed-xiao-epaper-75'].includes(board);
// ESP-IDF v6.1 esp_app_format.h: S31 = 0x0020; v5.5.4: C61 = 20, distinct from C6 (13).
export const chipForBoard=board=>WAVESHARE_BOARD_CAPABILITIES[board]?.chip||(isP4Rev3Board(board)?'esp32p4':isS31Board(board)?'esp32s31':isC61Board(board)?'esp32c61':isC5Board(board)?'esp32c5':isS2Board(board)?'esp32s2':isH2Board(board)?'esp32h2':LILYGO_BOARD_CHIPS[board]||(isCore2Board(board)||isClassicStickBoard(board)||/^(esp32|ttgo-tdisplay|atom-lite|atom-matrix|atom-voice|atom-matrix-v11|stamppico|lilygo-t-beam-v1-2|m5stack-basic|m5-tough)$/.test(board)?'esp32':isC3Board(board)?'esp32c3':(board==='nesso-n1'||String(board).includes('c6'))?'esp32c6':['esp32p4','stamp-p4'].includes(board)?'esp32p4':'esp32s3'));
export const chipIdForBoard=board=>({esp32:0,esp32s2:2,esp32c3:5,esp32s3:9,esp32c6:13,esp32h2:16,esp32p4:18,esp32c61:20,esp32c5:23,esp32s31:32})[chipForBoard(board)];
export const isC6Board=board=>chipForBoard(board)==='esp32c6';
export const C5_SRAM_RANGES=Object.freeze([Object.freeze({start:0x40800000,end:0x40860000}),Object.freeze({start:0x50000000,end:0x50004000})]);
// IDF v5.5.4 esp32s2/soc.h: two aliases of one 320 KiB SRAM, not 640 KiB.
export const S2_SRAM_RANGES=Object.freeze([Object.freeze({start:0x40020000,end:0x40070000,kind:'iram'}),Object.freeze({start:0x3ffb0000,end:0x40000000,kind:'dram'})]);
// H2 TRM v1.2 / IDF v5.5.1: 320 KiB HP SRAM and 4 KiB LP SRAM.
export const H2_SRAM_RANGES=Object.freeze([Object.freeze({start:0x40800000,end:0x40850000}),Object.freeze({start:0x50000000,end:0x50001000})]);
// S013's source-backed virtual profile is rev < 3; do not silently change ROM
// families for an image targeting ECO5. ESP-IDF esp_image_header_t offsets 15/17.
export function validateBoardRevision(bytes,board) {
  if(board!=='stamp-p4'&&WAVESHARE_BOARD_CAPABILITIES[board]?.chip!=='esp32p4'&&LILYGO_BOARD_CHIPS[board]!=='esp32p4'&&!isP4Rev3Board(board)&&!isC5Board(board)&&!isC61Board(board)&&!isS31Board(board))return;
  check(bytes.length>=24,'INCOMPLETE_IMAGE','Image header is truncated.');
  const view=new DataView(bytes.buffer,bytes.byteOffset,bytes.byteLength);
  if(isS31Board(board)) {
    check(view.getUint16(15,true)===0,'CHIP_REVISION_UNSUPPORTED','This ESP32-S31 runtime uses rev0; this image requires a later revision.');
    return;
  }
  // C5 MP: IDF v5.5.3@2c211b236707889e8400c4dc5644dd5c4ee071e0
  // esp_app_format.h / bootloader_common_loader.c use full min/max fields.
  if(isC5Board(board)) {
    check(view.getUint16(15,true)<=100&&(view.getUint16(17,true)===0||view.getUint16(17,true)>=100),'CHIP_REVISION_UNSUPPORTED','ESP32-C5 uses MP rev100; this image excludes that revision.');
    return;
  }
  if(isC61Board(board)) {
    check(view.getUint16(15,true)<=100&&(view.getUint16(17,true)===0||view.getUint16(17,true)>=100),'CHIP_REVISION_UNSUPPORTED','CoreMatrix uses ESP32-C61 rev100; this image excludes that revision.');
    return;
  }
  if(isP4Rev3Board(board)) {
    const minimum=view.getUint16(15,true),maximum=view.getUint16(17,true);
    check(minimum>=300&&minimum<=301&&(maximum===0||maximum>=301),'CHIP_REVISION_UNSUPPORTED','This P4 virtual profile uses rev3.1 and requires an image built for rev >= 3 that includes rev3.1.');
    return;
  }
  if(WAVESHARE_BOARD_CAPABILITIES[board]?.virtualRevision===100) {
    check(view.getUint16(15,true)<=100&&(view.getUint16(17,true)===0||view.getUint16(17,true)>=100),'CHIP_REVISION_UNSUPPORTED','This P4 virtual profile uses rev1.0 and requires an image that includes that revision.');
    return;
  }
  check(view.getUint16(15,true)<300,'CHIP_REVISION_UNSUPPORTED','Stamp-P4 currently uses the rev < 3 runtime profile; this image requires rev >= 3.');
}
export async function validateAppImage(bytes,board,flashBytes) {
  if(isC5Board(board)||isSramOnlyBoard(board))return validateSramImage(bytes,flashBytes,chipForBoard(board));
  check(bytes.length>=288&&bytes[0]===0xe9&&bytes[1]>0&&bytes[1]<=16,'APP_IMAGE_INVALID','Not a complete ESP application image.');
  const view=new DataView(bytes.buffer,bytes.byteOffset,bytes.byteLength);
  check(view.getUint32(32,true)===0xabcd5432&&view.getUint32(28,true)>=256,'APP_IMAGE_INVALID','Application descriptor is missing; this is not an app-only image.');
  check(view.getUint16(12,true)===chipIdForBoard(board),'CHIP_MISMATCH','Application targets a different ESP32 chip.');
  validateBoardRevision(bytes,board);
  check(bytes.length+0x10000<=flashBytes,'FLASH_OVERFLOW','Application exceeds the product flash size at the direct-boot offset.');
  check(bytes[23]===0||bytes[23]===1,'APP_IMAGE_INVALID','Invalid application digest flag.');
  let cursor=24,checksum=0xef;
  for(let i=0;i<bytes[1];i++) {
    check(cursor+8<=bytes.length,'APP_IMAGE_INVALID','Application segment header is truncated.');
    const length=view.getUint32(cursor+4,true);cursor+=8;
    check(cursor+length<=bytes.length,'APP_IMAGE_INVALID','Application segment is truncated.');
    for(const byte of bytes.subarray(cursor,cursor+length))checksum^=byte;
    cursor+=length;
  }
  const checksumAt=Math.floor(cursor/16)*16+15;
  check(checksumAt<bytes.length,'APP_IMAGE_INVALID','Application checksum is missing.');
  check(bytes[checksumAt]===checksum,'APP_IMAGE_CHECKSUM','Application image checksum does not match.');
  let end=checksumAt+1;
  if(bytes[23]) {
    check(end+32<=bytes.length,'APP_IMAGE_INVALID','Application digest is truncated.');
    const digest=new Uint8Array(await crypto.subtle.digest('SHA-256',bytes.subarray(0,end)));
    check(digest.every((v,i)=>v===bytes[end+i]),'APP_IMAGE_DIGEST','Application embedded SHA-256 does not match.');end+=32;
  }
  check(bytes.subarray(end).every(v=>v===0xff||v===0),'APP_IMAGE_INVALID','File contains additional data beyond one application image; use its flash layout.');
  return {imageBytes:end,chipId:chipIdForBoard(board)};
}

// Explicit bare-metal app-direct contracts. This is not a ROM/IDF loader.
// esp_image_header_t + esptool checksum/SHA layout are shared with ESP images;
// Every load/entry address must fit the selected Machine's implemented SRAM.
async function validateSramImage(bytes,flashBytes,kind) {
  const s2=kind==='esp32s2',h2=kind==='esp32h2';
  const chip=s2?'S2':h2?'H2':'C5',chipId=s2?2:h2?16:23,revision=s2?0:h2?null:100,ranges=s2?S2_SRAM_RANGES:h2?H2_SRAM_RANGES:C5_SRAM_RANGES;
  check(bytes.length>=36&&bytes[0]===0xe9&&bytes[1]>0&&bytes[1]<=16,'APP_IMAGE_INVALID',`Choose a complete ESP32-${chip} SRAM application image.`);
  const view=new DataView(bytes.buffer,bytes.byteOffset,bytes.byteLength);
  check(view.getUint16(12,true)===chipId,'CHIP_MISMATCH','Application targets a different ESP32 chip.');
  if(s2)check(bytes[14]===0,'CHIP_REVISION_UNSUPPORTED','This S2 diagnostic profile models revision 0; the legacy minimum revision must also be 0.');
  // IDF5.5.4 bootloader_common_loader.c uses the full revision fields; byte14
  // is deprecated (not a full revision value) and must not reject valid images.
  if(h2) {
    check(view.getUint16(15,true)===0&&[0,65535].includes(view.getUint16(17,true)),'CHIP_REVISION_UNSUPPORTED','H2 wafer revision is unverified; only unrestricted revision headers are accepted.');
    check(flashBytes===4*1024*1024&&(bytes[3]>>4)<=2,'FLASH_CAPACITY_UNSUPPORTED','H2 requires a 4 MiB board and an image declaring 1, 2 or 4 MiB Flash.');
  } else check(view.getUint16(15,true)<=revision&&(view.getUint16(17,true)===0||view.getUint16(17,true)>=revision),'CHIP_REVISION_UNSUPPORTED',`This application excludes the ESP32-${chip} rev${revision} diagnostic profile.`);
  check(bytes.length+0x10000<=flashBytes,'FLASH_OVERFLOW','Application exceeds the product flash size at the direct-boot offset.');
  check(bytes[23]===0||bytes[23]===1,'APP_IMAGE_INVALID','Invalid application digest flag.');
  const entry=view.getUint32(4,true),segments=[];let cursor=24,checksum=0xef;
  for(let i=0;i<bytes[1];i++) {
    check(cursor+8<=bytes.length,'APP_IMAGE_INVALID','Application segment header is truncated.');
    const address=view.getUint32(cursor,true),length=view.getUint32(cursor+4,true);cursor+=8;
    check(length>0&&address%4===0&&length%4===0,`${chip}_SRAM_SEGMENT_INVALID`,`${chip} SRAM segments must be nonempty and 4-byte aligned.`);
    const range=ranges.find(r=>address>=r.start&&address+length<=r.end);
    check(range,`${chip}_SRAM_REQUIRED`,`${chip} currently runs SRAM applications only; ROM/cache, mapped Flash and other unlisted memory regions are outside this profile.`);
    const physicalOffset=s2?address-range.start:address;
    check(!segments.some(s=>physicalOffset<s.physicalOffset+s.length&&s.physicalOffset<physicalOffset+length),`${chip}_SRAM_OVERLAP`,'Application SRAM segments overlap, including physical aliases.');
    check(cursor+length<=bytes.length,'APP_IMAGE_INVALID','Application segment is truncated.');
    for(const byte of bytes.subarray(cursor,cursor+length))checksum^=byte;
    segments.push({address,length,physicalOffset});cursor+=length;
  }
  const alignment=s2?4:2;
  check(entry%alignment===0&&(!s2||entry>=S2_SRAM_RANGES[0].start&&entry+alignment<=S2_SRAM_RANGES[0].end)&&segments.some(s=>entry>=s.address&&entry+alignment<=s.address+s.length),`${chip}_SRAM_ENTRY_INVALID`,`${chip} entry must be ${alignment}-byte aligned and inside a loaded ${s2?'IRAM':'SRAM'} segment.`);
  const descriptorMagic=view.getUint32(32,true)===0xabcd5432;
  // A bare S2 data segment can legitimately start with this integer constant.
  // Only a complete descriptor enables implicit app-only URL detection.
  const applicationDescriptor=descriptorMagic&&(!s2||segments[0].length>=256);
  if(!s2)check(!applicationDescriptor||segments[0].length>=256,'APP_IMAGE_INVALID','ESP-IDF application descriptor is truncated.');
  const checksumAt=Math.floor(cursor/16)*16+15;
  check(checksumAt<bytes.length,'APP_IMAGE_INVALID','Application checksum is missing.');
  check(bytes.subarray(cursor,checksumAt).every(v=>v===0),'APP_IMAGE_INVALID',`${chip} application checksum padding must be zero.`);
  check(bytes[checksumAt]===checksum,'APP_IMAGE_CHECKSUM','Application image checksum does not match.');
  let end=checksumAt+1;
  if(bytes[23]) {
    check(end+32<=bytes.length,'APP_IMAGE_INVALID','Application digest is truncated.');
    const digest=new Uint8Array(await crypto.subtle.digest('SHA-256',bytes.subarray(0,end)));
    check(digest.every((v,i)=>v===bytes[end+i]),'APP_IMAGE_DIGEST','Application embedded SHA-256 does not match.');end+=32;
  }
  check(bytes.subarray(end).every(v=>v===0xff),'APP_IMAGE_INVALID',`${chip} SRAM app-direct accepts one application image followed only by erased 0xff bytes.`);
  return {imageBytes:end,chipId,entry,segments:segments.map(({address,length})=>({address,length})),applicationDescriptor};
}
