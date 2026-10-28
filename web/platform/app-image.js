// ESP-IDF v5.5.4 esp_app_format.h / esp_app_desc.h; esptool image checksum
// is XOR of segment bytes seeded with 0xef, at the end of a 16-byte block.
// An application descriptor distinguishes app-only files from bootloaders.
// https://docs.espressif.com/projects/esptool/en/latest/esp32s3/advanced-topics/firmware-image-format.html
// https://github.com/espressif/esp-idf/blob/v5.5.4/components/esp_app_format/include/esp_app_desc.h
const fail=(code,message)=>{throw Object.assign(new Error(message),{code});};
const check=(value,code,message)=>{if(!value)fail(code,message);};
export const chipIdForBoard=board=>/^(esp32|ttgo-tdisplay|atom-lite|atom-matrix|atom-voice)$/.test(board)?0:board==='esp32c3'?5:(board==='nesso-n1'||board.includes('c6'))?13:board==='esp32p4'?18:9;
export async function validateAppImage(bytes,board,flashBytes) {
  check(bytes.length>=288&&bytes[0]===0xe9&&bytes[1]>0&&bytes[1]<=16,'APP_IMAGE_INVALID','Not a complete ESP application image.');
  const view=new DataView(bytes.buffer,bytes.byteOffset,bytes.byteLength);
  check(view.getUint32(32,true)===0xabcd5432&&view.getUint32(28,true)>=256,'APP_IMAGE_INVALID','Application descriptor is missing; this is not an app-only image.');
  check(view.getUint16(12,true)===chipIdForBoard(board),'CHIP_MISMATCH','Application targets a different ESP32 chip.');
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
