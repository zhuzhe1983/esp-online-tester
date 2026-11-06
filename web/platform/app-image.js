// ESP-IDF v5.5.4 esp_app_format.h / esp_app_desc.h; esptool image checksum
// is XOR of segment bytes seeded with 0xef, at the end of a 16-byte block.
// An application descriptor distinguishes app-only files from bootloaders.
// https://docs.espressif.com/projects/esptool/en/latest/esp32s3/advanced-topics/firmware-image-format.html
// https://github.com/espressif/esp-idf/blob/v5.5.4/components/esp_app_format/include/esp_app_desc.h
const fail=(code,message)=>{throw Object.assign(new Error(message),{code});};
const check=(value,code,message)=>{if(!value)fail(code,message);};
export const isC5Board=board=>['esp32c5','stamp-c5','stamp-c5-dip'].includes(board);
export const isC61Board=board=>['esp32c61','corematrix'].includes(board);
// ESP-IDF v5.5.4 esp_app_format.h: C61 is chip 20, distinct from C6 (13).
export const chipIdForBoard=board=>isC61Board(board)?20:isC5Board(board)?23:/^(esp32|ttgo-tdisplay|atom-lite|atom-matrix|atom-voice|atom-matrix-v11|stamppico|lilygo-t-beam-v1-2)$/.test(board)?0:['esp32c3','stampc3','stampc3u'].includes(board)?5:(board==='nesso-n1'||board.includes('c6'))?13:['esp32p4','stamp-p4'].includes(board)?18:9;
export const C5_SRAM_RANGES=Object.freeze([Object.freeze({start:0x40800000,end:0x40860000}),Object.freeze({start:0x50000000,end:0x50004000})]);
// S013's source-backed virtual profile is rev < 3; do not silently change ROM
// families for an image targeting ECO5. ESP-IDF esp_image_header_t offsets 15/17.
export function validateBoardRevision(bytes,board) {
  if(board!=='stamp-p4'&&!isC61Board(board))return;
  check(bytes.length>=24,'INCOMPLETE_IMAGE','Image header is truncated.');
  const view=new DataView(bytes.buffer,bytes.byteOffset,bytes.byteLength);
  if(isC61Board(board)) {
    check(view.getUint16(15,true)<=100&&(view.getUint16(17,true)===0||view.getUint16(17,true)>=100),'CHIP_REVISION_UNSUPPORTED','CoreMatrix uses ESP32-C61 rev100; this image excludes that revision.');
    return;
  }
  check(view.getUint16(15,true)<300,'CHIP_REVISION_UNSUPPORTED','Stamp-P4 currently uses the rev < 3 runtime profile; this image requires rev >= 3.');
}
export async function validateAppImage(bytes,board,flashBytes) {
  if(isC5Board(board))return validateC5SramImage(bytes,flashBytes);
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

// C5's explicit bare-metal app-direct contract. This is not a ROM/IDF loader.
// esp_image_header_t + esptool checksum/SHA layout are shared with ESP images;
// every load/entry address must also fit the C5 Machine's implemented SRAM.
async function validateC5SramImage(bytes,flashBytes) {
  check(bytes.length>=36&&bytes[0]===0xe9&&bytes[1]>0&&bytes[1]<=16,'APP_IMAGE_INVALID','Choose a complete ESP32-C5 SRAM application image.');
  const view=new DataView(bytes.buffer,bytes.byteOffset,bytes.byteLength);
  check(view.getUint16(12,true)===23,'CHIP_MISMATCH','Application targets a different ESP32 chip.');
  // IDF5.5.4 bootloader_common_loader.c uses the full revision fields; byte14
  // is deprecated (not a full revision value) and must not reject valid images.
  check(view.getUint16(15,true)<=100&&(view.getUint16(17,true)===0||view.getUint16(17,true)>=100),'CHIP_REVISION_UNSUPPORTED','This application does not support the ESP32-C5 rev100 runtime profile.');
  check(bytes.length+0x10000<=flashBytes,'FLASH_OVERFLOW','Application exceeds the product flash size at the direct-boot offset.');
  check(bytes[23]===0||bytes[23]===1,'APP_IMAGE_INVALID','Invalid application digest flag.');
  const entry=view.getUint32(4,true),segments=[];let cursor=24,checksum=0xef;
  for(let i=0;i<bytes[1];i++) {
    check(cursor+8<=bytes.length,'APP_IMAGE_INVALID','Application segment header is truncated.');
    const address=view.getUint32(cursor,true),length=view.getUint32(cursor+4,true);cursor+=8;
    check(length>0&&address%4===0&&length%4===0,'C5_SRAM_SEGMENT_INVALID','C5 SRAM segments must be nonempty and 4-byte aligned.');
    check(C5_SRAM_RANGES.some(r=>address>=r.start&&address+length<=r.end),'C5_SRAM_REQUIRED','C5 currently runs SRAM applications only. Flash-mapped IDF/UIFlow images need an unsupported ROM/cache startup.');
    check(!segments.some(s=>address<s.address+s.length&&s.address<address+length),'C5_SRAM_OVERLAP','Application SRAM segments overlap.');
    check(cursor+length<=bytes.length,'APP_IMAGE_INVALID','Application segment is truncated.');
    for(const byte of bytes.subarray(cursor,cursor+length))checksum^=byte;
    segments.push({address,length});cursor+=length;
  }
  check(entry%2===0&&segments.some(s=>entry>=s.address&&entry+2<=s.address+s.length),'C5_SRAM_ENTRY_INVALID','C5 entry must be 2-byte aligned and inside a loaded SRAM segment.');
  const applicationDescriptor=view.getUint32(32,true)===0xabcd5432;
  check(!applicationDescriptor||segments[0].length>=256,'APP_IMAGE_INVALID','ESP-IDF application descriptor is truncated.');
  const checksumAt=Math.floor(cursor/16)*16+15;
  check(checksumAt<bytes.length,'APP_IMAGE_INVALID','Application checksum is missing.');
  check(bytes.subarray(cursor,checksumAt).every(v=>v===0),'APP_IMAGE_INVALID','C5 application checksum padding must be zero.');
  check(bytes[checksumAt]===checksum,'APP_IMAGE_CHECKSUM','Application image checksum does not match.');
  let end=checksumAt+1;
  if(bytes[23]) {
    check(end+32<=bytes.length,'APP_IMAGE_INVALID','Application digest is truncated.');
    const digest=new Uint8Array(await crypto.subtle.digest('SHA-256',bytes.subarray(0,end)));
    check(digest.every((v,i)=>v===bytes[end+i]),'APP_IMAGE_DIGEST','Application embedded SHA-256 does not match.');end+=32;
  }
  check(bytes.subarray(end).every(v=>v===0xff),'APP_IMAGE_INVALID','C5 SRAM app-direct accepts one application image followed only by erased 0xff bytes.');
  return {imageBytes:end,chipId:23,entry,segments,applicationDescriptor};
}
